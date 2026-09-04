#!/usr/bin/env bash
# Smoke-test an AppImage (or AppDir) produced by scripts/build-appimage.sh
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck source=scripts/lib/sanitize-host-appimage-env.sh
source "$PROJECT_ROOT/scripts/lib/sanitize-host-appimage-env.sh"
sanitize_host_appimage_env

APPIMAGE_PATH="${1:-}"
[[ -n "$APPIMAGE_PATH" ]] || { echo "Usage: $0 <AppImage|AppDir>" >&2; exit 2; }

if file "$APPIMAGE_PATH" 2>/dev/null | grep -qi 'shell script'; then
  echo "error: refusing dummy shell-script AppImage" >&2
  exit 1
fi

if [[ "$APPIMAGE_PATH" == *"*"* ]]; then
  # shellcheck disable=SC2206
  EXPANDED=($APPIMAGE_PATH)
  APPIMAGE_PATH="${EXPANDED[0]}"
fi
[[ -e "$APPIMAGE_PATH" ]] || { echo "error: not found: $APPIMAGE_PATH" >&2; exit 1; }

WORKDIR="$(mktemp -d -t lepramim-smoke-XXXXXX)"
cleanup() {
  local rc=$?
  [[ -n "${DAEMON_PID:-}" ]] && kill "$DAEMON_PID" 2>/dev/null || true
  [[ -n "${APP_PID:-}" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ $rc -eq 0 ]] && rm -rf "$WORKDIR"
  exit $rc
}
trap cleanup EXIT INT TERM

if [[ -d "$APPIMAGE_PATH" ]]; then
  APPDIR="$(cd "$APPIMAGE_PATH" && pwd -P)"
else
  ABS_APPIMAGE="$(cd "$(dirname "$APPIMAGE_PATH")" && pwd -P)/$(basename "$APPIMAGE_PATH")"
  cd "$WORKDIR"
  "$ABS_APPIMAGE" --appimage-extract >/dev/null
  APPDIR="$WORKDIR/squashfs-root"
  cd "$PROJECT_ROOT"
fi

[[ -x "$APPDIR/usr/bin/lepramim" ]] || { echo "error: missing usr/bin/lepramim" >&2; exit 1; }
[[ -x "$APPDIR/AppRun" ]] || { echo "error: missing AppRun" >&2; exit 1; }

LEPRAMIM_BIN="$APPDIR/usr/bin/lepramim"
"$LEPRAMIM_BIN" --version

RUNTIME_BASE="$(mktemp -d -t lepramim-runtime-XXXXXX)"
CONFIG_BASE="$(mktemp -d -t lepramim-config-XXXXXX)"
CACHE_BASE="$(mktemp -d -t lepramim-cache-XXXXXX)"
export XDG_RUNTIME_DIR="$RUNTIME_BASE"
export XDG_CONFIG_HOME="$CONFIG_BASE"
export XDG_CACHE_HOME="$CACHE_BASE"
mkdir -p "$XDG_RUNTIME_DIR/lepramim"
chmod 0700 "$XDG_RUNTIME_DIR"

SOCKET="$XDG_RUNTIME_DIR/lepramim/lepramim.sock"
"$LEPRAMIM_BIN" daemon >"$WORKDIR/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 50); do
  [[ -S "$SOCKET" ]] && break
  if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    if grep -q "missing artifact" "$WORKDIR/daemon.log"; then
      echo "Daemon startup correctly reported that models are not bundled."
      DAEMON_PID=""
      break
    fi
    cat "$WORKDIR/daemon.log"
    exit 1
  fi
  sleep 0.2
done

if [[ -S "$SOCKET" ]]; then
  curl --silent --fail --unix-socket "$SOCKET" http://lepramim/healthz >/dev/null
  curl --silent --fail --unix-socket "$SOCKET" http://lepramim/state >/dev/null
elif [[ -n "$DAEMON_PID" ]]; then
  echo "error: daemon socket not ready"
  cat "$WORKDIR/daemon.log"
  exit 1
fi

if command -v xvfb-run >/dev/null 2>&1; then
  echo "Launching lepramim app under xvfb for 3s..."
  timeout 3s xvfb-run -a env \
    XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
    XDG_CONFIG_HOME="$XDG_CONFIG_HOME" \
    XDG_CACHE_HOME="$CACHE_BASE" \
    "$LEPRAMIM_BIN" app >"$WORKDIR/app.log" 2>&1 || true
  echo "App smoke log head:"
  head -20 "$WORKDIR/app.log" 2>/dev/null || true
else
  echo "warning: xvfb-run not available; skipping GUI launch smoke"
fi

if [[ -n "$DAEMON_PID" ]]; then
  kill "$DAEMON_PID" 2>/dev/null || true
  wait "$DAEMON_PID" 2>/dev/null || true
fi
echo "=== smoke-appimage: passed ==="
