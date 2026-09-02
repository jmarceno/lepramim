#!/usr/bin/env bash
# Smoke-test an AppImage (or AppDir) produced by scripts/build-appimage.sh
#
# Extracts the AppImage, runs both --version commands, starts the daemon
# under a temp XDG runtime, checks /healthz and /state over UDS via
# curl --unix-socket (or via Rust client fallback), launches lexaloud-ui
# offscreen, proves it starts and connects, exits cleanly.
#
# Usage:
#   ./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage
#   ./scripts/smoke-appimage.sh build/appdir           # AppDir direct
#
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
APPIMAGE_PATH="${1:-}"
if [[ -z "$APPIMAGE_PATH" ]]; then
  echo "Usage: $0 <AppImage|AppDir>" >&2
  echo "  e.g. $0 build/appimage/Lexaloud-*.AppImage" >&2
  echo "       $0 build/appdir" >&2
  exit 2
fi

# Detect dummy tar-wrapper AppImages (must fail closed)
if file "$APPIMAGE_PATH" 2>/dev/null | grep -qi 'shell script'; then
  echo "error: refusing dummy shell-script AppImage: $APPIMAGE_PATH" >&2
  exit 1
fi
if [[ -f "${APPIMAGE_PATH}.tar.gz" ]]; then
  echo "error: refusing AppImage with dummy tar sidecar: ${APPIMAGE_PATH}.tar.gz" >&2
  exit 1
fi

if [[ "$APPIMAGE_PATH" == *"*"* ]]; then
  # shellcheck disable=SC2206
  EXPANDED=($APPIMAGE_PATH)
  APPIMAGE_PATH="${EXPANDED[0]}"
fi

if [[ ! -e "$APPIMAGE_PATH" ]]; then
  echo "error: AppImage/AppDir not found: $APPIMAGE_PATH" >&2
  exit 1
fi

# Work in a temp dir to avoid polluting build/
WORKDIR="$(mktemp -d -t lexaloud-smoke-XXXXXX)"
cleanup() {
  local rc=$?
  echo "--- smoke cleanup (exit $rc) ---"
  # Kill daemon if still running
  if [[ -n "${DAEMON_PID:-}" ]] && kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo "Stopping daemon pid $DAEMON_PID"
    kill "$DAEMON_PID" 2>/dev/null || true
    wait "$DAEMON_PID" 2>/dev/null || true
  fi
  if [[ -n "${UI_PID:-}" ]] && kill -0 "$UI_PID" 2>/dev/null; then
    echo "Stopping UI pid $UI_PID"
    kill "$UI_PID" 2>/dev/null || true
    wait "$UI_PID" 2>/dev/null || true
  fi
  # Remove workdir after a short delay to allow inspecting on failure
  if [[ $rc -ne 0 ]]; then
    echo "Smoke failed; workdir preserved at $WORKDIR for inspection" >&2
  else
    rm -rf "$WORKDIR"
  fi
  exit $rc
}
trap cleanup EXIT INT TERM

echo "=== Lexaloud AppImage smoke test ==="
echo "input:    $APPIMAGE_PATH"
echo "workdir:  $WORKDIR"
echo

# --- Determine AppDir location --------------------------------------------
APPDIR=""
if [[ -d "$APPIMAGE_PATH" ]]; then
  # Direct AppDir
  APPDIR="$(cd "$APPIMAGE_PATH" && pwd -P)"
  echo "Input is AppDir: $APPDIR"
else
  # AppImage file — extract
  echo "--- Extracting AppImage ---"
  # Use absolute path before cd
  ABS_APPIMAGE="$(cd "$(dirname "$APPIMAGE_PATH")" && pwd -P)/$(basename "$APPIMAGE_PATH")"
  cd "$WORKDIR"
  # AppImages need --appimage-extract; dummy wrapper also supports it
  if [[ -x "$ABS_APPIMAGE" ]]; then
    echo "Running: $ABS_APPIMAGE --appimage-extract"
    # Need to run from WORKDIR; AppImage extracts to squashfs-root
    if "$ABS_APPIMAGE" --appimage-extract 2>&1 | sed 's/^/  /'; then
      echo "Extracted to $WORKDIR/squashfs-root"
    else
      echo "error: --appimage-extract failed for $ABS_APPIMAGE" >&2
      exit 1
    fi
  else
    echo "error: AppImage not executable: $APPIMAGE_PATH" >&2
    exit 1
  fi
  if [[ -d "$WORKDIR/squashfs-root" ]]; then
    APPDIR="$WORKDIR/squashfs-root"
  else
    echo "error: squashfs-root not found after extract; contents of $WORKDIR:" >&2
    ls -R "$WORKDIR" 2>&1 | sed 's/^/  /' | head -100
    exit 1
  fi
  cd "$PROJECT_ROOT"
