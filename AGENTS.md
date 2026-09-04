Memory (Graphiti): at session end, register an episode via the graphiti-memory MCP server (add_memory, group_id lepramim) summarizing what changed and what was learned; whenever a task needs context from prior sessions, search Graphiti first (search_nodes / search_memory_facts). The Graphiti MCP server (Graphiti Agent Memory v1.29.1) answers at http://192.168.0.59:8000/mcp (remote MCP, Streamable HTTP). If the graphiti-memory tools are missing from the session tool catalog (e.g. after an app/MCP restart), the server itself is usually still up: shake hands directly — POST initialize (protocolVersion 2024-11-05), capture the mcp-session-id response header, send it back on every follow-up call (tools/list, tools/call). Do not treat a missing tool catalog entry as a dead server; probe the URL first.

## Cursor AppImage env hijack (recurring)

This repo is often developed inside **Cursor shipped as an AppImage**. Cursor exports host AppImage variables into every integrated terminal and agent shell:

- `APPIMAGE` → path to `cursor.appimage`
- `APPDIR` → `/tmp/.mount_cursor*`
- `ARGV0`, `OWD`
- `LD_LIBRARY_PATH` prepended with Cursor’s mount (`/tmp/.mount_cursor*/usr/lib…`)

That hijacks:

1. **Nested AppImage tools** (`appimagetool`, `linuxdeploy`) run with `APPIMAGE_EXTRACT_AND_RUN=1`
2. **Our AppDir variable / extract smoke tests** if anything reads bare `$APPDIR`
3. **`resolve_binary_path()`** if it trusts bare `$APPIMAGE` (would spawn Cursor instead of Lepramim)
4. **Linking / runtime** via Cursor’s `LD_LIBRARY_PATH` entries

### Required practice

- Before AppImage packaging, smoke, or native builds in this environment, call:

  ```bash
  source "$PROJECT_ROOT/scripts/lib/sanitize-host-appimage-env.sh"
  sanitize_host_appimage_env
  ```

  Already wired into `scripts/build-appimage.sh`, `scripts/build-native.sh`, and `scripts/smoke-appimage.sh`.

- Never trust bare `$APPIMAGE` / `$APPDIR` as “ours”. Prefer `$LEPRAMIM_APPIMAGE` / `$LEPRAMIM_APPDIR`. Rust path resolution must require the basename to contain `lepramim` (see `is_lepramim_appimage_path` in `src/platform/service.rs`).
- `packaging/appimage/AppRun` must ignore foreign `$APPIMAGE` values and strip `/tmp/.mount_*` from `LD_LIBRARY_PATH`.
- When debugging “wrong binary”, “weird .so”, or broken `appimagetool` under Cursor, check `env | grep -E 'APPIMAGE|APPDIR|LD_LIBRARY_PATH'` first and sanitize.

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
