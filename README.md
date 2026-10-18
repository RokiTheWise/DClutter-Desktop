<div align="center">

<img src="assets/logo.png" width="160" alt="DClutter">

# DClutter for Windows

**Swipe to clean your files.**

[![CI](https://img.shields.io/github/actions/workflow/status/RokiTheWise/DClutter-Windows/ci.yml?branch=main&style=flat-square&logo=githubactions&logoColor=white&label=CI&labelColor=24292e)](https://github.com/RokiTheWise/DClutter-Windows/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RokiTheWise/DClutter-Windows?style=flat-square&logo=github&logoColor=white&labelColor=24292e&color=orange)](https://github.com/RokiTheWise/DClutter-Windows/releases)
[![Downloads](https://img.shields.io/github/downloads/RokiTheWise/DClutter-Windows/total.svg?style=flat-square&logo=github&logoColor=white&labelColor=24292e&color=blue)](https://github.com/RokiTheWise/DClutter-Windows/releases)
[![Platform](https://img.shields.io/badge/Windows-10%20%7C%2011-lightgrey.svg?style=flat-square&logo=windows&logoColor=white&labelColor=24292e)](#install)

**[⬇ Download DClutter for Windows 1.0.0](https://github.com/RokiTheWise/DClutter-Windows/releases/download/v1.0.0/DClutter_1.0.0_x64-setup.exe)**
&nbsp;·&nbsp; [all releases](https://github.com/RokiTheWise/DClutter-Windows/releases)

</div>

The Windows (and later Linux) version of [DClutter](https://github.com/RokiTheWise/DClutter),
built with [Tauri 2](https://tauri.app). It runs fully offline: the interface
ships inside the app and every file operation happens on your machine.

## What it does

DClutter opens on your Downloads folder and shows one file at a time, most
actionable first, with a preview and the context to decide:

- **Keep** leaves it where it is.
- **Stage** marks it for trash. Nothing is deleted until you review the list
  and confirm, and then it goes to the Recycle Bin, never straight to deletion.
- **Skip** puts it at the end of the queue.
- **File** moves it into one of up to three folders you choose, right away.
- **Rename** renames it in place.

Every decision can be undone, including moves and renames, and the session
picks up where you left off after you close the app. Previews are the ones
File Explorer would show, plus the first page of every PDF.

## Controls

| Key | Action |
|---|---|
| `→` / `←` | Keep / stage for trash (hold to repeat) |
| `Space` | Skip |
| `↑` | Open the destination shelf; then `←` `→` to choose, `Enter` to file, `Delete` to remove a folder, `Esc` to close |
| `1`–`3` | File into a destination |
| `↓` or `F2` | Rename |
| `Enter` | Open the file |
| `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | Undo / redo |
| `Ctrl+Enter` | Review staged files and trash them |
| `Ctrl+N` | Add a destination folder |

Right-click the card to open it, show it in File Explorer, rename it or copy
its name.

### Touchpad and mouse

| Gesture | Action |
|---|---|
| Two fingers right / left | Keep / stage for trash |
| Two fingers up, slide across, let go | File into the highlighted folder |
| Drag back down | Close the shelf without filing |
| Drag the card right / left | Keep / stage, with a mouse or a finger |

If swipes run the wrong way, your touchpad is set to reverse scrolling; turn
on **Reverse swipe direction** in Settings (`Ctrl+,`).

## Install

**[⬇ Download the installer](https://github.com/RokiTheWise/DClutter-Windows/releases/download/v1.0.0/DClutter_1.0.0_x64-setup.exe)**
for Windows 10 or 11 (64-bit), or the
[.msi](https://github.com/RokiTheWise/DClutter-Windows/releases/download/v1.0.0/DClutter_1.0.0_x64_en-US.msi)
if you deploy with Group Policy or another tool that wants one.

1. Run **DClutter_1.0.0_x64-setup.exe**.
2. **Windows will probably say "Windows protected your PC". This is expected.**
   Click **More info**, then **Run anyway**.
3. Follow the installer. DClutter installs for your account only and needs no
   administrator rights.

It opens normally from then on. Uninstall it from **Settings → Apps →
Installed apps** like any other app.

DClutter uses WebView2, which comes with Windows 11 and with an up-to-date
Windows 10. On the rare PC without it, the installer fetches it once; after
that, DClutter never goes online.

<details>
<summary>Why does Windows warn about it?</summary>

SmartScreen warns about any app that isn't signed with a code-signing
certificate, which costs money every year, and that hasn't been downloaded
often enough yet to build a reputation. This project doesn't have a
certificate yet. The warning isn't a claim that something is wrong with the
app.

What you can do instead of taking that on trust: the entire source is in this
repository, and you can build it yourself (see [Development](#development)),
which skips the warning because nothing you build locally is marked as
downloaded.

</details>

## Layout

| Path | What's there |
|---|---|
| `crates/dclutter-core` | Queue scoring, duplicate and extracted-archive detection, and the triage session with undo and save/restore. Pure Rust, no Windows APIs. |
| `crates/dclutter-platform` | Everything that touches Windows: reading a folder (with each download's source from its `Zone.Identifier` stream), moving, renaming and recycling files without ever overwriting, shell and PDF previews, and saved sessions and destinations. |
| `src-tauri` | The desktop app shell: windows, commands, filesystem access. |
| `src` | The interface (SvelteKit + TypeScript). Design tokens live in `src/lib/styles/tokens.css`. |
| `fixtures` | Scoring cases shared with the macOS app. See [fixtures/README.md](fixtures/README.md). |
| `docs/dclutter-design.md` | The product and visual design spec. |

## Development

You need Node 20+, Rust (stable) and the Microsoft C++ Build Tools. WebView2
ships with Windows 10 and 11.

```bash
npm install
```

```bash
npm run tauri dev
```

Run the scoring tests:

```bash
cargo test -p dclutter-core
```

Print a folder's queue from the command line, to check scores against real
files (the number is how many rows to show):

```bash
cargo run -p dclutter-platform --example rank -- "%USERPROFILE%\Downloads" 20
```

Build the installers (`.exe` and `.msi`, in `target/release/bundle`):

```bash
npm run tauri build
```

## License

Apache 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
