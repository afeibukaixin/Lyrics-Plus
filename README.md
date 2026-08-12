<div align="center">
  <h1>Lyrics Plus</h1>
  <p>A synchronized desktop lyrics app for macOS, designed to make lyrics feel at home on your desktop.</p>
  <p><strong>macOS 13+ · Apple Silicon & Intel · MIT License</strong></p>
  <p><a href="README_ZH.md">简体中文</a> · English</p>
</div>

Lyrics Plus follows the active song, playback state, and position in Apple Music, Spotify, or compatible third-party macOS media apps through the system media channel, then keeps the main window and desktop overlay in sync. Its current feature scope includes multi-provider lyrics search, translations and romanization, word-level timing, a local lyrics library, online artwork lookup, and a highly customizable desktop overlay.

Built with Tauri 2, React, TypeScript, and Rust, Lyrics Plus is focused on a native macOS desktop experience.

## Five Lyrics Modes

![Five desktop lyrics modes in Lyrics Plus](docs/screenshots/lyrics-modes.png)

Switch freely between a transparent background, horizontal single-line, horizontal dual-line, vertical single-line, and vertical dual-line layouts. Font size, colors, opacity, background, and alignment remain fully adjustable.

## Feature Support

| Category | Feature | Status |
|---|---|:---:|
| Players | Apple Music playback synchronization | ✅ |
| Players | Spotify playback synchronization | ✅ |
| Players | System Media synchronization for compatible third-party macOS media apps | ✅ |
| Players | System Media source selection with third-party app allowlist / blocklist | ✅ |
| Players | Selected player following and silent startup | ✅ |
| Players | Automatic song, playback state, and position updates | ✅ |
| Lyrics discovery | Concurrent multi-provider search and candidate ranking | ✅ |
| Lyrics discovery | Manual candidate selection and title-based re-search | ✅ |
| Lyrics discovery | Provider ordering, enablement, connection tests, title filtering, and Simplified/Traditional matching | ✅ |
| Lyrics discovery | Local LRC import and song association | ✅ |
| Lyrics management | Lyrics library, offline cache, and timing offset | ✅ |
| Lyrics content | Synchronized lyrics, translation, and romanization | ✅ |
| Lyrics content | Word-level timing | ✅ |
| Artwork | Player artwork priority with online artwork lookup | ✅ |
| Artwork | Cover Art Archive / iTunes sources, storefront selection, testing, ordering, and managed cache | ✅ |
| Desktop lyrics | Transparent, solid, and glass backgrounds | ✅ |
| Desktop lyrics | Horizontal, vertical, single-line, and dual-line layouts | ✅ |
| Desktop lyrics | Word sweep, bounce, and highlight effects | ✅ |
| Desktop lyrics | Font size, colors, opacity, and long-text handling | ✅ |
| Desktop overlay | Always on top, all Spaces, and multiple displays | ✅ |
| Desktop overlay | Move, resize, lock, click-through, and position reset | ✅ |
| System | Menu bar access, window state restoration, and global shortcuts | ✅ |
| Advanced settings | JSONC configuration, import/export, and live debug logs | ✅ |
| Languages | Simplified Chinese, Traditional Chinese (Taiwan and Hong Kong), and English UI | ✅ |

## Word-level Karaoke Effects

<table>
  <tr>
    <th width="50%">Word Sweep</th>
    <th width="50%">Word Bounce</th>
  </tr>
  <tr>
    <td><img src="docs/screenshots/karaoke-sweep.gif" alt="Word sweep karaoke effect in Lyrics Plus"></td>
    <td><img src="docs/screenshots/karaoke-bounce.gif" alt="Word bounce karaoke effect in Lyrics Plus"></td>
  </tr>
</table>

Lyrics with word-level timing can use sweep, bounce, or full-word highlight effects. Standard synchronized lyrics continue to advance accurately line by line.

## Player Integration

<table>
  <tr>
    <th width="50%">Spotify</th>
    <th width="50%">Apple Music</th>
  </tr>
  <tr>
    <td><img src="docs/screenshots/spotify-integration.png" alt="Lyrics Plus synchronized with Spotify"></td>
    <td><img src="docs/screenshots/apple-music-integration.png" alt="Lyrics Plus synchronized with Apple Music"></td>
  </tr>
</table>

Lyrics Plus reads the active player's track information and playback position to keep the main window and desktop overlay synchronized. Lyrics are retrieved independently through third-party lyrics services, so they do not depend on the player's built-in lyrics availability.

In addition to the dedicated Apple Music and Spotify integrations, System Media mode can follow third-party macOS media apps that expose Now Playing metadata through the system media channel. In Settings → Application, use an allowlist to control only selected apps or a blocklist to exclude browsers and other sources. New installations default to an empty allowlist, so no third-party source is controlled until added. Metadata and playback controls depend on what each media app exposes.

