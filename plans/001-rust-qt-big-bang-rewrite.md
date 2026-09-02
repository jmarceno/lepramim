# Plan 001: Rust core + Qt Widgets big-bang rewrite

> **Executor instructions:** Follow the phases in order and run each gate before
> continuing. The phases are checkpoints on one rewrite branch, not separately
> releasable migrations. If a STOP condition occurs, report it rather than
> adding a Python fallback, a second implementation, or an unplanned framework.
>
> **Drift check (run first):** `git diff --stat fcd9d01..HEAD -- .`
> Reconcile changes to any in-scope file against this plan before implementation.

**Status:** TODO  
**Priority:** P1  
**Effort:** L (multi-week)  
**Risk:** HIGH  
**Planned:** 2026-09-02  
**Baseline:** `fcd9d01`

## Outcome

Replace Lexaloud completely with a native Linux application whose core,
daemon, CLI, TTS, preprocessing, player, and platform services are written in
Rust and whose thin GUI is compiled C++ using Qt 6 Widgets.
The final repository and release contain no Python runtime, Python application
or test code, PySide, FastAPI, PyInstaller, wheels, virtual environments, or
legacy compatibility path. Native Qt is deliberately retained for the first
working release to reduce porting work and risk. Moving the GUI to Iced or
another toolkit is explicitly deferred until after that release works.

This is one big-bang replacement. Development happens on one rewrite branch and
may be temporarily incomplete there, but it is merged/released only as a fully
working product. There is no dual implementation, feature flag, fallback
binary, staged user migration, or retained legacy code.

"Working first, good later" is an explicit implementation rule:

- Reproduce behavior before improving architecture.
- Prefer direct modules, concrete types, cloning, channels, and small amounts
  of localized duplication over premature frameworks or abstractions.
- Introduce traits only at the three test seams that require them: TTS engine,
  audio sink, and platform commands.
- Do not block parity on cosmetic UI polish, speculative portability, perfect
  error taxonomies, zero-copy optimization, or idiomatic refactors.
- Safety-critical boundaries still remain fail-closed: model integrity, socket
  permissions, request limits, CUDA provider verification, and destructive
  uninstall behavior.

## Fixed decisions

| Area | Decision |
|---|---|
| Language | Stable Rust, edition 2024, committed `Cargo.lock` |
| Product shape | Rust `lexaloud` executable for CLI/daemon/core plus C++ `lexaloud-ui` for Qt windows/tray, shipped together |
| Async runtime/API | Tokio + Axum/Hyper over the existing Unix domain socket |
| GUI | Qt 6 Widgets in compiled C++, mechanically ported from the existing PySide6 behavior |
| Integration boundary | The Qt frontend talks to Rust through the owner-only UDS API; no Rust/C++ FFI in v1 |
| Tray | `QSystemTrayIcon` and `QMenu`, preserving the current tray implementation model |
| Audio | CPAL, with Linux host/device handling validated on PipeWire, PulseAudio, and ALSA |
| TTS | Kokoro-82M via a pinned `ort` 2.x release and the ONNX Runtime C API |
| Desktop services | `zbus` for MPRIS/KGlobalAccel and `ashpd` for portal/global-shortcut integration |
| CLI/config | Clap + Serde + TOML; preserve current commands, exit behavior, paths, and config keys unless called out below |
| Packaging | CPU AppImage plus a native/source CUDA 12 installation path; no PyPI release |
| Platforms | Linux only; Ubuntu 24.04 and Debian 13 tier 1, Fedora/Arch/Mint/Pop!_OS tier 2 |

The existing UDS boundary is the clean integration mechanism. Keep the Qt
frontend stateless except for presentation state: it reads daemon state and
invokes commands through the socket. Extend the owner-only API with narrowly
scoped config/setup/model operations where needed so TOML parsing, validation,
downloads, and product rules remain in Rust. Do not duplicate those rules in
C++, invoke the CLI as a subprocess for routine UI operations, or introduce
CXX-Qt/FFI merely to make the product one process. Two native executables inside
one AppImage are acceptable and reduce coupling.

The Qt code should be a direct behavioral port of the current PySide widgets,
not a redesign. Preserve the same widget hierarchy, labels, actions, timers,
visibility rules, and tray behavior wherever practical. This is specifically
chosen to maximize the probability and speed of the first working release.

CPU and CUDA are separate supported execution backends. Neither is a legacy
fallback. A user-selected CUDA configuration must fail loudly if CUDA is not
actually active; it must never silently run on CPU. The CPU AppImage explicitly
selects CPU.

## Current contracts to preserve

