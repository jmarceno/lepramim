# Third-party licenses

Lexaloud is distributed under the MIT license (see `LICENSE`). This file
discloses the licenses of the runtime dependencies that ship alongside a
working Lexaloud native installation.

## Disclosure: GPL-3.0 dynamic dependency (eSpeak NG)

Lexaloud's own source code is MIT-licensed. The runtime text-to-speech
stack may use **eSpeak NG** for phonemization:

- `eSpeak NG` — GPL-3.0-or-later (see https://github.com/espeak-ng/espeak-ng)
- `espeak-ng-data` — same license, contains pronunciation dictionaries.

These are **dynamic dependencies** (system library / data files). Lexaloud's Rust
code does not statically link against, embed, or copy GPL-3.0 source under a
single binary. However, anyone **redistributing** the installed runtime stack
(for example, bundling into an AppImage) should honor the GPL-3.0 terms for
these components. The AppImage bundles `espeak-ng-data` when available on the
build host; see `scripts/build-appimage.sh`.

If you want a fully permissive stack, you would need to replace
eSpeak NG with an MIT/BSD/Apache-licensed phonemizer. No such drop-in
replacement existed at the time of this release for Kokoro's phonemizer path.

## Key runtime dependencies (native)

| Component | License | Notes |
|-----------|---------|-------|
| `Qt 6` (Base, Tools, Svg, DBus, Network) | LGPLv3 / GPLv3 | Desktop UI, tray, control window, plugins. See https://www.qt.io/licensing |
| `ONNX Runtime` (CPU, optional CUDA EP) | MIT | Inference engine for Kokoro-82M. See https://github.com/microsoft/onnxruntime/blob/main/LICENSE |
| `eSpeak NG` + `espeak-ng-data` | GPL-3.0-or-later | Phonemizer (dynamic). See disclosure above |
| `PortAudio` (`libportaudio2`) | MIT | Audio output backend (CPAL uses ALSA/Pulse/PipeWire via PortAudio on Linux) |
| `CPAL` + `alsa` / `pulse` bindings | MIT / Apache-2.0 | Rust audio abstraction (depends on system ALSA/Pulse) |
| `Tokio`, `Axum`, `Hyper`, `Tower` | MIT | Async runtime + HTTP daemon over UDS |
| `Clap` | MIT / Apache-2.0 | CLI argument parsing |
| `Serde`, `serde_json`, `toml` | MIT / Apache-2.0 | Serialization / config |
| `pulldown-cmark` | MIT | Markdown parsing for preprocessor |
| `sha2`, `hex`, `regex`, `directories` | MIT / Apache-2.0 | Utilities |

Qt 6 is the largest third-party component in the AppImage. Only an allowlisted
subset of Qt plugins is bundled (see `scripts/build-appimage.sh`): platforms
(qxcb, qwayland, qoffscreen), xcbglintegrations, wayland, imageformats (svg, ico),
iconengines, generic evdev plugins, and platformthemes (gtk3, xdgdesktopportal if present).
Full Qt license texts are available at https://doc.qt.io/qt-6/licensing.html.

## Kokoro-82M model weights

The Kokoro-82M neural TTS model weights are developed by
[hexgrad/Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) and
distributed separately from this repository. The weights are licensed under
Apache-2.0 per the HuggingFace model card (verify at
https://huggingface.co/hexgrad/Kokoro-82M before redistributing).

Lexaloud downloads the ONNX-converted weights from the
[`kokoro-onnx`](https://github.com/thewh1teagle/kokoro-onnx) GitHub releases
on first run, SHA256-pinned in `src/models.rs`.

## NVIDIA CUDA runtime (optional, host-provided)

When using the CUDA backend, the host must provide NVIDIA's CUDA runtime
libraries (`libcuda.so`, `libcublas.so`, etc.) via the system driver install
(e.g., `nvidia-driver` + CUDA toolkit). Lexaloud does not bundle or redistribute
these libraries; the driver is installed by the user per NVIDIA's EULA.

See:
- https://docs.nvidia.com/cuda/eula/index.html
- https://developer.nvidia.com/cudnn-license

If CUDA is not present, the daemon falls back to CPU (verified via
`lexaloud status` `session_providers`).

## Optional runtime dependencies

### Speech Rule Engine (LaTeX-to-speech)

When a user opts in with `scripts/install.sh --with-math-speech`,
Lexaloud installs [`speech-rule-engine@4.1.3`](https://github.com/Speech-Rule-Engine/speech-rule-engine)
via `npm` into `~/.local/share/lexaloud/sre/node_modules/` and
links its `sre` binary into `~/.local/bin/`.

- License: **Apache-2.0**
- Installed by: user opt-in via `--with-math-speech` (never as part of
  the default install)
- Not part of the native AppImage; requires Node.js ≥18 on the host.

See [`docs/install/math-speech.md`](docs/install/math-speech.md) for
the walkthrough.

## Transitive Rust dependencies

A full list with licenses can be regenerated at any time:

```bash
cargo deny check licenses
# or
cargo tree --format "{p} {l}"
```

All crates present in `Cargo.lock` are MIT, Apache-2.0, BSD, ISC, or MPL-2.0
unless noted above and in `deny.toml` (GPL is `warn`, not `deny`, because
eSpeak NG is GPL but dynamically linked; release gate requires explicit
acceptance here).
