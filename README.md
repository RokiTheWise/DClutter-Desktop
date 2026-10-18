<div align="center">

<img src="assets/logo.png" width="160" alt="DClutter">

# DClutter for Windows

**Swipe to clean your files.**

[![CI](https://img.shields.io/github/actions/workflow/status/RokiTheWise/DClutter-Windows/ci.yml?branch=main&style=flat-square&logo=githubactions&logoColor=white&label=CI&labelColor=24292e)](https://github.com/RokiTheWise/DClutter-Windows/actions/workflows/ci.yml)

</div>

The Windows (and later Linux) version of [DClutter](https://github.com/RokiTheWise/DClutter),
built with [Tauri 2](https://tauri.app). It runs fully offline: the interface
ships inside the app and every file operation happens on your machine.

> **Status:** working, not yet released. Everything below runs from source;
> installers come with the first release.

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
