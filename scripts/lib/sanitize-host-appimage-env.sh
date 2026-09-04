# shellcheck shell=bash
# Source this from packaging / smoke / native build scripts.
#
# Cursor (and other IDEs shipped as AppImages) export APPIMAGE, APPDIR, ARGV0,
# OWD, and prepend their mount under LD_LIBRARY_PATH into every integrated
# terminal and agent shell. Nested AppImage tools (appimagetool, linuxdeploy)
# and our own binary-path resolution then get hijacked by the host IDE.
#
# Usage:
#   # shellcheck source=scripts/lib/sanitize-host-appimage-env.sh
#   source "$PROJECT_ROOT/scripts/lib/sanitize-host-appimage-env.sh"
#   sanitize_host_appimage_env

sanitize_host_appimage_env() {
  local host_appdir="${APPDIR:-}"
  local host_appimage="${APPIMAGE:-}"

  unset APPIMAGE APPDIR ARGV0 OWD 2>/dev/null || true

  # Drop LD_LIBRARY_PATH entries that live inside the host AppImage mount.
  if [[ -n "${LD_LIBRARY_PATH:-}" ]]; then
    local cleaned="" part rest="$LD_LIBRARY_PATH"
    while [[ -n "$rest" ]]; do
      part="${rest%%:*}"
      if [[ "$rest" == *:* ]]; then
        rest="${rest#*:}"
      else
        rest=""
      fi
      [[ -z "$part" ]] && continue
      if [[ -n "$host_appdir" ]]; then
        case "$part" in
          "$host_appdir"|"$host_appdir"/*) continue ;;
        esac
      fi
      case "$part" in
        /tmp/.mount_*) continue ;;
      esac
      cleaned="${cleaned:+$cleaned:}$part"
    done
    if [[ -n "$cleaned" ]]; then
      export LD_LIBRARY_PATH="$cleaned"
    else
      unset LD_LIBRARY_PATH
    fi
  fi

  if [[ -n "$host_appimage" || -n "$host_appdir" ]]; then
    echo "sanitize-host-appimage-env: cleared host AppImage env (was APPIMAGE=${host_appimage:-<unset>} APPDIR=${host_appdir:-<unset>})" >&2
  fi
}
