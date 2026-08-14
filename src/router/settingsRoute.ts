const STORAGE_KEY = "lyrics-plus.last-settings-section";
const PRIMARY_SECTIONS = new Set(["style", "display", "lyrics", "player", "application", "about"]);

export function lastSettingsSection() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved && PRIMARY_SECTIONS.has(saved) ? saved : "style";
  } catch {
    return "style";
  }
}

export function rememberSettingsPath(pathname: string) {
  const section = pathname.match(/^\/settings\/([^/]+)$/)?.[1];
  if (!section || !PRIMARY_SECTIONS.has(section)) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, section);
  } catch {
    // 本地存储不可用时仍可正常使用设置页，只是不记忆上次分类。
  }
}