You can also choose a system media app to launch Lyrics Plus while it is running. Silent startup keeps that launch out of the way until you need the main window.

## Online Artwork Lookup

Player-provided artwork is always preferred. When enabled, Online Artwork Lookup checks third-party sources only when artwork is missing, its shortest side is below 256px, or a selected System Media app is configured to always try online artwork for visually blurred covers.

Cover Art Archive and iTunes can be enabled, tested, and reordered. iTunes can use the China, Taiwan, Hong Kong, or United States storefront; automatic mode follows the app language. Artwork is cached by album where possible, and Settings → Artwork lets you inspect the cache, change its folder, or clear only files managed by Lyrics Plus.

## Download and First Launch

Visit [GitHub Releases](https://github.com/afeibukaixin/Lyrics-Plus/releases/latest) to download the latest build for your Mac:

- Apple Silicon: Macs with M1, M2, M3, M4, or later Apple chips.
- Intel: Macs with an Intel processor.

Move Lyrics Plus to the Applications folder, then open it.

> [!IMPORTANT]
> Current builds use macOS ad-hoc signing and are not signed with an Apple Developer ID or notarized. If macOS blocks the first launch, open System Settings → Privacy & Security, verify the app source, and choose Open Anyway.

The dedicated Apple Music and Spotify integrations may trigger a macOS Automation permission request. Allow Lyrics Plus to control those apps under System Settings → Privacy & Security → Automation; this permission is separate from the System Media channel used by compatible third-party apps.

Online lyrics depend on third-party services. Search results and response times may vary with network conditions and provider availability.

## Updates

Installed release builds from v1.0.2 onward check for updates at launch by default. When a newer version is found, Lyrics Plus asks before downloading and installing it; updates are never installed silently. Manage automatic checks or check manually in Settings → About & Updates. Development builds do not check for updates.

## Global Shortcuts

| Shortcut | Action |
|---|---|
| `⌘ ⇧ L` | Show or hide desktop lyrics |
| `⌘ ⇧ U` | Lock or unlock desktop lyrics |
| `⌘ ⇧ 0` | Reset and show desktop lyrics |

## Local Development

You will need Node.js, pnpm, the Rust toolchain, and Xcode Command Line Tools.

```bash
git clone https://github.com/afeibukaixin/Lyrics-Plus.git
cd Lyrics-Plus
pnpm install
pnpm tauri dev
```

Build a local application bundle:

```bash
pnpm tauri build
```

Before submitting changes, run at least:

```bash
pnpm exec tsc --noEmit
cd src-tauri && cargo test
```

UI, localization, typography, and logging changes should follow the existing project conventions: [`i18n`](docs/i18n.md), [`typography`](docs/typography.md), and [`logging`](docs/logging.md). Issues and pull requests are welcome.

## Development Note

> [!TIP]
> Lyrics Plus is also a vibe-coding experiment driven by natural language and AI tools. The author leads product design, requirements, testing, and maintenance, while implementation is completed in collaboration with AI. By a fitting coincidence, the AI quota ran out just as the first feature-complete version came together.

## Free Software, Network Access, and Copyright

Lyrics Plus is an open-source project developed and maintained out of personal interest. The application is completely free through official channels, with no membership, subscription, activation code, or mandatory payment. Third parties may redistribute or sell copies under the MIT License, but any related download, installation, or packaging service is independent of this project and is not required to use the application.

When online lyrics providers or Online Artwork Lookup are enabled, metadata used for matching—including the song title, artist, album, and duration—is sent to the relevant third-party service. Artwork matching may contact Cover Art Archive, MusicBrainz, and iTunes. Content, availability, accuracy, licensing scope, and data practices are determined by each provider.

Lyrics, album artwork, and other music-related content remain the property of their respective rights holders. Lyrics Plus only provides search, parsing, caching, import, and display functionality; it does not own or grant rights to that content. This project is not affiliated with, endorsed by, or acting on behalf of Apple Music, Spotify, lyrics providers, content platforms, or rights holders.

Use the software only where permitted by applicable law and service terms. It is provided “as is,” without guarantees that third-party services will remain available or that every match will be accurate. Back up important local lyrics and configuration files.

## Acknowledgements

Lyrics Plus draws product direction, interaction ideas, and practical experience from these open-source projects:

- [MxIris-LyricsX-Project/LyricsX](https://github.com/MxIris-LyricsX-Project/LyricsX)
- [ddddxxx/LyricsX](https://github.com/ddddxxx/LyricsX)

Thank you to their authors and maintainers for their long-standing contributions to the macOS lyrics ecosystem.

## License

Released under the [MIT License](LICENSE).
