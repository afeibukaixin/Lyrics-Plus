import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";

export const updatePreviewMode = import.meta.env.DEV
  ? new URLSearchParams(window.location.search).get("update-preview")
  : null;
export const updatePreview = updatePreviewMode !== null;
export const updatePreviewReleaseNotes = `Lyrics Plus v2.1.0

中文

歌词来源与匹配
- 新增酷我、咪咕、Musixmatch 和 AMLL TTML 歌词源。
- 支持 Musixmatch 本地凭据管理和 AMLL Native API 地址配置。
- 完善歌词源管理、候选排序、匹配权重、自动匹配阈值和标题过滤。

歌词显示
- 优化逐字歌词动画与双语歌词的对齐效果。
- 改进长歌词换行，避免窗口缩放时出现截断。
- 增加多显示器环境下的窗口位置恢复能力。

设置与交互
- 重做应用内更新状态按钮和更新详情弹窗。
- 下载进度改用 SVG 圆环，并通过平滑动画同步百分比。
- 优化键盘导航、焦点状态以及减少动态效果模式。

稳定性
- 修复切换播放器后歌词状态偶尔未刷新的问题。
- 修复系统从睡眠恢复后媒体会话失联的问题。
- 改进网络超时、失败重试和错误提示。

English

- Added more lyric providers and matching options.
- Improved word-by-word animation and translated lyric alignment.
- Refined update progress, keyboard navigation, and error handling.
- Fixed several player reconnection and window restoration issues.`;

export function readCurrentVersion() {
  return getVersion();
}

export function checkForUpdate() {
  return check({ timeout: 15_000 });
}

export function downloadAndInstall(update: Update, onEvent: (event: DownloadEvent) => void) {
  return update.downloadAndInstall(onEvent);
}

export function closeUpdate(update: Update) {
  return update.close();
}

export function relaunchApplication() {
  return relaunch();
}

export function waitForPreviewStep(duration: number) {
  return new Promise<void>((resolve) => window.setTimeout(resolve, duration));
}
