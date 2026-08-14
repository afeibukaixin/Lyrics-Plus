<div align="center">
  <h1>Lyrics Plus</h1>
  <p>Synchronized desktop lyrics for macOS.</p>
  <p><strong>macOS 13+ · Apple Silicon & Intel · MIT License</strong></p>
  <p><a href="README_ZH.md">简体中文</a> · English</p>
</div>

> [!IMPORTANT]
> Lyrics Plus is a vibe-coding experiment. As the conversations grew, repeated context loss caused completed behavior to regress and turned development into an exhausting loop of fixing old and new bugs. Active development is paused for now; updates may resume when time and energy allow.

Lyrics Plus follows Apple Music, Spotify, and compatible macOS media apps, keeping synchronized lyrics aligned with the current song and playback position. It is built with Tauri 2, React, TypeScript, and Rust.

## Desktop Lyrics

![Five desktop lyrics modes in Lyrics Plus](docs/screenshots/lyrics-modes.png)

Choose transparent, horizontal or vertical, single-line or dual-line layouts. Adjust fonts, colors, opacity, alignment, backgrounds, long-text behavior, and word-level karaoke effects.

## Current Features

- **Players:** Apple Music, Spotify, and compatible System Media apps; source filtering, player-follow launch, and silent startup.
- **Lyrics:** concurrent LRCLIB, Kugou, QQMusic, and Netease search; candidate ranking, provider controls, title filtering, and Simplified/Traditional Chinese matching.
- **Content:** synchronized LRC, translations, romanization, word-level timing, local import, offline library, and timing offset adjustment.
- **Desktop:** always-on-top overlay across Spaces and displays, with move, resize, lock, click-through, and position reset.
- **Application:** a settings-only main window, menu bar controls, global shortcuts, quick lyrics switching, JSONC configuration, live debug logs, four UI languages, and opt-in updates.

## Download

Download the latest build from [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest):

- `aarch64` for Apple Silicon Macs.
- `x64` for Intel Macs.

Move Lyrics Plus to the Applications folder, then open it.

> [!NOTE]
> Current builds use macOS ad-hoc signing and are not notarized with an Apple Developer ID. If macOS blocks the first launch, open System Settings → Privacy & Security and choose Open Anyway.

Apple Music and Spotify may request Automation permission. System Media support and playback controls depend on what each third-party app exposes.

## Global Shortcuts

| Shortcut | Action |
|---|---|
| `⌘ ⇧ L` | Show or hide desktop lyrics |
| `⌘ ⇧ U` | Lock or unlock desktop lyrics |
| `⌘ ⇧ 0` | Reset and show desktop lyrics |

## Local Development

Requires Node.js, pnpm, Rust, and Xcode Command Line Tools.

```bash
git clone https://github.com/afeibukaixin/Lyrics-Plus.git
cd Lyrics-Plus
pnpm install
pnpm tauri dev
```

Checks and local build:

```bash
pnpm exec tsc --noEmit
cd src-tauri && cargo test
pnpm tauri build
```

## Network Access and Copyright

When an online lyrics source is enabled, matching metadata such as title, artist, album, and duration is sent to the selected third-party services. Their content, availability, licensing, and data practices are outside this project's control.

Lyrics remain the property of their respective rights holders. Lyrics Plus only searches, parses, caches, imports, and displays them; it is not affiliated with Apple Music, Spotify, any lyrics provider, or any rights holder. The software is provided as is under the MIT License.

## Acknowledgements

- [MxIris-LyricsX-Project/LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX)
- [ddddxxx/LyricsX](https://github.com/ddddxxx/LyricsX)

## License

[MIT](LICENSE)
