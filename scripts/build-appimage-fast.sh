#!/usr/bin/env bash
# Fast incremental AppImage build using the cached builder image.
# Reuses the Debian layer (apt packages) and the Cargo/Qt caches when
# possible, so only cargo/cmake + AppDir work runs on source changes.
# First run builds the builder image once (~3 min), subsequent runs skip it.
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILDER_TAG="lexaloud-builder:24.04"
CONTAINERFILE="$PROJECT_ROOT/packaging/Containerfile.builder"

restore_output_ownership() {
  local exit_code=$?

  trap - EXIT
  for d in "$PROJECT_ROOT/dist" "$PROJECT_ROOT/build"; do
    if [[ -d "$d" ]]; then
      # Root in a rootless Podman container maps to a subordinate host UID.
      # Translate it back to the invoking host user after every build.
      if ! podman unshare chown -R 0:0 "$d" 2>/dev/null; then
        echo "warning: could not restore host ownership of $d" >&2
      fi
      # Also ensure host user owns files (podman unshare may map to 0:0, then chown to caller)
      # Try to chown to current UID/GID if unshare failed
      if [[ $EUID -ne 0 ]] && ! podman unshare true 2>/dev/null; then
        # Without podman unshare, try direct chown if we have permission
        chown -R "$(id -u):$(id -g)" "$d" 2>/dev/null || true
      fi
    fi
  done
  exit "$exit_code"
}

trap restore_output_ownership EXIT

if ! podman image exists "$BUILDER_TAG" 2>/dev/null; then
  echo "Builder image $BUILDER_TAG not found — building it once..."
  podman build -t "$BUILDER_TAG" -f "$CONTAINERFILE" "$PROJECT_ROOT"
fi

# Cargo registry + git cache volumes survive across builds (save re-downloading crates)
podman volume create lexaloud-cargo-registry >/dev/null 2>&1 || true
podman volume create lexaloud-cargo-git >/dev/null 2>&1 || true
# Pip cache kept for Phase 9 branch compatibility (removed in Phase 10)
podman volume create lexaloud-pip-cache >/dev/null 2>&1 || true

echo "Running native build inside $BUILDER_TAG (incremental, cargo cache mounted)..."
podman run --rm \
  -v "$PROJECT_ROOT:/workspace:Z" -w /workspace \
  -v lexaloud-cargo-registry:/root/.cargo/registry \
  -v lexaloud-cargo-git:/root/.cargo/git \
  -v lexaloud-pip-cache:/root/.cache/pip \
  -e LEXALOUD_INCREMENTAL=1 \
  -e CARGO_HOME=/root/.cargo \
  -e PIP_CACHE_DIR=/root/.cache/pip \
  "$BUILDER_TAG" ./scripts/build-appimage.sh "$@"
