import type {
  AppConfig,
  LyricsDisplayPreferences,
} from "../../../shared/types";

/** 将基础歌词样式按继承开关物化到各显示模式，保持本地预览与已保存配置一致。 */
export function materializeLyricsStyleInheritance(config: AppConfig): AppConfig {
  const base = config.lyrics.baseAppearance;
  const inheritance = config.lyrics.styleInheritance;
  const next: AppConfig = {
    ...config,
    lyrics: {
      ...config.lyrics,
      displays: {
        statusBar: { ...config.lyrics.displays.statusBar, appearance: { ...config.lyrics.displays.statusBar.appearance } },
        listWindow: { ...config.lyrics.displays.listWindow, appearance: { ...config.lyrics.displays.listWindow.appearance } },
        notch: { ...config.lyrics.displays.notch, appearance: { ...config.lyrics.displays.notch.appearance } },
      },
    },
    overlay: { ...config.overlay, appearance: { ...config.overlay.appearance } },
  };
  if (inheritance.desktop.inheritFontFamily) next.overlay.appearance.fontFamily = base.fontFamily;
  if (inheritance.desktop.inheritColors) Object.assign(next.overlay.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
    solidColor: base.backgroundColor,
  });
  if (inheritance.statusBar.inheritFontFamily) next.lyrics.displays.statusBar.appearance.fontFamily = base.fontFamily;
  if (inheritance.statusBar.inheritColors) Object.assign(next.lyrics.displays.statusBar.appearance, {
    textColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    highlightColor: base.activeColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
  });
  if (inheritance.listWindow.inheritFontFamily) next.lyrics.displays.listWindow.appearance.fontFamily = base.fontFamily;
  if (inheritance.listWindow.inheritColors) Object.assign(next.lyrics.displays.listWindow.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
  });
  if (inheritance.notch.inheritFontFamily) next.lyrics.displays.notch.appearance.fontFamily = base.fontFamily;
  if (inheritance.notch.inheritColors) Object.assign(next.lyrics.displays.notch.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
    backgroundColor: base.backgroundColor,
  });
  return next;
}

export function applyPendingNotchPreferences(
  config: AppConfig,
  pending: LyricsDisplayPreferences["notch"] | null,
): AppConfig {
  if (!pending) return config;
  return {
    ...config,
    lyrics: {
      ...config.lyrics,
      displays: { ...config.lyrics.displays, notch: pending },
    },
  };
}
