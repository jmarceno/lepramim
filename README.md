# Lexaloud

Local, private Linux text-to-speech with a system-tray Qt UI and a Rust daemon.

Lexaloud reads selected text (or pasted content) using the Kokoro ONNX model, with preprocessing tuned for academic PDFs, citations, math symbols, and Markdown.

## Quick start

```bash
# Build the daemon and CLI
cargo build --release

# Download Kokoro model artifacts (~350 MB)
./target/release/lexaloud download-models

# Install user systemd unit, config, and desktop entry
./target/release/lexaloud setup

# Speak the current X11/Wayland selection
./target/release/lexaloud speak-selection
```

Models are cached under `~/.cache/lexaloud/models`. The daemon listens on a user-scoped Unix socket (mode `0700` directory) and exposes a small HTTP API for the Qt UI.

## AppImage

```bash
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
```

The CPU AppImage bundles ONNX Runtime, eSpeak-NG data, Qt plugins, and clipboard helpers. CUDA is supported only for source installs with `--backend cuda12`.

## CLI

| Command | Description |
|---------|-------------|
| `lexaloud daemon` | Run the TTS daemon (refuses start without verified models) |
| `lexaloud setup` | Write config, download models, enable user systemd unit |
| `lexaloud uninstall` | Remove unit and desktop entry; keeps config and model cache |
| `lexaloud download-models` | Fetch Kokoro ONNX + voices.bin (optional `--llm`) |
| `lexaloud speak-selection` | Preprocess and speak the current selection |
| `lexaloud status` | Show daemon health and player state |

Run `lexaloud --help` for the full command surface.

## Configuration

Default config is written to `~/.config/lexaloud/config.toml` on `setup`. Key sections:

- `[tts]` — voice, speed, CUDA preference (source installs only)
- `[preprocessor]` — citation stripping, abbreviation expansion, PDF cleanup
- `[sre_latex]` — optional Speech Rule Engine LaTeX (requires Node + SRE)
- `[normalizer]` — optional LLM glossary/normalizer (off by default; build with `--features llm` for llama.cpp)

## Development

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cmake --preset release && cmake --build --preset release --parallel
QT_QPA_PLATFORM=offscreen ctest --preset release
```

Opt-in real TTS smoke (requires downloaded models):

```bash
LEXALOUD_REAL_TTS=1 cargo test real_tts_smoke_opt_in -- --nocapture
```

## Architecture

- **`lexaloud`** — Rust binary: CLI, Axum UDS API, Kokoro TTS (ort), CPAL audio, preprocessor pipeline
- **`lexaloud-ui`** — Qt 6 tray/control/overlay; talks to the daemon over UDS only
- **`scripts/`** — native stage, AppImage, install, and smoke helpers

Unit tests use `FakeProvider` and `NullSink` seams; production uses real Kokoro + CPAL and fails closed when models or CUDA providers are missing.

## License

MIT — see `LICENSE`.
