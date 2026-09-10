<div align="center">
  <h1>Lyrics Plus</h1>
  <p>A simple, synchronized lyrics companion for macOS.</p>
  <p>
    <a href="https://github.com/afeibukaixin/Lyrics-Plus/releases/latest"><img src="https://img.shields.io/github/v/release/afeibukaixin/Lyrics-Plus?style=flat-square" alt="Latest release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/afeibukaixin/Lyrics-Plus?style=flat-square" alt="MIT License"></a>
    <a href="https://qm.qq.com/q/KDcSY7Yhii"><img src="https://img.shields.io/badge/QQ%20Group-1045190390-12B7F5?style=flat-square&logo=qq&logoColor=white" alt="QQ Group 1045190390"></a>
  </p>
  <p><a href="README_ZH.md">简体中文</a> · English</p>
</div>

Lyrics Plus is a free and open-source macOS app that follows your music player and keeps lyrics in sync with the current track and playback position. It is built with Tauri 2, React, TypeScript, and Rust.

## Screenshots

<table>
  <tr>
    <td width="50%" align="center">
      <img src="docs/screenshots/lyrics-modes.png" alt="Lyrics Plus lyrics display modes" width="100%">
      <br>
      <sub>Lyrics display modes</sub>
    </td>
    <td width="50%" align="center">
      <img src="docs/screenshots/lyrics-plus-overview.png" alt="Lyrics Plus lyrics display and style settings" width="100%">
      <br>
      <sub>Lyrics display and style settings</sub>
    </td>
  </tr>
</table>

## Feature Support

| Feature | Support |
|---|---|
| Lyrics display | Desktop Lyrics, Menu Bar Lyrics, Lyrics Window, and Dynamic Island Lyrics, each independently configurable |
| Playback sources | Apple Music, Spotify, and compatible apps through macOS System Media, with automatic or manual selection and app filtering |
| Lyrics sources | Multiple online providers with enable/disable controls, priority ordering, health checks, and strict or smart modes |
| Lyrics search | Automatic and manual search, concurrent queries, candidate previews and selection, and metadata- and capability-based ranking |
| Lyrics content | Synced lyrics, translations, romanization, word-level karaoke timing, local import, and per-track timing offsets |
| Lyrics library | Manage local and downloaded lyrics, browse songs, artists, and lyrics, and bind, unbind, merge, or clean up library items |
| Appearance | Shared styles with per-mode inheritance, including fonts, colors, layouts, orientation, opacity, backgrounds, and long-text behavior |
| Controls | Customizable global shortcuts, window locking, always-on-top, auto-hide, follow restoration, and window-size reset |
| App experience | Light, dark, and system themes, multilingual UI, and automatic update checks and installation |
| Compatibility | macOS 13+, Apple Silicon, and Intel |

Online lyrics providers are optional. When enabled, matching metadata such as title, artist, album, and duration is sent to the selected third-party service.

## Download

Download the latest build from [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest):

- `aarch64` for Apple Silicon Macs.
- `x64` for Intel Macs.

> [!IMPORTANT]
> **First launch:** Move Lyrics Plus to the Applications folder before opening it.
>
> Lyrics Plus shows a legal notice and asks you to accept it before continuing. Current builds use macOS ad-hoc signing and are not notarized with an Apple Developer ID. If macOS shows a security prompt or blocks the first launch, open System Settings → Privacy & Security and choose Open Anyway.

## Notes

Apple Music and Spotify may ask for Automation permission. Player following may require approval in Login Items settings. Playback controls and metadata depend on what each application exposes to macOS.

## Local Development

Requires Node.js, pnpm, Rust, and Xcode Command Line Tools.

```bash
git clone https://github.com/afeibukaixin/Lyrics-Plus.git
cd Lyrics-Plus
pnpm install
pnpm tauri dev
```

Build a local application bundle with:

```bash
pnpm tauri build
```

## Disclaimer, Copyright, and License

Read the full [English disclaimer](DISCLAIMER.md). Lyrics and other music-related content remain the property of their respective rightsholders. Lyrics Plus only provides software features for searching, parsing, caching, importing, and displaying that content; it is not affiliated with Apple Music, Spotify, any lyrics provider, or any rightsholder.

The application code is released under the [MIT License](LICENSE). The MIT License applies to the project code, not to third-party lyrics or music content.

## Acknowledgements

- [MxIris-LyricsX-Project/LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX)
- [ddddxxx/LyricsX](https://github.com/ddddxxx/LyricsX)
- [ChouChiu/Lyrics-Helper](https://github.com/ChouChiu/Lyrics-Helper)

## ❤️ Support Lyrics Plus

If Lyrics Plus has been helpful to you, consider supporting its continued development through AFDIAN, WeChat, or Alipay. Thank you for your support!

<table border="1" cellpadding="12" cellspacing="0">
  <tr>
    <td width="33%" align="center">
      <a href="https://afdian.com/a/afeibukaixin"><img src="https://img.shields.io/badge/AFDIAN-Support-946CE6?style=flat-square&logo=afdian&logoColor=white" alt="Support Lyrics Plus on AFDIAN"></a>
      <br>
      <sub>Support the continued development of Lyrics Plus through AFDIAN.</sub>
    </td>
    <td width="33%" align="center">
      <strong>WeChat</strong>
      <br>
      <img src="docs/sponsors/wechat.jpg" alt="WeChat payment QR code" width="280">
    </td>
    <td width="33%" align="center">
      <strong>Alipay</strong>
      <br>
      <img src="docs/sponsors/alipay.jpg" alt="Alipay payment QR code" width="280">
    </td>
  </tr>
</table>

## 🌟 Sponsors

Thank you to everyone who supports Lyrics Plus. If you would like to be listed here, leave your GitHub username or nickname in the payment note; otherwise, sponsorships will remain anonymous.
