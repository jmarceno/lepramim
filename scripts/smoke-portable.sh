#!/usr/bin/env bash
# Smoke-test a portable single-file bundle produced by scripts/build-portable.sh
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck source=scripts/lib/sanitize-host-appimage-env.sh
source "$PROJECT_ROOT/scripts/lib/sanitize-host-appimage-env.sh"
sanitize_host_appimage_env

RUN_PATH="${1:-}"
[[ -n "$RUN_PATH" ]] || { echo "Usage: $0 <Lepramim-*-portable.run>" >&2; exit 2; }

if [[ "$RUN_PATH" == *"*"* ]]; then
  # shellcheck disable=SC2206
  EXPANDED=($RUN_PATH)
  RUN_PATH="${EXPANDED[0]}"
fi
[[ -f "$RUN_PATH" ]] || { echo "error: not found: $RUN_PATH" >&2; exit 1; }
[[ -x "$RUN_PATH" ]] || { echo "error: not executable: $RUN_PATH" >&2; exit 1; }
grep -qa '__LEPRAMIM_PAYLOAD_FOLLOWS__' "$RUN_PATH" || { echo "error: not a portable bundle (payload marker missing)" >&2; exit 1; }

ABS_RUN="$(cd "$(dirname "$RUN_PATH")" && pwd -P)/$(basename "$RUN_PATH")"

WORKDIR="$(mktemp -d -t lepramim-portable-smoke-XXXXXX)"
cleanup() {
  local rc=$?
  [[ -n "${DAEMON_PID:-}" ]] && kill "$DAEMON_PID" 2>/dev/null || true
  [[ -n "${APP_PID:-}" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ $rc -eq 0 ]] && rm -rf "$WORKDIR"
  exit $rc
}
trap cleanup EXIT INT TERM

# Isolated cache forces a clean extraction on first launch.
CACHE_BASE="$(mktemp -d -t lepramim-cache-XXXXXX)"
RUNTIME_BASE="$(mktemp -d -t lepramim-runtime-XXXXXX)"
CONFIG_BASE="$(mktemp -d -t lepramim-config-XXXXXX)"
export XDG_CACHE_HOME="$CACHE_BASE"
export XDG_RUNTIME_DIR="$RUNTIME_BASE"
export XDG_CONFIG_HOME="$CONFIG_BASE"
mkdir -p "$XDG_RUNTIME_DIR/lepramim"
chmod 0700 "$XDG_RUNTIME_DIR"

echo "--- first launch (clean extraction) ---"
"$ABS_RUN" --version
EXTRACTED="$("$ABS_RUN" --portable-root)"
[[ -x "$EXTRACTED/bin/lepramim" ]] || { echo "error: extracted tree missing binary: $EXTRACTED" >&2; exit 1; }
[[ -f "$EXTRACTED/.lepramim-portable-marker" ]] || { echo "error: cache marker missing" >&2; exit 1; }
echo "extracted: $EXTRACTED"

echo "--- second launch (cached, must not re-extract) ---"
MARKER_BEFORE="$(cat "$EXTRACTED/.lepramim-portable-marker")"
MARKER_MTIME_BEFORE="$(stat -c %Y "$EXTRACTED/.lepramim-portable-marker")"
sleep 1.1
"$ABS_RUN" --version >/dev/null
MARKER_AFTER="$(cat "$EXTRACTED/.lepramim-portable-marker")"
[[ "$MARKER_BEFORE" == "$MARKER_AFTER" ]] || { echo "error: marker changed on cached launch" >&2; exit 1; }
# The marker is rewritten after every extraction, so an unchanged mtime
# proves the cached tree was reused (tar restores archived mtimes, which
# makes payload-file mtimes useless for this check).
[[ "$(stat -c %Y "$EXTRACTED/.lepramim-portable-marker")" == "$MARKER_MTIME_BEFORE" ]] || {
  echo "error: payload was re-extracted on cached launch" >&2
  exit 1
}
echo "cache reuse OK"

echo "--- ldd closure of extracted binary ---"
LDD_OUT="$(env -u LD_LIBRARY_PATH ldd "$EXTRACTED/bin/lepramim" 2>&1 || true)"
if echo "$LDD_OUT" | grep -q "not found"; then
  echo "$LDD_OUT" | grep "not found" >&2
  echo "error: extracted binary has missing shared libraries" >&2
  exit 1
fi
echo "ldd OK"

echo "--- daemon health ---"
SOCKET="$XDG_RUNTIME_DIR/lepramim/lepramim.sock"
"$ABS_RUN" daemon >"$WORKDIR/daemon.log" 2>&1 &
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
  echo "daemon healthz/state OK"
elif [[ -n "$DAEMON_PID" ]]; then
  echo "error: daemon socket never appeared" >&2
  cat "$WORKDIR/daemon.log" >&2 || true
  exit 1
fi

echo
echo "=== smoke-portable passed ==="