The replacement is allowed to change internals freely. These externally useful
contracts remain stable so the rewrite does not become a product redesign:

- Configuration: `~/.config/lexaloud/config.toml`.
- Models: `~/.cache/lexaloud/models/`, using the existing Kokoro model and
  voices artifacts and their recorded hashes.
- Runtime socket: `$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock`, inside a mode-0700
  directory.
- Service: the per-user systemd service continues to start the daemon.
- HTTP API: `GET /healthz`, `GET /state`, and `POST /speak`, `/pause`,
  `/resume`, `/toggle`, `/stop`, `/skip`, `/back` retain the JSON shapes,
  status codes, payload cap, and state semantics in `docs/http-api.md`.
- Playback: sentence-level synthesis, ready queue depth 3 by default,
  cooperative generation cancellation, approximately 100 ms playback blocks,
  180 ms inter-sentence silence, and current skip/back/replace/append behavior.
- Capture: Wayland primary selection first and X11 primary selection where
  applicable, with UTF-8-safe `capture.max_bytes` enforcement.
- Desktop behavior: tray controls, control window, onboarding/setup, overlay,
  global shortcuts, MPRIS, notifications, bug report, model download and
  verification, and uninstall.
- Preprocessing order: MathJax dedupe, Markdown cleanup, optional SRE LaTeX,
  math symbols, PDF repair, numeric and parenthetical citations, Latin and
  academic abbreviations, URLs/email, numbers, optional LLM normalization,
  then sentence segmentation.

The current architecture documents the essential boundary:

```text
CLI/UI -> HTTP over owner-only Unix socket -> player -> preprocessor -> Kokoro
                                                       -> audio sink
```

The existing implementation anchors are:

- `src/lexaloud/daemon.py:257-319` — API routes.
- `src/lexaloud/player.py:84-428` — state machine, bounded queue, cancellation,
  100 ms sub-chunks, and 180 ms padding.
- `src/lexaloud/preprocessor/__init__.py:47-132` — ordered text pipeline.
- `src/lexaloud/providers/kokoro.py:54-230` — model initialization, warmup,
  synthesis, and execution-provider verification.
- `src/lexaloud/selection.py` — compositor-aware capture and size limiting.
- `src/lexaloud/app.py`, `indicator.py`, `overlay.py`, and `gui_control/` — UI
  behavior and Qt widget structure to translate mechanically into C++.
- `src/lexaloud/mpris.py`, `shortcuts.py`, and
  `gui_control/keybindings.py` — desktop integration behavior.

## Why this target

The current frozen runtime is about 336 MB before the external model cache.
The measured Qt payload under `PySide6/Qt` is about 86 MB and is deliberately
retained and pruned for v1. The roughly 18 MB PySide/Shiboken binding layer,
58 MB NumPy payload, Python runtime, FastAPI stack, PyInstaller machinery, and
other Python packages are removed. A Qt-based native AppDir is expected to be
roughly 150-180 MB before external models; this is an estimate to validate, not
a completion claim. Cutting implementation time and first-release risk has
priority over saving the additional size that an immediate Iced rewrite might
remove.

The measured current CLI overhead is roughly 240-260 ms for `--help` and
363-372 ms for `status`. Native process startup and direct UDS I/O should remove
most of that overhead. The plan does not promise inference speedups merely from
changing languages: Kokoro remains an ONNX workload, so ORT configuration,
phonemization, audio buffering, and warmup are the relevant runtime variables.

Current upstream evidence checked for this decision:

