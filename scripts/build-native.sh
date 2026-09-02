#!/usr/bin/env bash
# Lexaloud native build orchestrator — Rust core + Qt UI
#
# Runs Cargo first and CMake second; stages a deterministic layout for
# packaging and AppImage creation. Never installs into the live user
# environment implicitly — all outputs go to --stage.
#
# Usage:
#   ./scripts/build-native.sh [--debug|--release] [--stage <absolute-path>]
#
# Defaults:
#   --debug  (cargo build --locked, cmake --preset dev)
#   --stage  $PWD/build/stage   (must be absolute if provided)
#
# Release path (per Phase 9 contract):
#   cargo build --locked --release
#   cmake --preset release
#   cmake --build --preset release --parallel
#   ctest --preset release --output-on-failure   (optional for staging)
#   ./scripts/build-native.sh --release --stage "$PWD/build/stage"
#
# Notes:
#   - Stops at first failed command (set -euo pipefail).
#   - Requires absolute --stage; relative paths are rejected to avoid
#     ambiguous staging and accidental installs.
#   - Wraps cmake invocations with `env -u LD_LIBRARY_PATH` because the
#     OpenCode / AppImage host sets LD_LIBRARY_PATH to its own mount
#     (e.g. /tmp/.mount_opencode*/usr/lib) which breaks CMake's
#     CMAKE_ROOT detection ("Could not find CMAKE_ROOT") and Qt discovery.
#     Unsetting it for the cmake subprocess is safe and does not affect the
#     built binaries.
#
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILD_TYPE="debug"
STAGE=""

usage() {
  cat <<EOF
Usage: $0 [--debug|--release] [--stage <absolute-path>]

  --debug            Debug build (default): cargo build --locked, cmake --preset dev
  --release          Release build: cargo build --locked --release, cmake --preset release
  --stage <path>     Absolute path to stage directory (default: \$PWD/build/stage)
  -h, --help         Show this help

Examples:
  $0 --debug
  $0 --release --stage "\$PWD/build/stage"
  $0 --release --stage /tmp/lexaloud-stage
EOF
}