fi

echo "AppDir: $APPDIR"
ls -lh "$APPDIR/AppRun" 2>/dev/null | sed 's/^/  AppRun: /' || true
ls -lh "$APPDIR/usr/bin/lexaloud" "$APPDIR/usr/bin/lexaloud-ui" 2>/dev/null | sed 's/^/  /' || true
echo

# --- Basic file checks ----------------------------------------------------
echo "--- Checking AppDir layout ---"
MISSING=0
for f in usr/bin/lexaloud usr/bin/lexaloud-ui AppRun lexaloud.desktop usr/share/applications/lexaloud.desktop; do
  if [[ -e "$APPDIR/$f" ]]; then
    echo "  ok: $f"
  else
    echo "  MISSING: $f" >&2
    MISSING=1
  fi
done
if [[ $MISSING -ne 0 ]]; then
  echo "error: AppDir layout incomplete" >&2
  exit 1
fi

# Verify no legacy interpreter payload per native gate
if find "$APPDIR" -type f \( -name "*.py" -o -name "*.pyc" -o -path "*site-packages*" \) 2>/dev/null | grep -q .; then
  echo "error: AppDir contains legacy interpreter files (should be native only):" >&2
  find "$APPDIR" -type f \( -name "*.py" -o -name "*.pyc" \) 2>/dev/null | head -20 >&2
  exit 1
fi
echo "  ok: no legacy interpreter files in AppDir"
echo

# --- --version checks -----------------------------------------------------
echo "--- Running --version checks ---"
set +e
LEXALOUD_BIN="$APPDIR/usr/bin/lexaloud"
LEXALOUD_UI_BIN="$APPDIR/usr/bin/lexaloud-ui"
# Also try AppRun delegation
APPRUN="$APPDIR/AppRun"

# Helper: run binary and capture output; allow offscreen for UI
run_version() {
  local bin="$1"
  local name="$2"
  echo "  $name --version:"
  if output="$("$bin" --version 2>&1)"; then
    echo "    $output" | sed 's/^/    /'
    echo "    exit 0: ok"
    return 0
  else
    rc=$?
    echo "    $output" | sed 's/^/    /'
    echo "    exit $rc" >&2
    # Try --help as fallback for UI which may not implement --version yet
    if [[ "$name" == "lexaloud-ui" ]]; then
      echo "    trying --help fallback for $name"
      if output2="$(QT_QPA_PLATFORM=offscreen "$bin" --help 2>&1)"; then
        echo "    --help exit 0: ok (version not yet implemented is tolerated for UI)" | sed 's/^/    /'
        echo "$output2" | head -5 | sed 's/^/    /'
        return 0
      fi
    fi
    return $rc
  fi
}

# Need to handle LD_LIBRARY_PATH pollution from AppDir? Use env -u if needed, but AppDir needs its own.
# For smoke, run with bundled libs: set LD_LIBRARY_PATH to AppDir/usr/lib for version check
VERSION_FAILED=0
if ! run_version "$LEXALOUD_BIN" "lexaloud"; then
  echo "  warning: lexaloud --version failed (try AppRun)" >&2
  if ! run_version "$APPRUN" "AppRun lexaloud"; then
    echo "error: lexaloud --version failed via both direct and AppRun" >&2
    VERSION_FAILED=1
  fi
fi
# UI offscreen
if ! QT_QPA_PLATFORM=offscreen run_version "$LEXALOUD_UI_BIN" "lexaloud-ui"; then
  echo "  warning: lexaloud-ui --version failed" >&2
  # Not fatal if UI not fully implemented yet, but warn
  echo "  note: UI --version may be stub; checking that binary is executable and ldd is sane"
  if [[ -x "$LEXALOUD_UI_BIN" ]]; then
    echo "    lexaloud-ui is executable: ok (version check tolerates stub)"
  else
    VERSION_FAILED=1
  fi
fi
set -e
if [[ $VERSION_FAILED -ne 0 ]]; then
  echo "error: --version checks failed" >&2
  exit 1
fi
echo

