## IDE environment leakage (recurring)

This repo is often developed inside **Cursor shipped as an AppImage**. Cursor exports its own AppImage variables into every integrated terminal and agent shell:

- `APPIMAGE`, `APPDIR` (`/tmp/.mount_cursor*`), `ARGV0`, `OWD`
- `LD_LIBRARY_PATH` prepended with Cursor's mount (`/tmp/.mount_cursor*/usr/lib…`)

Those `LD_LIBRARY_PATH` entries hijack linking and runtime library loading.

### Required practice

- Before native or portable builds and smoke tests in this environment, call:

  ```bash
  source "$PROJECT_ROOT/scripts/lib/sanitize-host-env.sh"
  sanitize_host_env
  ```

  Already wired into `scripts/build-native.sh`, `scripts/build-portable.sh`, and `scripts/smoke-portable.sh`.

- The portable `.run` stub strips `/tmp/.mount_*` from `LD_LIBRARY_PATH` before exec'ing the app.
- When debugging "weird .so" under Cursor, check `env | grep -E 'APPIMAGE|APPDIR|LD_LIBRARY_PATH'` first and sanitize.

## Qt Quick debugging (hard-won)

Qt messages on this distro go to **journald, not stderr**. A fatal QML error once hid for an entire session. Always:

- Run UI sessions with `QT_FORCE_STDERR_LOGGING=1` (already set by `src/ui/mod.rs` unless overridden) so QML errors also hit the terminal.
- Check `journalctl --user -t lepramim --since '5 minutes ago'` for `QQmlApplicationEngine failed to load component` lines.
- `qmllint` passes files that still fail at runtime (e.g. default-property violations), so lint-clean ≠ loads.

Known traps:

1. **`QtObject` has no default property.** A bare `Connections {}` (or any object) child of a `QtObject` root fails the whole file with `Cannot assign to non-existent default property`. Bind it: `property Connections quitWatcher: Connections { ... }`. One such line once killed daemon spawn, all windows, and quit handling at once, because everything hangs off `bootstrap()`.
2. **cxx-qt keeps Rust snake_case for QML names.** `#[qproperty(bool, control_visible)]` is `controller.control_visible` in QML (NOT `controlVisible`) and its signal is `control_visibleChanged` (handler `onControl_visibleChanged`). Methods are camelCase only because each has an explicit `#[cxx_name]`. Verify against `target/debug/build/lepramim-*/out/cxxqtgen/src/ui/controller.cxx.cpp` (`*Changed` names) and the `Q_PROPERTY` lines in `controller.cxxqt.h`.
3. **Bare `Rectangle` reports implicit size 0.** A `Card` without `fillWidth`/`fillHeight` (or in a non-stretch slot) collapses to zero. `Card.qml` now forwards `body.implicitWidth/Height`; still pin `Layout.preferredHeight` where a card must keep height (see VoicePage Playback card). Verify visually: `QT_QPA_PLATFORM=xcb ./target/debug/lepramim app --control`, then `import -window $(xdotool search --name '^Lepramim$' | head -1) /tmp/opencode/ui.png`. Never drive the mouse on the user's session (no `xdotool click/mousemove`); screenshots are non-intrusive.
4. **`Qt.quit()` from QML is not a reliable exiter here** (proven no-op with no error). Tray Quit is handled Rust-side: `poll_input` → `quit_via_rust_shutdown` (stops daemon, reaps child, `process::exit(0)`). Keep the QML `Qt.quit()` call as belt-and-braces only.
5. **Startup watchdog.** `spawn_bootstrap_watchdog()` in `src/ui/mod.rs` exits(2) with log pointers if QML never consumes the bootstrap payload within 10 s, instead of running headless forever.
6. **Silent GUI-thread blocks.** `tick()`/`poll_input()` run on the Qt GUI thread: keep them to non-blocking drains. The daemon poll, downloads, and spawn already live on worker threads + channels. Never `thread::sleep` or do UDS I/O on invokables.
