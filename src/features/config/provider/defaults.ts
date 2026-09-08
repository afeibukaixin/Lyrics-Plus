import {
  defaultGlobalShortcuts,
  defaultLyricsBaseAppearance,
  defaultLyricsStyleInheritance,
  defaultListLyricsAppearance,
  defaultListLyricsLineOrder,
  defaultDesktopLyricsAppearance,
  defaultDesktopLyricsPresentation,
  defaultNotchLyricsAppearance,
  defaultNotchLyricsPresentation,
  defaultStatusBarLyricsAppearance,
  defaultStatusBarLyricsPresentation,
  type AppConfig,
} from "../../../shared/types";

const defaultTitleFilterKeywords = [
  "feat", "ft", "featuring", "主题曲", "片头曲", "片尾曲",
  "插曲", "电影", "电视剧", "动画", "游戏", "ost",
];

export const defaultConfig: AppConfig = {
  schemaVersion: 70,
  app: { theme: "dark", language: "system", playerSelection: "auto", systemMediaFilterMode: "allowlist", systemMediaApplications: [], playerFollowerApplication: null, hideDockIcon: false, hideMenuBarIcon: false, silentStartup: false, autoCheckUpdates: true, lyricsWindowsShowOnAllSpaces: false, shortcuts: defaultGlobalShortcuts },
  lyrics: {
    chineseConversion: "original",
    repairSimplifiedJapanese: false,
    providers: {
      mode: "smart",
      autoApplyThreshold: 60,
      autoSearchDebounceMs: 2000,
      maxCandidatesPerProvider: 20,
      preferCapabilities: true,
      capabilityPreferenceTolerance: 10,
      matchWeights: { title: 64, artist: 16, album: 5, duration: 10, version: 5 },
      normalizeChinese: true,
      titleFilterKeywords: defaultTitleFilterKeywords,
      amllBaseUrl: "https://api.amll.dev",
      providers: [
        { id: "lrclib", enabled: true },
        { id: "kugou", enabled: true },
        { id: "qqmusic", enabled: true },
        { id: "netease", enabled: true },
        { id: "amll_ttml", enabled: false },
        { id: "musixmatch", enabled: false },
        { id: "qishui", enabled: true },
      ],
    },
    displays: {
      desktop: {
        enabled: true,
        locked: false,
        hideWhenNotPlaying: false,
        presentation: defaultDesktopLyricsPresentation,
        appearance: defaultDesktopLyricsAppearance,
      },
      statusBar: { enabled: false, hideWhenNotPlaying: false, presentation: defaultStatusBarLyricsPresentation, appearance: defaultStatusBarLyricsAppearance },
      listWindow: { enabled: false, alwaysOnTop: false, locked: false, showTranslation: true, showRomanization: false, lineOrder: defaultListLyricsLineOrder, appearance: defaultListLyricsAppearance },
      notch: {
        enabled: false,
        hideWhenNotPlaying: false,
        monitorId: null,
        showLyrics: false,
        leftSlot: "artwork",
        rightSlot: "spectrum",
        presentation: defaultNotchLyricsPresentation,
        inlineLyricsOnNonNotch: true,
        appearance: defaultNotchLyricsAppearance,
      },
    },
    baseAppearance: defaultLyricsBaseAppearance,
    styleInheritance: defaultLyricsStyleInheritance,
  },
};
