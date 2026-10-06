# shellcheck shell=bash
# Source this from packaging / smoke / native build scripts.
#
# IDEs shipped as AppImages (e.g. Cursor) export APPIMAGE, APPDIR, ARGV0,
# OWD, and prepend their mount under LD_LIBRARY_PATH into every integrated
# terminal and agent shell, so builds and test runs would link or load the
# IDE's bundled libraries instead of the system / bundled ones.
#
# Usage:
#   # shellcheck source=scripts/lib/sanitize-host-env.sh
#   source "$PROJECT_ROOT/scripts/lib/sanitize-host-env.sh"
#   sanitize_host_env

sanitize_host_env() {
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
    echo "sanitize-host-env: cleared host IDE env (was APPIMAGE=${host_appimage:-<unset>} APPDIR=${host_appdir:-<unset>})" >&2
  fi
}
