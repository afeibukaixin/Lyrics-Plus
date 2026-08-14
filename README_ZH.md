<div align="center">
  <h1>Lyrics Plus</h1>
  <p>面向 macOS 的桌面同步歌词应用。</p>
  <p><strong>macOS 13+ · Apple Silicon & Intel · MIT License</strong></p>
  <p>简体中文 · <a href="README.md">English</a></p>
</div>

> [!IMPORTANT]
> 这是一次 Vibe Coding 尝试。随着对话不断变长，反复的上下文丢失会让已经完成的实现倒退，项目也逐渐陷入不断修复新旧 Bug 的循环，持续消耗大量精力。现阶段开发暂缓；后续会在时间和精力允许时继续更新。

Lyrics Plus 会跟随 Apple Music、Spotify 和兼容的 macOS 媒体应用，让同步歌词始终匹配当前歌曲与播放进度。项目使用 Tauri 2、React、TypeScript 和 Rust 构建。

## 桌面歌词

![Lyrics Plus 五种桌面歌词模式](docs/screenshots/lyrics-modes.png)

支持透明、横排或竖排、单排或双排布局，并可调整字体、颜色、透明度、对齐、背景、长文本处理和逐词卡拉 OK 效果。

## 当前功能

- **播放器：** Apple Music、Spotify 和兼容的系统媒体应用；支持来源过滤、播放器跟随启动与静默启动。
- **歌词：** 并发搜索 LRCLIB、酷狗、QQ 音乐和网易云音乐；支持候选排序、来源管理、标题过滤与繁简匹配。
- **歌词内容：** 同步 LRC、翻译、音译、逐词时间轴、本地导入、离线歌词库与时间偏移调整。
- **桌面显示：** 浮窗置顶、跨桌面空间和多显示器显示，以及移动、缩放、锁定、鼠标穿透与位置复位。
- **应用能力：** 仅用于设置的主窗口、菜单栏、全局快捷键、快速切换歌词、JSONC 配置、实时日志、四种界面语言与主动确认更新。

## 下载

前往 [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest) 下载最新版本：

- `aarch64`：Apple Silicon Mac。
- `x64`：Intel Mac。

将 Lyrics Plus 移动到“应用程序”文件夹后打开。

> [!NOTE]
> 当前版本使用 macOS ad-hoc 签名，暂未通过 Apple Developer ID 公证。如果首次启动被系统阻止，请前往“系统设置 → 隐私与安全性”并选择“仍要打开”。

Apple Music 和 Spotify 可能请求“自动化”权限。系统媒体信息与播放控制能力取决于第三方应用实际提供的内容。

## 全局快捷键

| 快捷键 | 功能 |
|---|---|
| `⌘ ⇧ L` | 显示或隐藏桌面歌词 |
| `⌘ ⇧ U` | 锁定或解锁桌面歌词 |
| `⌘ ⇧ 0` | 复位并显示桌面歌词 |

## 本地开发

需要 Node.js、pnpm、Rust 和 Xcode Command Line Tools。

```bash
git clone https://github.com/afeibukaixin/Lyrics-Plus.git
cd Lyrics-Plus
pnpm install
pnpm tauri dev
```

检查与本地构建：

```bash
pnpm exec tsc --noEmit
cd src-tauri && cargo test
pnpm tauri build
```

## 联网与版权

启用在线歌词源时，歌曲名、歌手、专辑和时长等匹配信息会发送给所选第三方服务。相关内容、可用性、授权范围和数据处理规则不受本项目控制。

歌词权利归相应权利人所有。Lyrics Plus 仅提供搜索、解析、缓存、导入和展示功能，与 Apple Music、Spotify、歌词服务及权利人不存在隶属或背书关系。软件按 MIT License 以现状提供。

## 致谢

- [MxIris-LyricsX-Project/LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX)
- [ddddxxx/LyricsX](https://github.com/ddddxxx/LyricsX)

## License

[MIT](LICENSE)