- [Qt Linux deployment](https://doc.qt.io/qt-6/linux-deployment.html) — required
  Qt libraries/plugins and the supported CMake deployment flow.
- [`QSystemTrayIcon`](https://doc.qt.io/qt-6/qsystemtrayicon.html) — the existing
  tray abstraction and its Linux StatusNotifierItem/XEmbed support.
- [`ort`](https://ort.pyke.io/) and [ONNX Runtime CUDA execution provider](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)
  — Rust binding and official CUDA compatibility/runtime requirements.
- [`zbus`](https://docs.rs/zbus/latest/zbus/) and
  [`ashpd`](https://docs.rs/ashpd/latest/ashpd/) — D-Bus and desktop portal
  integrations.
- [`cpal`](https://docs.rs/cpal/latest/cpal/) — Rust audio I/O and Linux host
  support.

Do not use `kokoro-en` or another young all-in-one Kokoro crate as an opaque
production dependency. It may be read as a reference, but Lexaloud must own the
small adapter that loads the existing model/voices artifacts, produces the
expected phoneme/tensor inputs, calls `ort`, and validates the selected
execution provider. This isolates immature ecosystem risk without rebuilding
ONNX Runtime itself.

## Target repository layout

Keep Rust as one Cargo package and the thin Qt frontend as one CMake target.
Use Cargo for Rust and CMake/Ninja for C++; a repository script orchestrates
both instead of hiding either toolchain behind a complex mixed-language build:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
CMakeLists.txt
CMakePresets.json
src/
  main.rs
  cli.rs
  config.rs
  error.rs
  models.rs
  daemon.rs
  api.rs
  player.rs
  audio.rs
  privacy.rs
  tts/
    mod.rs
    kokoro.rs
    phonemize.rs
    voices.rs
  preprocessor/
    mod.rs
    abbreviations.rs
    citations.rs
    markdown.rs
    math.rs
    numbers.rs
    pdf.rs
    segmenter.rs
    sre.rs
    llm.rs
  platform/
    mod.rs
    selection.rs
    shortcuts.rs
    mpris.rs
    notifications.rs
    service.rs
ui/
  CMakeLists.txt
  include/
    api_client.hpp
    control_window.hpp
    onboarding.hpp
    overlay.hpp
    tray.hpp
  src/
    main.cpp
    api_client.cpp
    control_window.cpp
    onboarding.cpp
    overlay.cpp
    tray.cpp
  tests/
    test_api_client.cpp
    test_control_window.cpp
    test_tray_state.cpp
assets/
packaging/
scripts/
  build-native.sh
tests/
  fixtures/
  api.rs
  config.rs
  models.rs
  player.rs
  preprocessor.rs
  selection.rs
  setup.rs
```

## Execution plan

All work occurs on `codex/rust-qt-rewrite`. The phases below are checkpoints
inside that branch, not separately released migrations. CI work is continuous:
add the Rust jobs with Phase 2, add the Qt jobs with Phase 7, and replace the
legacy/release workflows completely in Phase 9. Do not postpone all build and
CI work until the end of the rewrite.

### Phase 1 — Freeze executable behavior as fixtures

**Goal:** Capture the behavior that the Rust replacement must reproduce before
the Python test oracle is removed.

1. Create machine-readable JSON fixtures under `tests/fixtures/` for:
   - every preprocessing test input, configuration, ordered intermediate where
     relevant, and final sentence list;
   - config defaults, valid examples, rejected values, and current path rules;
   - API requests/responses/status codes and state transitions;
   - player sequences for replace, append, pause, resume, stop, skip, back,
     concurrent control requests, failed synthesis, and bounded pause;
   - selection output, truncation, invalid UTF-8, stale clipboard, and
     compositor/tool detection;
   - model metadata, hashes, URLs, voice names, and backend selection;
   - CLI output/exit code cases, with only stable user-facing text asserted.
2. Export fixtures from the current tests with a one-off Python fixture tool.
   Commit the resulting JSON, not the generator, to avoid carrying migration
   machinery into the final tree.
3. Record baseline measurements in `spikes/rust-rewrite-baseline.md`:
   executable/AppDir sizes, `--help` and `status` p50/p95 over at least 30 runs,
   idle RSS, daemon cold start, Kokoro warmup, and synthesis real-time factor.
4. Run the current full non-real test suite and all available opt-in real tests
   on the reference machine. Record exact passes, skips, and environmental
   failures. Do not weaken or delete a behavior because its test is awkward.

**Gate:** Fixtures cover every current test module and the recorded Python suite
passes at baseline. Missing real hardware/software is documented rather than
reported as passed.

### Phase 2 — Establish the minimal native skeleton

**Goal:** Produce one Rust binary that can parse the existing config, expose
the full command surface, log safely, and start a placeholder owner-only UDS
server.

1. Add `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/main.rs`,
   `src/cli.rs`, `src/config.rs`, `src/error.rs`, and `src/privacy.rs`.
2. Pin exact dependency versions. Start with only: `tokio`, `axum`, `hyper`,
   `tower-http`, `clap`, `serde`, `serde_json`, `toml`, `tracing`,
   `tracing-subscriber`, `thiserror`, and `directories`/`xdg`.
3. Recreate all current subcommands and argument validation with Clap. Commands
   that are not implemented yet must return a clear non-zero error; they must
   never silently succeed.
4. Parse the existing config format and default values. Unknown keys remain
   tolerated or rejected exactly as today; secret/path values are redacted in
   logs.
5. Bind the existing socket location, ensure the parent is mode 0700, remove
   only a proven-stale socket owned by the current user, and refuse unsafe
   locations or foreign owners.
6. Add the initial Rust CI job immediately with locked `cargo fmt`, `check`,
   `clippy`, and model-free `test` gates. It may coexist with the Python
   characterization job on the rewrite branch, but the Python job is removed
   before the final gate.

**Tests:** Unit tests for CLI parsing, config fixtures, path selection, redaction,
and socket ownership/mode. `cargo test` must not need a model or display.

### Phase 3 — Make Kokoro speak natively

**Goal:** Get verified CPU audio first, then verified CUDA 12 audio, using the
same cached artifacts and without Python, NumPy, or subprocess inference.

1. Pin an `ort` release only after a compile-and-run spike confirms the exact
   ONNX Runtime version, model opset, Linux glibc floor, and CUDA 12/cuDNN
   combination. Prefer dynamic loading for the source/CUDA install and package
   the tested CPU ORT libraries in the AppImage.
2. Implement `tts/phonemize.rs` using native eSpeak NG bindings or the smallest
   maintained Rust wrapper that can reproduce the current language/phoneme
   output. Bundle the required eSpeak voice data and license notices.
3. Implement `tts/voices.rs` to read the existing voices artifact and select the
   same voice embedding slices. Do not convert user model caches unless the new
   format is demonstrably smaller and the installer downloads it atomically.
4. Implement `tts/kokoro.rs`: session options, input tensors, inference,
   waveform extraction, trim behavior, speed/language/voice handling, serialized
   warmup and synthesis, and cancellation checks before/after blocking calls.
5. Expose the actual active execution providers. When CUDA is selected, require
   the CUDA provider to be present after session creation and fail the daemon
   startup otherwise. Never convert that case to a warning or automatic CPU
   execution.
6. Compare Rust and Python output using shape, sample rate, duration tolerance,
   finite-sample checks, peak/RMS bounds, and intelligibility listening checks.
   Bit identity is not required unless both paths naturally produce it.

**Gate:** Real CPU synthesis works with the existing artifacts. CUDA 12 works
on the reference NVIDIA machine and reports CUDA as active. A deliberately
broken CUDA environment fails loudly. Warm CPU/GPU synthesis is no more than 5%
slower than the recorded baseline for the same text and hardware; otherwise
profile before proceeding.

### Phase 4 — Port the preprocessing pipeline

**Goal:** Match the committed fixture corpus using native Rust for the default
path.

1. Port each transformation as a small pure function and preserve the exact
   stage order from `src/lexaloud/preprocessor/__init__.py`.
2. Use `regex`, Unicode-aware string handling, and a Markdown parser; port the
   existing rules and exceptions rather than replacing them with an unrelated
   generic text-normalization crate.
3. Port sentence segmentation rules and their abbreviation protection directly
   from observed behavior. Do not call Python or ship `pysbd`.
4. Preserve SRE LaTeX as an optional bounded subprocess integration if SRE is
   enabled; validate timeout, output limits, and error behavior.
5. Preserve optional local-LLM normalization through a native llama.cpp binding
   or C API with strict model/download validation. Keep it after rules and
   before segmentation. This feature may be completed late in the branch, but
   the rewrite cannot merge without it if it is supported in the release being
   replaced.

**Gate:** Every preprocessing fixture matches exactly. Property tests cover no
panics, output bounds, Unicode validity, and idempotence where the current stage
promises it. Real SRE and LLM smoke tests pass when their opt-in dependencies
are installed.

### Phase 5 — Port the player and audio path

**Goal:** Reproduce queueing and control behavior with audible CPAL output.

1. Implement `AudioChunk { samples: Vec<f32>, sample_rate: u32 }` and a concrete
   CPAL sink. Convert/resample only when the selected device requires it.
2. Implement the player as one Tokio-owned state machine. Use a monotonic job
   ID/cancellation token, `VecDeque` pending/in-flight collections, and a bounded
   `mpsc` channel of depth 3 by default.
3. Run ORT synthesis with `spawn_blocking`; keep only one synthesis at a time
   until measurements prove concurrency is beneficial and safe.
4. Preserve replace/append, pause/resume/toggle, stop, skip, and back behavior,
   including requeueing in-flight sentences and surfacing all-synthesis-failed
   as `last_error`.
5. Write in approximately 100 ms blocks and insert 180 ms silence between
   sentences. Device loss or stream failure must update `last_error`, stop the
   job safely, and allow a later request to retry device creation.

**Gate:** Fixture-driven state tests pass under a fake engine and fake sink;
bounded-queue tests prove pause does not grow memory; a real-device smoke test
plays, pauses within the expected tolerance, skips, rewinds, stops, and recovers
after device reopen.

### Phase 6 — Complete UDS API and CLI vertical slice

**Goal:** Make the headless replacement fully usable before adding GUI work.

1. Implement the API routes and shared state in `api.rs`/`daemon.rs` using Axum.
2. Enforce body size before JSON parsing, reject nulls, preserve 200/400/413/422
   behavior, and serialize the existing state schema.
3. Implement a minimal UDS HTTP client for CLI commands; avoid a large general
   TLS stack because all control traffic is local.
4. Port selection capture and freshness checks. External `wl-paste`/`xclip`
   remain acceptable working-first dependencies because compositor-native
   selection protocols are not the rewrite's critical value.
5. Port all CLI commands, daemon auto-start/session behavior, stable exit codes,
   and helpful daemon-down errors.
6. Verify concurrent requests cannot interleave destructive player transitions.
7. Add typed GUI-support routes for configuration, model/setup status, and
   diagnostics before the C++ frontend needs them. Keep request/response types
   small and cover authorization-by-socket-permissions, validation, timeouts,
   and concurrent configuration updates. Document these additions alongside
   the preserved playback API.

**Gate:** The Rust CLI drives the Rust daemon through every endpoint; `curl
--unix-socket` examples work unchanged; API and CLI fixture tests pass; no
Python process is involved.

### Phase 7 — Mechanically port the PySide UI to compiled Qt Widgets

**Goal:** Get the existing UI behavior working with the least redesign and
least new framework risk.

1. Add a CMake-built `lexaloud-ui` executable using Qt 6 Core, Gui, Widgets,
   Network, DBus, and Svg only where actually required. Use C++20, CMake
   `AUTOMOC`, `AUTOUIC`, and `AUTORCC`; do not introduce QML, Qt Quick, CXX-Qt,
   a Rust Qt binding, or another GUI toolkit.
2. Implement `ui/src/api_client.cpp` with `QLocalSocket` and `QJsonDocument`.
   It must support the small HTTP-over-UDS contract, bounded response bodies,
   timeouts, malformed responses, disconnects, and daemon-down errors. Keep all
   product behavior in Rust.
3. Extend the Rust UDS API only as needed for GUI config reads/writes, model
   status/setup progress, diagnostics, and lifecycle operations. Reuse Rust
   validation and atomic writes. Do not parse/write TOML, verify models, manage
   systemd, or implement shortcut policy independently in C++.
4. Port `indicator.py` first using `QSystemTrayIcon`, `QMenu`, `QAction`, and
   `QTimer`, preserving its action order, enable/disable rules, tooltip/status,
   refresh behavior, autostart control, and quit semantics.
5. Port `gui_control/control_window.py` and related dialogs using the same Qt
   widget types and layouts. Translate the existing structure directly; do not
   redesign, create a new component system, or improve styling during parity.
6. Port onboarding from `app.py` and the overlay from `overlay.py`, preserving
   close/hide behavior, one-window ownership, focus behavior, timers,
   positioning, and action results.
7. Keep UI process state limited to widget state and request correlation. The
   daemon remains independently restartable and authoritative; reconnect after
   socket loss without restarting or embedding Rust.
8. Use existing icons/assets. Prune Qt deployment to the modules, QPA plugins,
   image plugins, platform themes, translations, and libraries actually used,
   but retain accessibility and GNOME/KDE/COSMIC/Wayland/X11 requirements.
9. Add QtTest coverage for API parsing, action-state mapping, settings forms,
   daemon-down/reconnect behavior, close-to-tray, and overlay state. Run widget
   tests with `QT_QPA_PLATFORM=offscreen`; keep a real desktop smoke matrix for
   behavior that offscreen Qt cannot prove.
10. Add the CMake/Ninja build and QtTest CI job as soon as `lexaloud-ui` exists,
    including one real UDS integration test against the Rust daemon. Do not wait
    for packaging to discover cross-language build failures.

**Gate:** Start, close-to-tray, reopen, settings save, voice/backend change,
playback controls, onboarding, overlay, reconnect, and quit work on GNOME
Wayland, KDE Plasma Wayland, COSMIC Wayland, and one X11 session. The UI process
loads Qt but no Python/PySide library; the Rust CLI and daemon load neither Qt
nor Python, verified with `ldd` and `/proc/$PID/maps`.

### Phase 8 — Port desktop and lifecycle integrations

**Goal:** Restore the behavior around the core app that users rely on daily.

1. Port MPRIS with `zbus`; emit state/property changes from the player channel
   and map play/pause/next/previous/stop exactly.
2. Port global-shortcut strategies in current priority order. Use `ashpd` for
   portal-capable desktops and `zbus` for KDE KGlobalAccel. Keep the proven
   command-based GNOME/XFCE/Cinnamon setup only where the desktop requires it.
3. Port notifications through the freedesktop notification D-Bus service.
4. Port systemd user-unit generation/enable/start checks, session detection,
   autostart behavior, and stale socket cleanup.
5. Port setup, model download with atomic rename/hash verification, bug-report
   redaction, and uninstall. Uninstall must enumerate exact owned paths and
   never recursively delete an unresolved environment-derived directory.

**Gate:** Integration tests use temporary XDG roots and mocked D-Bus/process
boundaries. Real smoke tests pass on each applicable desktop; unsupported
shortcut mechanisms yield an actionable message rather than silent success.

### Phase 9 — Replace the build system, CI, packaging, and release

**Goal:** Provide one reproducible native build path and make it the only path
accepted by CI or release automation.

#### 9.1 Build-system contract

1. Create root `CMakeLists.txt` and `CMakePresets.json` for the Qt target, with
   `dev`, `ci`, and `release` configure/build/test presets. Root CMake delegates
   to `ui/CMakeLists.txt`; it does not compile the Rust core through an opaque
   CMake wrapper. Fix their binary directories as `build/ui-dev`,
   `build/ui-ci`, and `build/ui-release` so scripts and artifact checks never
   guess output locations.
2. Create `scripts/build-native.sh` as the human/packaging orchestrator. It runs
   Cargo first and CMake second and accepts `--debug`, `--release`, and
   `--stage <absolute-path>`. It must stop at the first failed command and never
   install into the live user environment implicitly.
3. The release path is exactly:

   ```bash
   cargo build --locked --release
   cmake --preset release
   cmake --build --preset release --parallel
   ctest --preset release --output-on-failure
   ./scripts/build-native.sh --release --stage "$PWD/build/stage"
   ```

4. Staging produces a deterministic layout containing `bin/lexaloud`,
   `bin/lexaloud-ui`, desktop/service files, icons, model metadata, licenses,
   and only necessary shared libraries/assets. Fail if either executable or a
   declared runtime dependency is missing.
5. Set Rust release profile initially to `lto = "thin"`,
   `codegen-units = 1`, `strip = "symbols"`, and `panic = "abort"`. Compile C++
   release code with hidden default symbol visibility and strip only at staging;
   keep unstripped CI/debug artifacts available for diagnostics.
6. Pin the Rust toolchain, Cargo dependencies, container base digest, Qt major
   and minimum minor version, CMake minimum, compiler minimum, ONNX Runtime,
   and AppImage tools. Record all of them in the build documentation.

#### 9.2 Pull-request CI

Replace `.github/workflows/lint.yml` and `.github/workflows/test.yml`, mirror
the same gates in applicable Gitea workflows, and update branch-protection job
names. CI must contain these independently visible jobs:

1. **Rust quality** on Ubuntu 24.04:

   ```bash
   cargo fmt --all -- --check
   cargo check --locked --all-targets --all-features
   cargo clippy --locked --all-targets --all-features -- -D warnings
   ```

2. **Rust tests** on Ubuntu 24.04 with a cached Cargo registry/git directory and
   `target/`, keyed by `Cargo.lock` and compiler version:

   ```bash
   cargo test --locked --all-targets --all-features
   ```

   Unit/default integration tests must not download models, require a display,
   contact the network, or require CUDA/audio hardware. Real CPU/CUDA/audio
   tests remain explicit scheduled/manual jobs and report skips honestly.

3. **Qt build and tests** on Ubuntu 24.04. Install the documented Qt 6
   development packages, Ninja, CMake, and compiler; enforce C++ warnings as
   errors in the CI preset:

   ```bash
   clang-format --dry-run --Werror $(find ui -type f \
     \( -name '*.cpp' -o -name '*.hpp' \) -print)
   cmake --preset ci
   cmake --build --preset ci --parallel
   QT_QPA_PLATFORM=offscreen ctest --preset ci --output-on-failure
   ```

   At least one integration test starts the built Rust daemon with temporary
   XDG directories and exercises the real C++ UDS client; mocks alone are not
   sufficient for the language/process boundary.

4. **Native integration and AppImage** after the previous jobs pass, using the
   same container definition as releases:

   ```bash
   ./scripts/build-native.sh --release --stage "$PWD/build/stage"
   ./scripts/build-appimage.sh
   ./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
   ```

   The smoke script extracts the AppImage, runs both `--version` commands,
   starts the daemon under a temporary XDG runtime, checks `/healthz` and
   `/state` over UDS, launches `lexaloud-ui` offscreen long enough to prove it
   starts and connects, and exits cleanly. Upload the AppImage, staged-file
   manifest, `ldd` reports, and size report as CI artifacts.

5. **Dependency/license audit:** run `cargo deny check` against committed policy
   plus a generated inventory of bundled Qt, ONNX Runtime, eSpeak, and system
   libraries. Keep vulnerability auditing informational only if an advisory has
   been explicitly triaged with owner/expiry; license/source-notice failures are
   blocking before release.

6. Configure workflow concurrency/cancellation as today, minimum `contents:
   read` permissions for PR jobs, immutable/pinned third-party actions where
   practical, and no secrets in pull-request builds.

#### 9.3 Packaging and release

1. Rewrite `scripts/install.sh` for release binaries/source builds and preserve
   distro-specific runtime dependencies. It must not create a venv or require
   Python.
2. Rewrite `scripts/build-appimage.sh`, `build-appimage-fast.sh`,
   `packaging/Containerfile.builder`, and AppRun to use the native stage. Bundle
   the Rust binary, C++ Qt UI, CPU ONNX Runtime, eSpeak data, required Qt
   libraries/QPA plugins, graphics/audio dependencies, desktop file, icon, and
   licenses. Generate the Qt deployment set from actual linkage/plugins and
   then apply a reviewed allowlist; never copy the entire SDK blindly.
3. Replace `.github/workflows/release.yml` and `.gitea/workflows/release.yml`.
   A `v*.*.*` tag rebuilds from a clean checkout/container, reruns the complete
   gate, creates SHA-256 checksums and a staged-file/SBOM manifest, uploads the
   CPU AppImage and metadata, and creates release notes. Remove wheel, sdist,
   trusted PyPI publishing, and every Python build step.
4. Rewrite README, build/contribution docs, architecture, install,
   configuration, models, troubleshooting, security, CLI/API, and third-party
   licenses. Document the two-process boundary and exact native build commands;
   remove Python/PySide and immediate-Iced instructions.
5. Update `.gitignore`, `.pre-commit-config.yaml`, Dependabot configuration,
   pull-request template, branch-protection config, CODEOWNERS if paths changed,
   and release docs for Cargo/CMake/C++/Qt artifacts.

**Gate:** All PR jobs pass from a clean checkout; a clean Ubuntu 24.04 release
container produces the AppImage; the uploaded CI artifact passes the same smoke
script after download; CI and release contain no Python setup/build/publish
step. A clean supported machine launches the Qt UI and speaks with CPU, while
source/CUDA instructions activate and verify CUDA 12 on the reference host.

### Phase 10 — Delete the legacy implementation and run the big-bang gate

**Goal:** Leave exactly one implementation and prove the release artifact.

Delete, after their Rust replacements pass:

- `src/lexaloud/`, `src/lexaloud.egg-info/`, and every `.py`/`.pyi` file.
- All Python tests and helpers under `tests/`; keep only converted fixtures and
  Rust tests.
- `pyproject.toml`, Python lock/requirements files, PyInstaller spec/entrypoint,
  Python caches, and venv/build references.
- Python-only spike code and stale migration/handoff documents.
- Any packaging, CI, desktop entry, docs, badges, ignore rules, or dependency
  licenses that describe Python, PySide, FastAPI, NumPy, sounddevice,
  Pydantic, uvicorn, httpx, pytest, ruff, mypy, pip, wheels, or PyPI.

Do not keep deleted code in a `legacy/`, `old/`, vendor directory, disabled
module, archive, branch within the repo, generated patch, or commented block.
Git history is the only archive.

Run the complete gate from a clean checkout:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
clang-format --dry-run --Werror $(find ui -type f \
  \( -name '*.cpp' -o -name '*.hpp' \) -print)
cmake --preset release
cmake --build --preset release --parallel
QT_QPA_PLATFORM=offscreen ctest --preset release --output-on-failure
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
```

Then prove the legacy removal and artifact linkage:

```bash
test -z "$(find . -path ./.git -prune -o -type f \
  \( -name '*.py' -o -name '*.pyi' -o -name '*.spec' \) -print)"
! rg -n -i 'python|pyside|pyqt|fastapi|pyinstaller|numpy|sounddevice|pydantic|uvicorn|pytest|ruff|mypy|pip install|pypi' \
  --glob '!.git/**' --glob '!plans/**' .
ldd target/release/lexaloud | tee /tmp/lexaloud-core-ldd.txt
ldd build/ui-release/lexaloud-ui | tee /tmp/lexaloud-ui-ldd.txt
! rg -i 'python|PySide|PyQt|Qt[056]' /tmp/lexaloud-core-ldd.txt
! rg -i 'python|PySide|PyQt' /tmp/lexaloud-ui-ldd.txt
rg 'Qt6Core|Qt6Gui|Qt6Widgets' /tmp/lexaloud-ui-ldd.txt
```

Inspect the AppImage contents and a live process map too; `ldd` alone does not
show dynamically loaded libraries:

```bash
./scripts/build-appimage.sh
./build/appimage/*.AppImage --appimage-extract
find squashfs-root -type f | sort > /tmp/lexaloud-appimage-files.txt
! rg -i 'python|pyside|pyqt|pyinstaller|site-packages|\.py[co]$' \
  /tmp/lexaloud-appimage-files.txt
```

Start the extracted AppImage, inspect `/proc/$PID/maps`, exercise the UDS API,
run the UI/desktop matrix, and repeat the baseline benchmarks. Store the final
results in `spikes/rust-rewrite-results.md`.

## Final acceptance criteria

The rewrite is complete only when all of the following are true:

- The clean-checkout Rust gates and AppImage build pass.
- The clean-checkout CMake/Ninja build, clang-format check, QtTest suite,
  Rust-to-C++ UDS integration test, and AppImage smoke script pass in CI.
- Every Phase 1 behavior fixture has a passing Rust test or a documented,
  explicitly approved contract change.
- Real CPU and CUDA 12 Kokoro smoke tests pass; CUDA selection proves the CUDA
  provider is active and broken CUDA fails loudly.
- The CLI/API/player/preprocessor, tray, Qt Widgets windows, overlay, onboarding,
  shortcuts, MPRIS, notifications, setup, update/install documentation, model
  management, bug report, and uninstall work end to end.
- GNOME, KDE Plasma, COSMIC, and X11 validation evidence is recorded; tier-1
  distro tests pass and tier-2 gaps are stated accurately.
- The installed/release application needs no Python or PySide. Only
  `lexaloud-ui` loads the pruned native Qt runtime; the Rust CLI/daemon do not.
- The repository contains no legacy Python source/test/build path and no stale
  instructions that can install or release it.
- The native AppDir is at least 40% smaller than the measured 336 MB baseline
  before external model files (target: at most 200 MB). Any miss requires an
  itemized size report and explicit acceptance before merge; it must not delay
  a working build for speculative Iced work.
- `lexaloud --help` is at most 50 ms p95 and daemon-backed `lexaloud status` is
  at most 100 ms p95 on the same baseline machine over 30 warm runs.
- Warm synthesis is not more than 5% slower than baseline on the same model,
  backend, text, and hardware. Any claimed speedup is backed by the recorded
  benchmark rather than attributed to Rust by assumption.
- Idle daemon and UI RSS, cold start, warmup, first audio latency, artifact size,
  and synthesis real-time factor are reported before/after.

## Stop conditions

Stop the rewrite and report evidence—do not restore or silently retain Python—
if any of these occurs:

- The existing Kokoro model/voices cannot be reproduced through native ORT and
  eSpeak with intelligible, structurally valid output.
- A compatible, redistributable CPU ORT or CUDA 12 runtime cannot be pinned for
  supported systems.
- The C++ Qt client cannot reliably communicate with the Rust daemon over the
  existing UDS protocol after a minimal `QLocalSocket` reproducer and protocol
  tests.
- Required Qt modules/plugins cannot be redistributed under compatible terms
  or cannot be pruned below the explicitly accepted bundle-size boundary.
- A required desktop behavior is impossible on a tier-1 compositor without a
  user-visible contract change.
- A dependency or model license prevents distributing the required AppImage.
- The final tree passes tests only by weakening an existing security, size,
  cancellation, provider-verification, or destructive-operation guard.

At a stop condition, preserve the rewrite branch and measurements, state the
smallest blocked contract and available choices, and wait for a new product
decision. A hidden compatibility layer is not an allowed resolution.

## Deliberately deferred until after the working replacement

These are follow-up work, not merge blockers unless profiling reveals a defect:

- Splitting into a Cargo workspace or public internal crates.
- Generic provider/plugin architecture beyond the one Kokoro implementation.
- Replacing every external selection/desktop utility with direct protocols.
- Fine-grained domain errors, exhaustive tracing spans, and allocation tuning.
- UI redesign, animation, custom theming, accessibility polish beyond usable
  labels/focus/keyboard operation, and responsive-layout refinement.
- Replacing the compiled Qt Widgets frontend with Iced or another GUI toolkit.
- Combining the Rust and Qt processes through CXX-Qt/FFI or a single binary.
- Parallel inference, advanced caching, SIMD hand-tuning, or custom GPU kernels.
- Windows/macOS support.

The first post-rewrite pass should profile real bottlenecks, simplify the
working code, remove temporary duplication, and then adopt stricter Rust design
practices without changing the proven behavior.
