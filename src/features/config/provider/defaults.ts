import {
  defaultGlobalShortcuts,
  defaultLyricsBaseAppearance,
  defaultLyricsStyleInheritance,
  defaultListLyricsAppearance,
  defaultNotchLyricsAppearance,
  defaultOverlayStyle,
  defaultStatusBarLyricsAppearance,
  type AppConfig,
} from "../../../shared/types";

const defaultOverlayAppearance = (({
  horizontalMaxWidth: _horizontalMaxWidth,
  verticalMaxHeight: _verticalMaxHeight,
  ...appearance
}: typeof defaultOverlayStyle) => appearance)(defaultOverlayStyle);

const defaultTitleFilterKeywords = [
  "feat", "ft", "featuring", "主题曲", "片头曲", "片尾曲",
  "插曲", "电影", "电视剧", "动画", "游戏", "ost",
];

export const defaultConfig: AppConfig = {
  schemaVersion: 63,
  app: { theme: "dark", language: "system", playerSelection: "auto", systemMediaFilterMode: "allowlist", systemMediaApplications: [], playerFollowerApplication: null, hideDockIcon: false, hideMenuBarIcon: false, silentStartup: false, autoCheckUpdates: true, lyricsWindowsShowOnAllSpaces: false, shortcuts: defaultGlobalShortcuts },
  lyrics: {
    chineseConversion: "original",
    repairSimplifiedJapanese: false,
    providers: {
      mode: "smart",
      autoApplyThreshold: 60,
      autoApplyDurationGuardEnabled: true,
      autoApplyDurationToleranceSeconds: 15,
      autoSearchDebounceMs: 2000,
      preferCapabilities: true,
      capabilityPreferenceTolerance: 4,
      matchWeights: { title: 64, artist: 16, album: 16, duration: 4 },
      normalizeChinese: true,
      titleFilterKeywords: defaultTitleFilterKeywords,
      amllBaseUrl: "https://api.amll.dev",
      providers: [
        { id: "lrclib", enabled: true },
        { id: "kugou", enabled: true },
        { id: "qqmusic", enabled: true },
        { id: "netease", enabled: true },
        { id: "kuwo", enabled: true },
        { id: "amll_ttml", enabled: true },
        { id: "migu", enabled: true },
        { id: "musixmatch", enabled: true },
      ],
    },
    displays: {
      statusBar: { enabled: false, hideWhenNotPlaying: false, doubleLine: false, showTranslation: false, showRomanization: false, appearance: defaultStatusBarLyricsAppearance },
      listWindow: { enabled: false, alwaysOnTop: false, locked: false, showTranslation: true, showRomanization: false, appearance: defaultListLyricsAppearance },
      notch: {
        enabled: false,
        hideWhenNotPlaying: false,
        monitorId: null,
        showLyrics: false,
        leftSlot: "artwork",
        rightSlot: "spectrum",
        layout: "single",
        doubleLineMode: "rolling",
        showTranslation: false,
        showRomanization: false,
        inlineLyricsOnNonNotch: true,
        appearance: defaultNotchLyricsAppearance,
      },
    },
    baseAppearance: defaultLyricsBaseAppearance,
    styleInheritance: defaultLyricsStyleInheritance,
  },
  overlay: {
    visible: true,
    locked: false,
    hideWhenNotPlaying: false,
    appearance: defaultOverlayAppearance,
  },
};
