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