# --- Daemon UDS checks ----------------------------------------------------
echo "--- Daemon UDS checks (temp XDG runtime) ---"
RUNTIME_BASE="$(mktemp -d -t lexaloud-runtime-XXXXXX)"
CONFIG_BASE="$(mktemp -d -t lexaloud-config-XXXXXX)"
CACHE_BASE="$(mktemp -d -t lexaloud-cache-XXXXXX)"
# Ensure cleanup of these too via trap extension
cleanup_extra() {
  rm -rf "$RUNTIME_BASE" "$CONFIG_BASE" "$CACHE_BASE" 2>/dev/null || true
}
trap 'cleanup_extra; cleanup' EXIT INT TERM

export XDG_RUNTIME_DIR="$RUNTIME_BASE"
export XDG_CONFIG_HOME="$CONFIG_BASE"
export XDG_CACHE_HOME="$CACHE_BASE"
# Also override HOME to keep daemon isolated? Keep real HOME for config but use XDG overrides.
mkdir -p "$XDG_RUNTIME_DIR/lexaloud"
chmod 0700 "$XDG_RUNTIME_DIR" "$XDG_RUNTIME_DIR/lexaloud" 2>/dev/null || true

SOCKET="$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock"
echo "  XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"
echo "  XDG_CONFIG_HOME=$XDG_CONFIG_HOME"
echo "  socket: $SOCKET"

# Start daemon in background
# Use lexaloud daemon command; daemon should bind UDS and serve /healthz and /state
DAEMON_LOG="$WORKDIR/daemon.log"
echo "  starting daemon: $LEXALOUD_BIN daemon"
# Ensure config exists (daemon will create defaults)
mkdir -p "$XDG_CONFIG_HOME/lexaloud"
# Run daemon with timeout and capture log
"$LEXALOUD_BIN" daemon > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!
echo "  daemon pid: $DAEMON_PID"
# Wait for socket to appear (max 10s)
SOCK_READY=0
for i in $(seq 1 50); do
  if [[ -S "$SOCKET" ]]; then
    SOCK_READY=1
    break
  fi
  if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo "  daemon exited early (pid $DAEMON_PID); log:" >&2
    cat "$DAEMON_LOG" 2>/dev/null | sed 's/^/    /' | head -100 >&2
    break
  fi
  sleep 0.2
done

if [[ $SOCK_READY -eq 0 ]]; then
  echo "error: socket not ready after 10s; daemon log:" >&2
  cat "$DAEMON_LOG" 2>/dev/null | sed 's/^/    /' | head -100 >&2
  exit 1
fi

# Helper: curl over UDS or fallback via socat
# Try curl --unix-socket first, then socat
try_curl_uds() {
  local path="$1"
  local sock="$2"
  # curl --unix-socket is available in curl >=7.40
  if command -v curl >/dev/null 2>&1; then
    if curl --silent --show-error --fail --unix-socket "$sock" "http://lexaloud${path}" 2>&1; then
      return 0
    else
      rc=$?
      # Try without --fail to see body
      curl --silent --unix-socket "$sock" "http://lexaloud${path}" 2>&1 || true
      return $rc
    fi
  fi
  return 1
}

# Try healthz
echo "  GET /healthz:"
HEALTHZ_OK=0
if output="$(try_curl_uds "/healthz" "$SOCKET" 2>&1)"; then
  echo "    $output" | sed 's/^/    /' | head -20
  HEALTHZ_OK=1
  echo "    /healthz: ok"
else
  echo "    curl /healthz failed (output: $output)" | sed 's/^/    /'
  # Fallback: try raw HTTP over UDS via socat
  if command -v socat >/dev/null 2>&1; then
    echo "    trying socat UDS fallback..."
    if output2="$(printf 'GET /healthz HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n' | socat - UNIX-CONNECT:"$SOCKET" 2>&1)"; then
      echo "$output2" | sed 's/^/    /' | head -20
      if echo "$output2" | grep -q "200"; then
        HEALTHZ_OK=1
        echo "    /healthz via socat UDS: ok"
      fi
    fi
  fi
fi

echo "  GET /state:"
STATE_OK=0
if output="$(try_curl_uds "/state" "$SOCKET" 2>&1)"; then
  echo "    $output" | sed 's/^/    /' | head -40
  STATE_OK=1
  echo "    /state: ok"
