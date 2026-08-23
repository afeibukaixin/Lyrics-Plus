<div align="center">
  <h1>Lyrics Plus</h1>
  <p>面向 macOS 的同步歌词伴侣应用。</p>
  <p><strong>macOS 13+ · Apple Silicon 与 Intel · MIT License</strong></p>
  <p>简体中文 · <a href="README.md">English</a></p>
</div>

Lyrics Plus 是一款免费开源的 macOS 应用，会跟随音乐播放器，让歌词始终匹配当前歌曲与播放进度。2.0.0 重构了设置体验，并将桌面歌词、菜单栏歌词、歌词窗口和灵动岛统一为可配置的展示系统。

项目使用 Tauri 2、React、TypeScript 和 Rust 构建。

## 截图

![Lyrics Plus 样式设置、字体与快捷配色](docs/screenshots/lyrics-style-settings.png)

![Lyrics Plus 播放器设置与系统媒体筛选](docs/screenshots/player-and-lyrics.png)

![Lyrics Plus 桌面歌词模式](docs/screenshots/lyrics-modes.png)

## 按你的方式显示歌词

- **桌面歌词：** 始终置顶的歌词浮窗，支持跨桌面空间和多显示器显示，以及移动、缩放、锁定、鼠标穿透、停止播放时隐藏和位置复位。
- **菜单栏歌词：** 在 macOS 菜单栏显示当前歌词，可配置宽度、颜色和超长文本滚动行为。
- **歌词窗口：** 显示可滚动的完整歌词，并选择是否显示翻译和音译。
- **灵动岛歌词：** 将歌词吸附到屏幕顶部，悬停后显示控制工具栏，并可选择单排或双排歌词。

可以让每种模式继承统一的字体和配色，也可以单独调整。支持配置字体、字号、字重、对齐方式、透明度、背景、当前行和非当前行颜色、翻译、音译，以及可用时的逐字卡拉 OK 时间轴。

## 播放来源

- **Apple Music 和 Spotify：** 使用 macOS 专用监听，获取播放状态并支持自动化控制。
- **系统媒体：** 跟随 macOS 提供信息的兼容第三方媒体应用，并支持允许列表和排除列表筛选。
- **跟随播放器：** 通过随应用附带的辅助服务，让 Lyrics Plus 随指定播放器启动和退出。
- **启动控制：** 支持静默启动、菜单栏访问、隐藏 Dock 图标，以及智能或手动选择播放器。

Apple Music 和 Spotify 可能会请求“自动化”权限。播放器跟随功能可能需要在“登录项”设置中批准。播放控制和歌曲信息取决于各应用向 macOS 提供的能力。

## 歌词搜索与歌词库

- 并发搜索 LRCLIB、酷狗、QQ 音乐、网易云音乐、酷我音乐、AMLL TTML、咪咕音乐和 Musixmatch。
- Musixmatch 默认通过非官方 Desktop 接口自动获取匿名 Token；用户也可以填写 Desktop Token 或官方 Developer API Key。Token 保存在独立的本机凭据文件中，不随应用配置导出。
- 调整或禁用歌词源，选择智能或严格排序，测试歌词源状态，并设置自动匹配相似度。
- 根据歌名、歌手、专辑、时长和内容能力对候选结果排序，支持标题过滤和繁简中文匹配。
- 在可用时显示同步 LRC、翻译、音译和逐字时间轴。
- 支持导入本地歌词、维护离线歌词库、关联歌曲歌词和调整时间偏移。

在线歌词源是可选功能。启用后，歌曲名、歌手、专辑和时长等匹配信息会发送给所选第三方服务。

## 设置与维护

设置窗口分为样式、显示与交互、歌词、播放器、应用、调试日志、配置和关于与更新。

- 使用支持注释和尾随逗号的 JSONC 编辑配置。
- 出现问题时查看实时调试日志。
- 支持四种界面语言：English、简体中文、繁体中文（香港）和繁体中文（台湾）。
- 查看发布说明，带进度下载更新，并选择何时重启应用。

## 下载

前往 [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest) 下载最新版本：

- `aarch64`：Apple Silicon Mac。
- `x64`：Intel Mac。

首次打开前，请将 Lyrics Plus 移动到“应用程序”文件夹。当前版本使用 macOS ad-hoc 签名，暂未通过 Apple Developer ID 公证。如果 macOS 阻止首次启动，请前往“系统设置 → 隐私与安全性”并选择“仍要打开”。

## 全局快捷键

以下是默认快捷键。其他歌词展示模式的快捷键可以在“应用”设置中配置。

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

构建本地应用包：

```bash
pnpm tauri build
```

## 免责声明、版权与许可证

阅读完整的[中文免责声明](DISCLAIMER_ZH.md)。歌词及其他音乐相关内容的权利归相应权利人所有。Lyrics Plus 仅提供搜索、解析、缓存、导入和展示等软件功能，与 Apple Music、Spotify、歌词服务及任何权利人不存在隶属或背书关系。

应用代码按 [MIT License](LICENSE) 发布。MIT License 适用于项目代码，不适用于第三方歌词或音乐内容。

## 致谢

- [MxIris-LyricsX-Project/LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX)
- [ddddxxx/LyricsX](https://github.com/ddddxxx/LyricsX)
