import { useEffect, useState } from "react";

const DEFAULT_ACCENT_COLOR = "#ffffff";
const SAMPLE_SIZE = 32;
const QUANTIZATION_STEP = 32;
const MIN_ALPHA = 128;
const MIN_LUMINANCE = 0.08;
const MAX_LUMINANCE = 0.92;
const ACCENT_COLOR_CACHE_LIMIT = 32;

type ColorBucket = {
  count: number;
  red: number;
  green: number;
  blue: number;
  saturation: number;
};

const accentColorCache = new Map<string, string>();

function cachedAccentColor(artworkId: string) {
  const color = accentColorCache.get(artworkId);
  if (!color) return null;
  accentColorCache.delete(artworkId);
  accentColorCache.set(artworkId, color);
  return color;
}

function cacheAccentColor(artworkId: string, color: string) {
  accentColorCache.delete(artworkId);
  accentColorCache.set(artworkId, color);
  while (accentColorCache.size > ACCENT_COLOR_CACHE_LIMIT) {
    const oldest = accentColorCache.keys().next().value;
    if (oldest === undefined) break;
    accentColorCache.delete(oldest);
  }
}

function rgbToHsl(red: number, green: number, blue: number) {
  const r = red / 255;
  const g = green / 255;
  const b = blue / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const lightness = (max + min) / 2;

  if (max === min) {
    return { hue: 0, saturation: 0, lightness };
  }

  const delta = max - min;
  const saturation = lightness > 0.5
    ? delta / (2 - max - min)
    : delta / (max + min);
  let hue: number;

  switch (max) {
    case r:
      hue = (g - b) / delta + (g < b ? 6 : 0);
      break;
    case g:
      hue = (b - r) / delta + 2;
      break;
    default:
      hue = (r - g) / delta + 4;
      break;
  }

  return { hue: hue / 6, saturation, lightness };
}

function hueToRgb(p: number, q: number, input: number) {
  let hue = input;
  if (hue < 0) hue += 1;
  if (hue > 1) hue -= 1;
  if (hue < 1 / 6) return p + (q - p) * 6 * hue;
  if (hue < 1 / 2) return q;
  if (hue < 2 / 3) return p + (q - p) * (2 / 3 - hue) * 6;
  return p;
}

function hslToHex(hue: number, saturation: number, lightness: number) {
  if (saturation === 0) {
    const value = Math.round(lightness * 255).toString(16).padStart(2, "0");
    return `#${value}${value}${value}`;
  }

  const q = lightness < 0.5
    ? lightness * (1 + saturation)
    : lightness + saturation - lightness * saturation;
  const p = 2 * lightness - q;
  const red = hueToRgb(p, q, hue + 1 / 3);
  const green = hueToRgb(p, q, hue);
  const blue = hueToRgb(p, q, hue - 1 / 3);
  return `#${[red, green, blue]
    .map((value) => Math.round(value * 255).toString(16).padStart(2, "0"))
    .join("")}`;
}

function extractAccentColor(image: HTMLImageElement): string | null {
  if (!image.naturalWidth || !image.naturalHeight) return null;

  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;

  let pixels: ImageData;
  try {
    context.clearRect(0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    pixels = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
  } catch {
    return null;
  }

  const buckets = new Map<string, ColorBucket>();
  for (let index = 0; index < pixels.data.length; index += 4) {
    const red = pixels.data[index];
    const green = pixels.data[index + 1];
    const blue = pixels.data[index + 2];
    const alpha = pixels.data[index + 3];
    if (alpha < MIN_ALPHA) continue;

    const luminance = (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255;
    if (luminance <= MIN_LUMINANCE || luminance >= MAX_LUMINANCE) continue;

    const { saturation } = rgbToHsl(red, green, blue);
    const bucketRed = Math.floor(red / QUANTIZATION_STEP);
    const bucketGreen = Math.floor(green / QUANTIZATION_STEP);
    const bucketBlue = Math.floor(blue / QUANTIZATION_STEP);
    const key = `${bucketRed}:${bucketGreen}:${bucketBlue}`;
    const bucket = buckets.get(key) ?? {
      count: 0,
      red: 0,
      green: 0,
      blue: 0,
      saturation: 0,
    };
    bucket.count += 1;
    bucket.red += red;
    bucket.green += green;
    bucket.blue += blue;
    bucket.saturation += saturation;
    buckets.set(key, bucket);
  }

  let selected: ColorBucket | null = null;
  let selectedScore = -1;
  for (const bucket of buckets.values()) {
    const averageSaturation = bucket.saturation / bucket.count;
    const score = bucket.count * (0.75 + averageSaturation * 0.25);
    if (score > selectedScore) {
      selected = bucket;
      selectedScore = score;
    }
  }
  if (!selected) return null;

  const red = selected.red / selected.count;
  const green = selected.green / selected.count;
  const blue = selected.blue / selected.count;
  const hsl = rgbToHsl(red, green, blue);
  const lightness = Math.min(0.72, Math.max(0.42, hsl.lightness));
  return hslToHex(hsl.hue, hsl.saturation, lightness);
}

export function useArtworkAccentColor(
  artworkId: string | null,
  artworkUrl: string | null,
) {
  const [accentColor, setAccentColor] = useState(DEFAULT_ACCENT_COLOR);

  useEffect(() => {
    let disposed = false;
    if (!artworkId) {
      setAccentColor(DEFAULT_ACCENT_COLOR);
      return () => {
        disposed = true;
      };
    }

    // 保留上一首颜色，直到当前封面异步加载完成，避免切歌时短暂闪回白色。
    if (!artworkUrl) {
      return () => {
        disposed = true;
      };
    }

    const cachedColor = cachedAccentColor(artworkId);
    if (cachedColor) {
      setAccentColor(cachedColor);
      return () => {
        disposed = true;
      };
    }

    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      if (disposed) return;
      const extractedColor = extractAccentColor(image) ?? DEFAULT_ACCENT_COLOR;
      cacheAccentColor(artworkId, extractedColor);
      setAccentColor(extractedColor);
    };
    image.onerror = () => {
      if (!disposed) setAccentColor(DEFAULT_ACCENT_COLOR);
    };
    image.src = artworkUrl;

    return () => {
      disposed = true;
      image.onload = null;
      image.onerror = null;
      image.src = "";
    };
  }, [artworkId, artworkUrl]);

  return accentColor;
}
