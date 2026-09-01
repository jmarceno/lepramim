#!/usr/bin/env bash
# Fast incremental AppImage build using the cached builder image.
# Reuses the Debian layer (apt packages) and the Python venv when possible,
# so only PyInstaller + AppDir work runs on source changes.
# First run builds the builder image once (~2 min), subsequent runs skip it.
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILDER_TAG="lexaloud-builder:24.04"
CONTAINERFILE="$PROJECT_ROOT/packaging/Containerfile.builder"

if ! podman image exists "$BUILDER_TAG" 2>/dev/null; then
  echo "Builder image $BUILDER_TAG not found — building it once..."
  podman build -t "$BUILDER_TAG" -f "$CONTAINERFILE" "$PROJECT_ROOT"
fi

# Pip cache volume survives across builds (saves re-downloading wheels)
podman volume create lexaloud-pip-cache >/dev/null 2>&1 || true

# Also keep a named volume for ccache if available (optional)
# podman volume create lexaloud-ccache >/dev/null 2>&1 || true

echo "Running build inside $BUILDER_TAG (incremental, pip cache mounted)..."
exec podman run --rm \
  -v "$PROJECT_ROOT:/workspace:Z" -w /workspace \
  -v lexaloud-pip-cache:/root/.cache/pip \
  -e LEXALOUD_INCREMENTAL=1 \
  -e PIP_CACHE_DIR=/root/.cache/pip \
  "$BUILDER_TAG" ./scripts/build-appimage.sh "$@"