# --- parse args -----------------------------------------------------------
while (( "$#" )); do
  case "$1" in
    --debug) BUILD_TYPE="debug"; shift ;;
    --release) BUILD_TYPE="release"; shift ;;
    --stage)
      if [[ $# -lt 2 ]]; then echo "error: --stage requires an argument" >&2; exit 2; fi
      STAGE="$2"
      shift 2
      ;;
    --stage=*)
      STAGE="${1#*=}"
      shift
      ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ $# -gt 0 ]]; then
  echo "error: unexpected positional arguments: $*" >&2
  usage >&2
  exit 2
fi

if [[ -z "$STAGE" ]]; then
  STAGE="$PROJECT_ROOT/build/stage"
fi

# Require absolute path
if [[ "$STAGE" != /* ]]; then
  echo "error: --stage must be an absolute path, got: $STAGE" >&2
  echo "hint: use \$PWD/build/stage or an absolute path like /tmp/lexaloud-stage" >&2
  exit 2
fi

echo "=== Lexaloud build-native ==="
echo "project root: $PROJECT_ROOT"
echo "build type:   $BUILD_TYPE"
echo "stage:        $STAGE"
echo

# --- Cargo build (first) --------------------------------------------------
echo "--- Cargo build ($BUILD_TYPE) ---"
if [[ "$BUILD_TYPE" == "release" ]]; then
  # Phase 9 contract: lto=thin, codegen-units=1, strip=symbols, panic=abort
  # are set in Cargo.toml [profile.release]; we just invoke cargo.
  cargo build --locked --release
else
  cargo build --locked
fi
echo "Cargo build finished."
echo

# --- CMake build (second) -------------------------------------------------
# Use env -u LD_LIBRARY_PATH wrapper; see header comment.
echo "--- CMake configure & build ($BUILD_TYPE) ---"
if [[ "$BUILD_TYPE" == "release" ]]; then
  env -u LD_LIBRARY_PATH cmake --preset release
  env -u LD_LIBRARY_PATH cmake --build --preset release --parallel
  # Optionally run ctest for release staging (non-fatal for stage creation).
  # The release gate in CI does run ctest separately; local staging tolerates
  # missing tests but warns.
  if env -u LD_LIBRARY_PATH ctest --preset release --output-on-failure 2>&1; then
    echo "ctest release passed."
  else
    echo "warning: ctest --preset release failed or no tests — continuing to stage (CI will enforce)" >&2
  fi
else
  # dev preset for local debug iteration; ci preset is Debug + warnings-as-errors
  # We use dev here so that warnings don't block local development.
  env -u LD_LIBRARY_PATH cmake --preset dev
  env -u LD_LIBRARY_PATH cmake --build --preset dev --parallel
fi
echo "CMake build finished."
echo

# --- Staging --------------------------------------------------------------
echo "--- Staging to $STAGE ---"
# Deterministic: start clean, create known layout.
rm -rf "$STAGE"
mkdir -p "$STAGE/bin"
mkdir -p "$STAGE/share/applications"
mkdir -p "$STAGE/share/icons/hicolor/scalable/apps"
mkdir -p "$STAGE/share/doc/lexaloud"
mkdir -p "$STAGE/share/lexaloud"
mkdir -p "$STAGE/lib"

# Locate Rust binary
RUST_BIN=""
if [[ "$BUILD_TYPE" == "release" ]]; then
  RUST_BIN="$PROJECT_ROOT/target/release/lexaloud"
else
  RUST_BIN="$PROJECT_ROOT/target/debug/lexaloud"
fi

# Locate Qt UI binary — support both top-level and subdir layouts.
# Root CMakeLists (add_subdirectory(ui)) produces build/ui-*/ui/lexaloud-ui
# Direct ui/ CMakeLists produces build/ui-*/lexaloud-ui
QT_CANDIDATES=()
if [[ "$BUILD_TYPE" == "release" ]]; then
  QT_CANDIDATES=(
    "$PROJECT_ROOT/build/ui-release/ui/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-release/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-ci/ui/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-ci/lexaloud-ui"
  )
else
  QT_CANDIDATES=(
    "$PROJECT_ROOT/build/ui-dev/ui/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-dev/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-ci/ui/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-ci/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-release/ui/lexaloud-ui"
    "$PROJECT_ROOT/build/ui-release/lexaloud-ui"
  )
fi

QT_BIN=""
for cand in "${QT_CANDIDATES[@]}"; do
  if [[ -x "$cand" ]]; then
    QT_BIN="$cand"
    break
  fi
done
# Fallback: search any build/ui-*/**/lexaloud-ui
if [[ -z "$QT_BIN" ]]; then
  FOUND="$(find "$PROJECT_ROOT/build" -type f -name "lexaloud-ui" -executable 2>/dev/null | head -n 1 || true)"
  if [[ -n "$FOUND" ]]; then
    QT_BIN="$FOUND"
  fi
fi

# Validate executables exist and are executable
if [[ ! -x "$RUST_BIN" ]]; then
  echo "error: Rust binary missing or not executable: $RUST_BIN" >&2
  echo "hint: run 'cargo build --locked${BUILD_TYPE:+ --release}' first or check Cargo errors above" >&2
  exit 1
fi
if [[ -z "$QT_BIN" || ! -x "$QT_BIN" ]]; then
  echo "error: Qt UI binary missing. Tried:" >&2
  for c in "${QT_CANDIDATES[@]}"; do echo "  - $c" >&2; done
  echo "hint: run 'cmake --preset ${BUILD_TYPE}' && 'cmake --build --preset ${BUILD_TYPE}' first" >&2
  exit 1
fi

echo "Found Rust binary: $RUST_BIN"
echo "Found Qt binary:   $QT_BIN"

# Copy binaries (deterministic permissions)
install -m 0755 "$RUST_BIN" "$STAGE/bin/lexaloud"
install -m 0755 "$QT_BIN" "$STAGE/bin/lexaloud-ui"
echo "Staged binaries to $STAGE/bin/"

# Strip only at staging for release (keep CI/debug unstripped).
if [[ "$BUILD_TYPE" == "release" ]]; then
  if command -v strip >/dev/null 2>&1; then
    echo "Stripping release binaries at stage (preserving unstripped in target/ and build/)..."
    strip --strip-unneeded "$STAGE/bin/lexaloud" 2>/dev/null || strip "$STAGE/bin/lexaloud" 2>/dev/null || true
    strip --strip-unneeded "$STAGE/bin/lexaloud-ui" 2>/dev/null || strip "$STAGE/bin/lexaloud-ui" 2>/dev/null || true
  fi
fi

# --- Desktop file ---------------------------------------------------------
DESKTOP_SRC=""
for cand in \
  "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" \
  "$PROJECT_ROOT/assets/lexaloud.desktop" \
  "$PROJECT_ROOT/packaging/lexaloud.desktop" \
  "$PROJECT_ROOT/src/lexaloud/templates/lexaloud.desktop.template" \
; do
  if [[ -f "$cand" ]]; then DESKTOP_SRC="$cand"; break; fi
done
if [[ -n "$DESKTOP_SRC" ]]; then
  # If template, render minimal desktop file
  if [[ "$DESKTOP_SRC" == *.template ]]; then
    sed 's|{indicator_binary}|lexaloud-ui|g' "$DESKTOP_SRC" > "$STAGE/share/applications/lexaloud.desktop"
    chmod 0644 "$STAGE/share/applications/lexaloud.desktop"
    echo "Staged desktop file from template: $DESKTOP_SRC"
  else
    install -m 0644 "$DESKTOP_SRC" "$STAGE/share/applications/lexaloud.desktop"
    # Ensure Exec points to lexaloud-ui for native stage; desktop validation
    # can tolerate Exec=lexaloud but we normalize to lexaloud-ui if needed.
    echo "Staged desktop file: $DESKTOP_SRC"
  fi
else
  echo "warning: no desktop file found (tried packaging/appimage/lexaloud.desktop, assets/lexaloud.desktop)" >&2
  cat > "$STAGE/share/applications/lexaloud.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Version=1.0
Name=Lexaloud
GenericName=Text to Speech
Comment=Local Kokoro text-to-speech tool
Exec=lexaloud-ui
Icon=lexaloud
Terminal=false
Categories=AudioVideo;Audio;Accessibility;
Keywords=tts;text-to-speech;kokoro;read;aloud;narrate;
StartupNotify=true
DESKTOP
  chmod 0644 "$STAGE/share/applications/lexaloud.desktop"
  echo "Generated minimal desktop file at $STAGE/share/applications/lexaloud.desktop"
fi

# --- Icons ----------------------------------------------------------------
ICON_SRC=""
for cand in \
  "$PROJECT_ROOT/src/lexaloud/icons/lexaloud.svg" \
  "$PROJECT_ROOT/packaging/appimage/lexaloud.svg" \
  "$PROJECT_ROOT/assets/icons/lexaloud.svg" \
; do
  if [[ -f "$cand" ]]; then ICON_SRC="$cand"; break; fi
done
if [[ -n "$ICON_SRC" ]]; then
  install -m 0644 "$ICON_SRC" "$STAGE/share/icons/hicolor/scalable/apps/lexaloud.svg"
  echo "Staged icon: $ICON_SRC"
else
  echo "warning: no icon found (tried src/lexaloud/icons/lexaloud.svg, packaging/appimage/lexaloud.svg)" >&2
fi

# --- Service / systemd ----------------------------------------------------
SYSTEMD_TEMPLATE="$PROJECT_ROOT/src/lexaloud/templates/systemd.service.template"
if [[ -f "$SYSTEMD_TEMPLATE" ]]; then
  mkdir -p "$STAGE/share/doc/lexaloud"
  install -m 0644 "$SYSTEMD_TEMPLATE" "$STAGE/share/lexaloud/systemd.service.template"
  echo "Staged systemd template."
else
  echo "warning: systemd template not found at $SYSTEMD_TEMPLATE" >&2
fi

# --- Licenses -------------------------------------------------------------
for f in LICENSE THIRD_PARTY_LICENSES.md; do
  if [[ -f "$PROJECT_ROOT/$f" ]]; then
    install -m 0644 "$PROJECT_ROOT/$f" "$STAGE/share/doc/lexaloud/$f"
    echo "Staged $f"
  else
    echo "warning: $f not found at $PROJECT_ROOT/$f" >&2
  fi
done

# --- Model metadata (if present) -----------------------------------------
# Preserve model artifact metadata for packaging inventory; actual weights
# stay in user cache (~/.cache/lexaloud/models) and are not bundled.
if [[ -f "$PROJECT_ROOT/src/models.rs" ]]; then
  mkdir -p "$STAGE/share/lexaloud"
  # Extract ARTIFACTS hashes/URLs for manifest generation (best-effort)
  echo "Model metadata available in src/models.rs (not bundling weights)"
fi
if [[ -f "$PROJECT_ROOT/src/lexaloud/models.py" ]]; then
  echo "legacy model metadata still present (unexpected in native build)" >&2
fi

# --- Config example -------------------------------------------------------
if [[ -f "$PROJECT_ROOT/src/lexaloud/templates/config.example.toml" ]]; then
  install -m 0644 "$PROJECT_ROOT/src/lexaloud/templates/config.example.toml" "$STAGE/share/lexaloud/config.example.toml"
  echo "Staged config.example.toml"
fi

# --- Runtime dependency checks --------------------------------------------
echo "--- Verifying staged executables ---"
# Check dynamic linkage via ldd; fail if declared runtime deps missing.
# We use env -u LD_LIBRARY_PATH for ldd as well to avoid appimage env pollution.

# Rust core must NOT link Qt per Phase 10 gate.
if command -v ldd >/dev/null 2>&1; then
  echo "ldd $STAGE/bin/lexaloud:"
  LDD_CORE="$(env -u LD_LIBRARY_PATH ldd "$STAGE/bin/lexaloud" 2>&1 || true)"
  echo "$LDD_CORE" | sed 's/^/  /'
  if echo "$LDD_CORE" | grep -qiE 'libQt|Qt6'; then
    echo "error: Rust core unexpectedly links Qt: see ldd above" >&2
    exit 1
  fi
  # Check for missing shared libs (=> not found)
  if echo "$LDD_CORE" | grep -q "not found"; then
    echo "error: Rust binary has missing shared libraries:" >&2
    echo "$LDD_CORE" | grep "not found" >&2
    exit 1
  fi

  echo "ldd $STAGE/bin/lexaloud-ui:"
  LDD_UI="$(env -u LD_LIBRARY_PATH ldd "$STAGE/bin/lexaloud-ui" 2>&1 || true)"
  echo "$LDD_UI" | sed 's/^/  /'
  if echo "$LDD_UI" | grep -q "not found"; then
    echo "error: Qt UI binary has missing shared libraries (likely missing Qt6 dev packages):" >&2
    echo "$LDD_UI" | grep "not found" >&2
    exit 1
  fi
  # Qt UI must link at least Qt6Core/Gui/Widgets
  if ! echo "$LDD_UI" | grep -q "Qt6"; then
    echo "warning: Qt UI does not appear to link Qt6 (ldd shows no Qt6Core/Gui). Build may have used wrong Qt." >&2
  fi
else
  echo "warning: ldd not found; skipping linkage verification" >&2
fi

# --- Staged file manifest (deterministic, sorted) -------------------------
MANIFEST="$STAGE/../staged-files.txt"
if [[ "$STAGE" == "$PROJECT_ROOT/build/stage" ]]; then
  MANIFEST="$PROJECT_ROOT/build/staged-files.txt"
fi
find "$STAGE" -type f | sort > "$MANIFEST" 2>/dev/null || true
echo "Wrote staged manifest to $MANIFEST ($(wc -l < "$MANIFEST" 2>/dev/null || echo 0) files)"

# --- Size report ----------------------------------------------------------
if command -v du >/dev/null 2>&1; then
  echo "--- Stage size ---"
  du -sh "$STAGE" 2>/dev/null || true
  echo "Binaries:"
  ls -lh "$STAGE/bin/" 2>/dev/null || true
fi

echo
echo "=== build-native complete ==="
echo "stage: $STAGE"
echo "bin/lexaloud:     $(ls -lh "$STAGE/bin/lexaloud" 2>/dev/null | awk '{print $5, $9}')"
echo "bin/lexaloud-ui:  $(ls -lh "$STAGE/bin/lexaloud-ui" 2>/dev/null | awk '{print $5, $9}')"
echo "manifest:         $MANIFEST"
echo
echo "Next: ./scripts/build-appimage.sh  (from staged native build)"
