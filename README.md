# Lexaloud

Local, private Linux text-to-speech with an Iced system-tray UI and a Rust daemon.

Lexaloud reads selected text using the Kokoro ONNX model, with preprocessing tuned for academic PDFs, citations, math symbols, and Markdown.

## Quick start

Double-click the AppImage. That is the whole install:

- missing speech models download automatically
- the speech daemon starts as a child of the app (no systemd unit)
- the tray icon appears

Quit from the tray to stop the daemon. Use **Start with desktop** in the tray if you want it at login (XDG autostart).

```bash
chmod +x Lexaloud-*-x86_64.AppImage
./Lexaloud-*-x86_64.AppImage
```

Models are cached under `~/.cache/lexaloud/models`. The daemon listens on a user-scoped Unix socket at `$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock`.

## Hotkeys (KDE / Plasma)

When the tray app is running, Lexaloud registers global shortcuts via KGlobalAccel:

| Shortcut | Action |
|----------|--------|
| **Meta+R** | Speak highlighted selection |
| **Meta+P** | Pause / resume playback |

Capture runs **in the app process** (no per-keypress AppImage spawn). On Wayland, Lexaloud snapshots the clipboard and sends a synthetic Ctrl+C when needed; on X11 it prefers PRIMARY selection.

## Build from source

```bash
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
```

After code changes, rebuild the stage before packaging:

```bash
rm -rf build/stage
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
```

CUDA is supported only for source installs with `--backend cuda12`. The CPU AppImage never bundles CUDA.

## CLI

Opening the AppImage with no arguments is the app. Subcommands talk to an **already running** daemon (start the AppImage or tray first):

| Command | Description |
|---------|-------------|
| `lexaloud` / `lexaloud app` | Download models if needed, start daemon, open tray |
| `lexaloud speak-selection` | Speak the current selection via CLI capture |
| `lexaloud speak-clipboard` | Speak the clipboard |
| `lexaloud status` | Show player state |
| `lexaloud daemon` | Run the speech daemon in the foreground |
| `lexaloud setup` | Write default config and XDG autostart entry |
| `lexaloud uninstall` | Remove autostart / leftover files; keeps config and models |

Run `lexaloud --help` for the full command surface.

## Configuration

Default config is written to `~/.config/lexaloud/config.toml` on first launch.

- `[provider]` — voice, speed, CUDA preference (source installs only)
- `[preprocessor]` — citation stripping, abbreviation expansion, PDF cleanup
- `[sre_latex]` — optional Speech Rule Engine LaTeX
- `[normalizer]` — optional LLM glossary (off by default; `--features llm`)

## Development

Install system deps (Debian/Ubuntu example):

```bash
sudo apt install libasound2-dev libssl-dev libdbus-1-dev \
  wl-clipboard xclip libfontconfig1-dev
```

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/build-native.sh --release --stage "$PWD/build/stage"
```

## License

MIT — see `LICENSE`.
