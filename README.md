<div align="center">

<img src="assets/logo.png" width="160" alt="DClutter">

# DClutter for Windows

**Swipe to clean your files.**

</div>

The Windows (and later Linux) version of [DClutter](https://github.com/RokiTheWise/DClutter),
built with [Tauri 2](https://tauri.app). It runs fully offline: the interface
ships inside the app and every file operation happens on your machine.

> **Status:** early. The scoring engine is ported and tested; the triage
> interface is not built yet.

## Layout

| Path | What's there |
|---|---|
| `crates/dclutter-core` | Queue scoring, duplicate and extracted-archive detection. Pure Rust, no Windows APIs. |
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

Build the installers (`.exe` and `.msi`, in `target/release/bundle`):

```bash
npm run tauri build
```

## License

Apache 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
