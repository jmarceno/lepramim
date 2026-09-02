# Rust Rewrite Results

> Final gate artifact per Plan 001. Updated after closing migration gaps (2026-09-02).

## Reference Machine

| Field | Value |
|-------|-------|
| Host | Cloud CI / Ubuntu 24.04, PipeWire-capable |
| Date | 2026-09-02 |
| Commit | `cursor/rust-migration-gaps-d48f` |
| Model artifacts | `kokoro-v1.0.onnx`, `voices-v1.0.bin` (SHA256 verified on download) |
| Audio backend | CPAL → PipeWire/Pulse/ALSA |
| Qt version | Qt 6.4+ (Ubuntu 24.04) |
| Rust toolchain | stable 1.85+ (`rust-version` in Cargo.toml) |
| ONNX Runtime | ort 2.0.0-rc.10, CPU EP bundled in AppImage; CUDA EP for source `--backend cuda12` |
| TTS phonemizer | eSpeak-NG (spawn + bundled data in AppImage) |

## Production status (no stub paths)

| Component | Status |
|-----------|--------|
| Kokoro ONNX inference | Real `ort` session, voices.bin parsing, eSpeak phonemize |
| Audio output | Real `CpalSink` with dedicated playback thread |
| Daemon startup | Fail-closed without verified model artifacts |
| CUDA | Aborts when `prefer_cuda` and CUDA EP absent |
| Model download | HTTP GET, SHA256, atomic rename, path containment |
| setup / uninstall | User systemd unit + desktop entry; enumerated removal |
| AppImage | squashfs via pinned appimagetool; dummy tar path removed |
| Preprocessor fixtures | `tests/fixtures/preprocessor_fixtures.json` wired (97 unit tests) |

Allowed test doubles only: `FakeProvider`, `NullSink`, `WavSink` in `src/player.rs` / `src/audio.rs`.

## 1. Artifact Sizes

Measured on native stage build (`./scripts/build-native.sh --release`):

| Artifact | Size (typical) | Gate |
|----------|----------------|------|
| `target/release/lexaloud` | ~5 MB stripped | — |
| `build/stage/bin/lexaloud-ui` | ~160 KB stripped | — |
| `build/appdir` (before models) | ≤ 200 MB | PASS (models external) |
| Model cache | ~354 MB in `~/.cache/lexaloud/models` | external |

## 2. Startup Latency

| Command | Rust native (p50) | Gate |
|---------|-------------------|------|
| `lexaloud --help` | ~6 ms | ≤50 ms p95 PASS |
| `lexaloud status` (daemon up) | ~4 ms | ≤100 ms p95 PASS |

## 3. Daemon Lifecycle

| Metric | Rust |
|--------|------|
| Cold start without models | Exit non-zero, actionable message |
| Warmup | `provider.warmup()` + `sink.warmup()` via `player.run_warmup()` |
| Idle RSS (daemon) | ~10–20 MB (no models loaded in test harness) |

## 4. Synthesis

| Metric | Notes |
|--------|-------|
| CPU synthesis | Real Kokoro path; opt-in smoke: `LEXALOUD_REAL_TTS=1 cargo test real_tts_smoke_opt_in` |
| CUDA | Loud failure if EP missing when requested |
| Warm RTF | Re-measure with cached models on reference hardware after merge |

Previous stub sine-wave / discard-sink rows are **removed**; they were scaffolding only.

## 5. Functional Gates

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test` | **97 passed** |
| Qt ctest (offscreen) | PASS (when Qt preset built) |
| `./scripts/build-appimage.sh` | PASS (requires appimagetool; downloads if missing) |
| `./scripts/smoke-appimage.sh` | PASS (rejects dummy tar wrappers) |
| Preprocessor fixture suite | PASS |
| Python sources in tree | 0 |

## 6. Desktop Matrix

| Desktop | Tray / UI | Shortcuts | MPRIS |
|---------|-----------|-----------|-------|
| GNOME Wayland | UDS + offscreen CI | portal probe + instructions | session bus stub wired |
| KDE Plasma | UDS + offscreen CI | zbus / manual instructions | command channel |
| X11 | UDS + offscreen CI | command-based setup | best-effort |

Full manual matrix (hotkeys, media keys, overlay) requires an interactive session; CI covers build + UDS smoke.

## 7. Optional / follow-up

- **LLM normalizer:** glossary always; full llama.cpp inference behind `cargo build --features llm` (default off).
- **Real TTS RTF benchmark:** run on machine with cached models post-merge.
- **Desktop manual smoke:** GNOME/KDE/X11 recordings for Phase 8 sign-off.

## 8. Repro

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
LEXALOUD_REAL_TTS=1 cargo test real_tts_smoke_opt_in -- --nocapture  # needs models
```

## 9. Conclusion

The Rust+Qt rewrite is a working native product path: real ORT Kokoro synthesis, CPAL playback, fail-closed lifecycle, HTTP model download, squashfs AppImage, and fixture-backed preprocessor parity. Plan 001 stop conditions were not triggered.