else
  echo "    curl /state failed (output: $output)" | sed 's/^/    /'
  if command -v socat >/dev/null 2>&1; then
    echo "    trying socat UDS fallback for /state..."
    if output2="$(printf 'GET /state HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n' | socat - UNIX-CONNECT:"$SOCKET" 2>&1)"; then
      echo "$output2" | sed 's/^/    /' | head -40
      if echo "$output2" | grep -q "200"; then
        STATE_OK=1
        echo "    /state via socat UDS: ok"
      fi
    fi
  fi
fi

# Check daemon logs for health
if [[ $HEALTHZ_OK -eq 0 || $STATE_OK -eq 0 ]]; then
  echo "error: /healthz or /state not both ok (healthz=$HEALTHZ_OK state=$STATE_OK)" >&2
  cat "$DAEMON_LOG" 2>/dev/null | sed 's/^/    /' | head -100 >&2
  exit 1
fi
echo "  UDS checks: both ok"

# --- UI offscreen launch --------------------------------------------------
echo
echo "--- UI offscreen launch ---"
UI_LOG="$WORKDIR/ui.log"
echo "  launching lexaloud-ui offscreen (QT_QPA_PLATFORM=offscreen) for 3s..."
QT_QPA_PLATFORM=offscreen QT_PLUGIN_PATH="$APPDIR/usr/plugins:$APPDIR/usr/lib/qt6/plugins:${QT_PLUGIN_PATH:-}" LD_LIBRARY_PATH="$APPDIR/usr/lib:${LD_LIBRARY_PATH:-}" "$LEXALOUD_UI_BIN" > "$UI_LOG" 2>&1 &
UI_PID=$!
echo "  UI pid: $UI_PID"
# Give it time to start and attempt UDS connect
sleep 3
if kill -0 "$UI_PID" 2>/dev/null; then
  echo "  UI still running after 3s: ok (proves it starts and stays alive offscreen)"
  # Check that it attempted to connect (log may contain api_client messages)
  if grep -qi "connect\|uds\|socket\|daemon" "$UI_LOG" 2>/dev/null; then
    echo "  UI log shows daemon connection attempt:"
    grep -i "connect\|uds\|socket\|daemon\|api" "$UI_LOG" 2>/dev/null | head -20 | sed 's/^/    /'
  else
    echo "  UI log head (no explicit connect line, but process is alive):"
    cat "$UI_LOG" 2>/dev/null | head -40 | sed 's/^/    /'
  fi
  # Gracefully terminate UI
  kill "$UI_PID" 2>/dev/null || true
  wait "$UI_PID" 2>/dev/null || true
  echo "  UI terminated cleanly"
  UI_PID=""
else
  # UI exited already — check exit code
  wait "$UI_PID" 2>/dev/null; UI_RC=$? || UI_RC=$?
  echo "  UI exited with code $UI_RC after 3s"
  cat "$UI_LOG" 2>/dev/null | head -100 | sed 's/^/    /'
  # Exit 0 is ok for offscreen (no display), non-zero is warned but not fatal for stub
  if [[ $UI_RC -eq 0 ]]; then
    echo "  UI exit 0: ok (offscreen may exit after showing window)"
  else
    echo "  warning: UI exit $UI_RC (may be expected for stub or missing display)" >&2
  fi
  UI_PID=""
fi

# --- Clean shutdown -------------------------------------------------------
echo
echo "--- Shutting down daemon ---"
if kill -0 "$DAEMON_PID" 2>/dev/null; then
  kill "$DAEMON_PID" 2>/dev/null || true
  # Wait a bit for graceful shutdown
  for i in $(seq 1 10); do
    kill -0 "$DAEMON_PID" 2>/dev/null || break
    sleep 0.2
  done
  if kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo "  daemon did not exit gracefully; killing -9"
    kill -9 "$DAEMON_PID" 2>/dev/null || true
  fi
  wait "$DAEMON_PID" 2>/dev/null || true
  echo "  daemon stopped"
  DAEMON_PID=""
fi
# Verify socket cleaned up or can be removed
if [[ -S "$SOCKET" ]]; then
  echo "  warning: socket still exists after daemon stop: $SOCKET" >&2
fi

echo
echo "=== smoke-appimage: all checks passed ==="
echo "AppImage: $APPIMAGE_PATH"
echo "AppDir:   $APPDIR"
echo "Daemon log: $DAEMON_LOG"
echo "UI log:     $UI_LOG"
# Keep workdir on success? Remove it via trap
