import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import { CustomEase } from "gsap/CustomEase";
import { useRef, useState } from "react";

gsap.registerPlugin(useGSAP, CustomEase);

const ARTWORK_FLIP_HALF_DURATION_SECONDS = 0.24;
const ARTWORK_FLIP_OUT_EASE = CustomEase.create("notch-artwork-flip-out", "0.4,0,1,1");
const ARTWORK_FLIP_IN_EASE = CustomEase.create("notch-artwork-flip-in", "0,0,0.2,1");

type ArtworkTransitionImageProps = {
  artworkLoading: boolean;
  artworkUrl: string | null;
  alt: string;
  className?: string;
  draggable?: boolean;
  fallbackSrc: string;
};

export function ArtworkTransitionImage({
  artworkLoading,
  artworkUrl,
  alt,
  className,
  draggable,
  fallbackSrc,
}: ArtworkTransitionImageProps) {
  const imageRef = useRef<HTMLImageElement>(null);
  const displayedSrcRef = useRef(fallbackSrc);
  const [displayedSrc, setDisplayedSrc] = useState(fallbackSrc);
  const targetSrc = artworkUrl ?? (artworkLoading ? displayedSrcRef.current : fallbackSrc);

  useGSAP(() => {
    const image = imageRef.current;
    if (!image || targetSrc === displayedSrcRef.current) return;

    const setDisplayedSource = (nextSrc: string) => {
      displayedSrcRef.current = nextSrc;
      // 直接同步 DOM 源，确保中点换源发生在同一帧；React 状态随后保持声明式状态一致。
      image.src = nextSrc;
      setDisplayedSrc(nextSrc);
    };

    const media = gsap.matchMedia();
    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      const reduceMotion = Boolean(context.conditions?.reduceMotion);
      const resetImage = () => gsap.set(image, {
        scale: 1,
        rotationY: 0,
        transformPerspective: 800,
        transformOrigin: "50% 50%",
        clearProps: "willChange",
      });

      if (reduceMotion) {
        setDisplayedSource(targetSrc);
        resetImage();
        return;
      }

      let disposed = false;
      let started = false;
      let timeline: gsap.core.Timeline | null = null;
      const preload = new window.Image();

      const start = () => {
        if (disposed || started) return;
        started = true;
        timeline = gsap.timeline({
          onComplete: () => {
            gsap.set(image, { clearProps: "willChange" });
          },
        });
        gsap.set(image, {
          transformPerspective: 800,
          transformOrigin: "50% 50%",
          backfaceVisibility: "hidden",
          willChange: "transform",
        });
        timeline
          .to(image, {
            scale: 0.5,
            rotationY: 90,
            duration: ARTWORK_FLIP_HALF_DURATION_SECONDS,
            ease: ARTWORK_FLIP_OUT_EASE,
          }, 0)
          .add(() => {
            if (disposed) return;
            setDisplayedSource(targetSrc);
            // -90° 与 +90° 在侧面视觉上等价，后半程继续沿同一方向翻到正面。
            gsap.set(image, { rotationY: -90 });
          }, ARTWORK_FLIP_HALF_DURATION_SECONDS)
          .to(image, {
            scale: 1,
            rotationY: 0,
            duration: ARTWORK_FLIP_HALF_DURATION_SECONDS,
            ease: ARTWORK_FLIP_IN_EASE,
          }, ARTWORK_FLIP_HALF_DURATION_SECONDS);
      };

      preload.onload = () => {
        if (typeof preload.decode !== "function") {
          start();
          return;
        }
        void preload.decode().catch(() => undefined).then(start);
      };
      preload.src = targetSrc;
      if (preload.complete && preload.naturalWidth > 0) start();

      return () => {
        disposed = true;
        preload.onload = null;
        preload.onerror = null;
        preload.src = "";
        timeline?.kill();
      };
    });

    return () => media.revert();
  }, {
    dependencies: [targetSrc],
    revertOnUpdate: true,
    scope: imageRef,
  });

  return <img alt={alt} className={className} draggable={draggable} ref={imageRef} src={displayedSrc} />;
}
