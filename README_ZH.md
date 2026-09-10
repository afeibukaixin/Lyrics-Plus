<div align="center">
  <h1>Lyrics Plus</h1>
  <p>简洁的 macOS 同步歌词工具。</p>
  <p>
    <a href="https://github.com/afeibukaixin/Lyrics-Plus/releases/latest"><img src="https://img.shields.io/github/v/release/afeibukaixin/Lyrics-Plus?style=flat-square" alt="最新版本"></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/afeibukaixin/Lyrics-Plus?style=flat-square" alt="MIT License"></a>
    <a href="https://qm.qq.com/q/KDcSY7Yhii"><img src="https://img.shields.io/badge/QQ%20%E7%BE%A4-1045190390-12B7F5?style=flat-square&logo=qq&logoColor=white" alt="QQ 群 1045190390"></a>
  </p>
  <p>简体中文 · <a href="README.md">English</a></p>
</div>

Lyrics Plus 是一款免费开源的 macOS 应用，会跟随音乐播放器，让歌词与当前歌曲和播放进度保持同步。项目使用 Tauri 2、React、TypeScript 和 Rust 构建。

## 截图

<table>
  <tr>
    <td width="50%" align="center">
      <img src="docs/screenshots/lyrics-modes.png" alt="Lyrics Plus 歌词展示模式" width="100%">
      <br>
      <sub>歌词展示模式</sub>
    </td>
    <td width="50%" align="center">
      <img src="docs/screenshots/lyrics-plus-overview.png" alt="Lyrics Plus 歌词展示与样式设置" width="100%">
      <br>
      <sub>歌词展示与样式设置</sub>
    </td>
  </tr>
</table>

## 功能支持

| 功能 | 支持内容 |
|---|---|
| 歌词展示 | 桌面歌词、菜单栏歌词、歌词窗口和灵动岛歌词，各模式均可独立启用和配置 |
| 播放来源 | Apple Music、Spotify，以及通过 macOS 系统媒体接入的兼容播放器；支持自动或手动选择和应用过滤 |
| 歌词源 | 支持多个在线歌词源，可启用、停用、排序、测试并使用严格或智能优先模式 |
| 歌词搜索 | 自动与手动搜索、并发查询、候选预览和选择，以及基于歌曲信息和歌词能力的匹配排序 |
| 歌词内容 | 同步歌词、翻译、音译、逐字卡拉 OK 时间轴、本地导入和逐曲时间偏移 |
| 歌词库 | 管理本地与下载歌词，按歌曲、歌手和歌词浏览，并支持绑定、解绑、合并和清理歌词库内容 |
| 样式自定义 | 基础样式与按模式继承，支持字体、颜色、布局、横竖排、透明度、背景和长歌词显示方式 |
| 操作控制 | 可自定义全局快捷键，并支持窗口锁定、置顶、自动隐藏、恢复跟随和尺寸重置 |
| 应用体验 | 明亮、深色及跟随系统主题，多语言界面，以及自动检查并安装更新 |
| 系统支持 | macOS 13+、Apple Silicon 和 Intel |

在线歌词源是可选功能。启用后，歌曲名、歌手、专辑和时长等匹配信息会发送给所选第三方服务。

## 下载

前往 [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest) 下载最新版本：

- `aarch64`：Apple Silicon Mac。
- `x64`：Intel Mac。

> [!IMPORTANT]
> **首次运行提示：** 请先将 Lyrics Plus 移动到“应用程序”文件夹，再打开应用。
>
> 首次运行时，Lyrics Plus 会显示免责声明和使用确认提示，阅读并同意后才能继续使用。当前版本使用 macOS ad-hoc 签名，暂未通过 Apple Developer ID 公证。如果 macOS 显示安全授权提示或阻止首次启动，请前往“系统设置 → 隐私与安全性”并选择“仍要打开”。

## 使用说明

Apple Music 和 Spotify 可能需要“自动化”权限；播放器跟随功能可能需要在“登录项”设置中批准。播放控制和歌曲信息取决于对应应用向 macOS 提供的能力。

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
- [ChouChiu/Lyrics-Helper](https://github.com/ChouChiu/Lyrics-Helper)

## ❤️ 支持 Lyrics Plus

如果 Lyrics Plus 对你有帮助，欢迎通过爱发电、微信或支付宝支持项目的持续维护。感谢你的支持！

<table border="1" cellpadding="12" cellspacing="0">
  <tr>
    <td width="33%" align="center">
      <a href="https://afdian.com/a/afeibukaixin"><img src="https://img.shields.io/badge/%E7%88%B1%E5%8F%91%E7%94%B5-%E6%94%AF%E6%8C%81-946CE6?style=flat-square&logo=afdian&logoColor=white" alt="通过爱发电支持 Lyrics Plus"></a>
      <br>
      <sub>可以通过爱发电支持 Lyrics Plus 的持续维护。</sub>
    </td>
    <td width="33%" align="center">
      <strong>微信</strong>
      <br>
      <img src="docs/sponsors/wechat.jpg" alt="微信收款二维码" width="280">
    </td>
    <td width="33%" align="center">
      <strong>支付宝</strong>
      <br>
      <img src="docs/sponsors/alipay.jpg" alt="支付宝收款二维码" width="280">
    </td>
  </tr>
</table>

## 🌟 赞助者

感谢每一位支持 Lyrics Plus 的朋友。如果希望在这里公开展示，请在赞助备注中留下 GitHub 用户名或昵称；未注明时将默认匿名。
