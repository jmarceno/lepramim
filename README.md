# Lexaloud

> A local, private text-to-speech tool for Linux. Select text, press a
> hotkey, hear it read by a neural voice running on your own machine.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org/)
[![Qt](https://img.shields.io/badge/Qt-6.4+-green.svg)](https://www.qt.io/)

<!-- Record a demo with: ./scripts/record-demo.sh -->
<!-- ![demo](docs/demo.gif) -->

## How it works

1. **Select text** in any application
2. **Press a global hotkey** (e.g., `Ctrl+0`)
3. **Hear it spoken** sentence by sentence, with pause / skip / rewind controls

Lexaloud runs a small native daemon on your machine that synthesizes speech
using [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M), an
open-weights neural voice model. Nothing leaves your computer — no
cloud API, no account, no telemetry.

To hear what Kokoro sounds like before installing, try the
[live demo on Hugging Face](https://huggingface.co/spaces/hexgrad/Kokoro-TTS).

## Features

- **Global hotkey on any desktop** — works on GNOME, KDE Plasma,
  Sway, Hyprland, XFCE, Cinnamon, and any window manager that
  supports custom keybindings. GNOME is the primary tested path with
  integrated tray + hotkey UI; other desktops bind the same CLI
  commands manually. See [`docs/hotkeys/`](docs/hotkeys/).
- **MPRIS2 / media keys** — desktop media keys, GNOME's top-bar
  media indicator, KDE's media widget, Bluetooth headphone buttons,
  and `playerctl` all control Lexaloud playback with zero
  configuration.
- **Floating overlay** — an always-on-top sentence caption bar (off
  by default). Enable via `[advanced] overlay = true` in
  `config.toml` or the control window's Settings tab.
- **XDG GlobalShortcuts portal** — Wayland-native global hotkey
  binding on KDE Plasma 6+, Sway, and Hyprland via the
  `org.freedesktop.portal.GlobalShortcuts` portal. GNOME does not
  support this portal and continues using the gsettings path.
- **GPU-accelerated neural TTS** — Kokoro-82M via ONNX Runtime with
  CUDA. CPU fallback runs at ~10x real-time, which is fine for reading along.
- **Sentence-granularity streaming** with bounded backpressure and
  cooperative cancellation. Pause, skip, rewind, or stop mid-article
  without audio clipping.
- **54 built-in voices** — every bundled American, British, Spanish,
  French, Hindi, Italian, Japanese, Brazilian Portuguese, and Mandarin
  Chinese Kokoro v1.0 voice is available in the control window.
- **Qt tray indicator + control window** — visible on any desktop
  whose tray supports the StatusNotifierItem protocol (GNOME with the
  `ubuntu-appindicators` extension, KDE, XFCE, Budgie, etc.). Voice,
  language, speed, and hotkey settings are available straight from the
  tray menu. Built with Qt 6 — no system GTK/PyGObject
  packages needed. The CLI works without the tray on minimal setups.
- **Privacy-first** — see the [Privacy](#privacy) section.
- **Open-source** — MIT-licensed code, Apache-2.0-licensed model
  weights. See [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md).

## Requirements

| Requirement | Details |
|-------------|---------|
| **OS** | Linux only. Tier 1: Ubuntu 24.04, Debian 13. Tier 2: Fedora 41, Arch, Mint, Pop!_OS. Not supported: Windows, macOS. |
| **Init system** | systemd (for the `--user` daemon unit). Non-systemd distros (Artix, Void) can run `lexaloud daemon` manually. |
| **Toolchain (source build)** | Rust 1.85+ (via rustup), CMake 3.21+, Ninja, Qt 6.4+ dev packages, C++ compiler (g++ 13+ or clang++ 18+). AppImage users do not need the toolchain. |
| **GPU (optional)** | NVIDIA with CUDA 12-compatible driver. AMD ROCm and Intel Arc are **not yet supported** — the daemon falls back to CPU, which runs at ~10x real-time and is fine for reading along. |
| **Audio** | PipeWire, PulseAudio, or bare ALSA (via PortAudio/`libportaudio2`). Most desktop Linux distros ship PipeWire by default. |
| **Disk** | ~400 MB for model weights (downloaded once on first setup) |
| **Desktop (optional)** | GNOME for the integrated tray + hotkey UI. KDE, Sway, XFCE, Cinnamon, and others work via manual hotkey binding — see [`docs/hotkeys/`](docs/hotkeys/). The CLI works headless. |

## Install

### Ubuntu / Debian (Tier 1)

```bash
sudo apt install wl-clipboard xclip libportaudio2 libnotify-bin \
                 qt6-base-dev qt6-tools-dev libqt6svg6-dev

git clone https://github.com/Gustavjiversen01/lexaloud.git
cd lexaloud
./scripts/build-native.sh --release
sudo ./scripts/install.sh --from-source

lexaloud setup
systemctl --user daemon-reload
systemctl --user enable --now lexaloud.service
```

Then bind a hotkey — see [`docs/hotkeys/gnome.md`](docs/hotkeys/gnome.md)
or the walkthrough `lexaloud setup` prints.

Full walkthrough: [`docs/install/ubuntu-debian.md`](docs/install/ubuntu-debian.md)

### Fedora (Tier 2)

```bash
sudo dnf install wl-clipboard xclip portaudio libnotify \
                 qt6-qtbase-devel qt6-qtsvg-devel cmake ninja-build
```

Then the same `git clone` → `./scripts/build-native.sh --release` → `./scripts/install.sh --from-source` → `lexaloud setup` →
`systemctl` flow. Full walkthrough:
[`docs/install/fedora.md`](docs/install/fedora.md)

### Arch / Manjaro (Tier 2)

```bash
sudo pacman -S wl-clipboard xclip portaudio libnotify qt6-base qt6-svg cmake ninja
```

Then `git clone` → `./scripts/build-native.sh --release` → `./scripts/install.sh --from-source` → `lexaloud setup` → `systemctl`.
Full walkthrough: [`docs/install/arch.md`](docs/install/arch.md)

### Other distros

The installer auto-detects your distro via `/etc/os-release` and prints
the right package names if any are missing. For distros not in the table,
file a PR against [`docs/install/`](docs/install/).

### AppImage (CPU)

The release page also provides a CPU-only AppImage with native binaries,
Qt GUI, and application dependencies. It does not require a toolchain
or distribution-specific package names on the host:

```bash
chmod +x Lexaloud-*-x86_64.AppImage
./Lexaloud-*-x86_64.AppImage
```

Running it IS the app: a tray icon appears, the model is downloaded
automatically on first start (one-time ~350 MB with a progress window),
default shortcuts are created, and the daemon runs as part of the app.
An optional "Start with desktop" autostart entry replaces the need for a
systemd service. See [`docs/install/appimage.md`](docs/install/appimage.md)
for details, the host-session boundary, and the Wayland clipboard workflow.

### Native build from source

```bash
# Debug build (fast compile, for development)
cargo build --locked
cmake --preset dev
cmake --build --preset dev --parallel
ctest --preset dev

# Release build (optimized, for install / AppImage)
cargo build --locked --release
cmake --preset release
cmake --build --preset release --parallel
ctest --preset release

# Or use the orchestrator (Cargo + CMake + staging)
./scripts/build-native.sh --release
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
```

See `scripts/build-native.sh --help` for options.

### Wayland users: read this

On GNOME Wayland (the default on Ubuntu 24.04), `speak-selection` may
return empty for some apps (VS Code, Obsidian, Slack) because Electron
apps don't always publish to the PRIMARY selection. The reliable
workflow is:

1. **Ctrl+C** to copy the selection to the clipboard
2. Press your **`speak-clipboard` hotkey**

Both commands are in the CLI — bind whichever suits your workflow, or
bind both to different keys. Details in
[`docs/gotchas.md`](docs/gotchas.md).

## CLI

```
lexaloud speak-selection      # capture PRIMARY selection, speak it
lexaloud speak-clipboard      # capture CLIPBOARD (after Ctrl+C), speak it
lexaloud pause                # pause at the next sentence boundary
lexaloud resume
lexaloud toggle               # pause if speaking, resume if paused
lexaloud skip                 # skip the current sentence
lexaloud back                 # rewind one sentence
lexaloud stop                 # stop and clear the queue
lexaloud status               # daemon state as JSON
lexaloud download-models      # fetch model weights (~340 MB, once)
lexaloud setup                # first-time configuration walkthrough
lexaloud uninstall            # stop daemon and remove its user integration
lexaloud bug-report           # system diagnostics for filing issues
lexaloud daemon               # run the daemon (normally via systemd)
lexaloud-ui                   # run the Qt control UI (also lexaloud app)
```

Exit codes: 0 success, 1 error, 2 empty selection, 3 daemon down,
4 oversized payload, 5 capture tool missing/timeout.

Full reference: [`docs/cli-reference.md`](docs/cli-reference.md)

## Privacy

**Lexaloud performs no telemetry.** No text, metadata, or usage
statistics are transmitted anywhere. The only outbound network calls
are the one-time model downloads on first setup, fetched over HTTPS
from the [`kokoro-onnx`](https://github.com/thewh1teagle/kokoro-onnx)
GitHub releases page and SHA256-verified against pins in
[`src/models.rs`](src/models.rs).

The daemon listens on a **Unix domain socket** at
`$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` (mode 0700 enforced by
systemd's `RuntimeDirectoryMode=`). Only processes running as your user
can reach it. There is no open TCP port.

Selection text is never written to disk. Log entries that mention a
sentence replace the content with a SHA-1 fingerprint + length, so
`journalctl` never contains readable user text.

## Known limitations (v0.3.0)

- **NVIDIA only for GPU acceleration** — AMD ROCm and Intel Arc are
  not supported. CPU fallback works on any x86_64 Linux.
- **No karaoke word-level highlighting** — deferred (Kokoro doesn't
  expose word timings).
- **No browser extension** — deferred.
- **Sentence-level pause granularity** — the last ~100 ms of the
  current sub-chunk may play out after pressing pause.
- **GNOME Wayland primary-selection gaps** — some Electron apps don't
  publish to PRIMARY. Workaround: use `speak-clipboard` + Ctrl+C.
  See [`docs/gotchas.md`](docs/gotchas.md).
- **GlobalShortcuts portal not supported on GNOME** — GNOME 46/47
  does not implement the XDG GlobalShortcuts portal. GNOME users
  continue using the gsettings-based hotkey path.

Full list: [`ROADMAP.md`](ROADMAP.md)

## Architecture

A native Rust daemon (Tokio + Axum, `systemd --user`) owns the TTS provider and audio
sink. A thin CLI sends HTTP requests over the Unix socket. A Qt 6
tray indicator + control window connects via QLocalSocket and polls daemon state for visual feedback.
Two processes: `lexaloud` (daemon+CLI) and `lexaloud-ui` (Qt UI), communicating over UDS.

Component diagram + data-flow walkthrough:
[`docs/architecture.md`](docs/architecture.md). Design decisions:
[`docs/design-rationale.md`](docs/design-rationale.md).

## Tests

```bash
# Rust core
cargo fmt --check
cargo check --locked
cargo clippy --locked -- -D warnings
cargo test --locked

# Qt UI
cmake --preset ci
cmake --build --preset ci --parallel
ctest --preset ci --output-on-failure

# Or via orchestrator
./scripts/build-native.sh --release
```

No GPU or audio device required — tests use stub providers and null sinks.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). Pull requests should be
signed off with `git commit -s` (DCO).

Please read [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) before
participating.

Security vulnerabilities: use
[GitHub private vulnerability reporting](https://github.com/Gustavjiversen01/lexaloud/security/advisories/new)
rather than public issues. See [`SECURITY.md`](SECURITY.md).

## Acknowledgments

- **[Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)** by
  hexgrad — the open-weights neural TTS model.
- **[`kokoro-onnx`](https://github.com/thewh1teagle/kokoro-onnx)** by
  thewh1teagle — the ONNX wrapper.
- **[ONNX Runtime](https://onnxruntime.ai/)** + NVIDIA CUDA for
  GPU-accelerated inference.
- **[eSpeak NG](https://github.com/espeak-ng/espeak-ng)** for phonemization.
- The **GNOME** and **freedesktop.org** communities for libnotify and
  systemd-user, and **Qt** for the desktop UI.

Significant portions of this codebase were developed in collaboration
with [Claude](https://claude.ai) (Anthropic) via Claude Code. Code
review and final editorial decisions are the author's.

## License

MIT. See [`LICENSE`](LICENSE) for the full text and
[`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md) for runtime
dependency disclosures (the TTS stack includes GPL-3.0 dynamic
dependencies via eSpeak NG).
