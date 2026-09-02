#!/usr/bin/env bash
# Lexaloud native build — single Rust binary (Iced tray UI in-process).
#
# Usage:
#   ./scripts/build-native.sh [--debug|--release] [--stage <absolute-path>] [--features llm]
#
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILD_TYPE="debug"
STAGE=""
FEATURES=""

usage() {
  cat <<EOF
Usage: $0 [--debug|--release] [--stage <absolute-path>] [--features <feats>]

  --debug            Debug build (default)
  --release          Release build
  --stage <path>     Absolute path to stage directory (default: \$PWD/build/stage)
  --features <feats> Extra cargo features (e.g. llm).
  -h, --help         Show this help
EOF
}

while (( "$#" )); do
  case "$1" in
    --debug) BUILD_TYPE="debug"; shift ;;
    --release) BUILD_TYPE="release"; shift ;;
    --stage)
      [[ $# -ge 2 ]] || { echo "error: --stage requires an argument" >&2; exit 2; }
      STAGE="$2"; shift 2 ;;
    --stage=*) STAGE="${1#*=}"; shift ;;
    --features)
      [[ $# -ge 2 ]] || { echo "error: --features requires an argument" >&2; exit 2; }
      FEATURES="$2"; shift 2 ;;
    --features=*) FEATURES="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ -z "$STAGE" ]] && STAGE="$PROJECT_ROOT/build/stage"
[[ "$STAGE" == /* ]] || { echo "error: --stage must be absolute: $STAGE" >&2; exit 2; }

echo "=== Lexaloud build-native ==="
echo "project root: $PROJECT_ROOT"
echo "build type:   $BUILD_TYPE"
echo "stage:        $STAGE"
echo "features:     ${FEATURES:-none}"
echo

echo "--- Cargo build ($BUILD_TYPE) ---"
CARGO_ARGS=(build --locked)
if [[ "$BUILD_TYPE" == "release" ]]; then
  CARGO_ARGS+=(--release)
fi
if [[ -n "$FEATURES" ]]; then
  CARGO_ARGS+=(--features "$FEATURES")
fi
cargo "${CARGO_ARGS[@]}"
if [[ "$BUILD_TYPE" == "release" ]]; then
  RUST_BIN="$PROJECT_ROOT/target/release/lexaloud"
else
  RUST_BIN="$PROJECT_ROOT/target/debug/lexaloud"
fi
[[ -x "$RUST_BIN" ]] || { echo "error: binary missing: $RUST_BIN" >&2; exit 1; }
echo "Cargo build finished."
echo

echo "--- Staging to $STAGE ---"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" \
         "$STAGE/share/applications" \
         "$STAGE/share/icons/hicolor/scalable/apps" \
         "$STAGE/share/doc/lexaloud" \
         "$STAGE/share/lexaloud"

install -m 0755 "$RUST_BIN" "$STAGE/bin/lexaloud"
if [[ "$BUILD_TYPE" == "release" ]] && command -v strip >/dev/null 2>&1; then
  strip --strip-unneeded "$STAGE/bin/lexaloud" 2>/dev/null || strip "$STAGE/bin/lexaloud" 2>/dev/null || true
fi

DESKTOP_SRC=""
for cand in \
  "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" \
  "$PROJECT_ROOT/src/lexaloud/templates/lexaloud.desktop.template" \
; do
  [[ -f "$cand" ]] && DESKTOP_SRC="$cand" && break
done
if [[ -n "$DESKTOP_SRC" ]]; then
  if [[ "$DESKTOP_SRC" == *.template ]]; then
    sed 's|{indicator_binary}|lexaloud|g' "$DESKTOP_SRC" > "$STAGE/share/applications/lexaloud.desktop"
  else
    install -m 0644 "$DESKTOP_SRC" "$STAGE/share/applications/lexaloud.desktop"
  fi
  chmod 0644 "$STAGE/share/applications/lexaloud.desktop"
else
  cat > "$STAGE/share/applications/lexaloud.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Version=1.0
Name=Lexaloud
GenericName=Text to Speech
Comment=Local Kokoro text-to-speech tool
Exec=lexaloud
Icon=lexaloud
Terminal=false
Categories=AudioVideo;Audio;Accessibility;
StartupNotify=false
DESKTOP
  chmod 0644 "$STAGE/share/applications/lexaloud.desktop"
fi

ICON_SRC="$PROJECT_ROOT/src/lexaloud/icons/lexaloud.svg"
[[ -f "$ICON_SRC" ]] && install -m 0644 "$ICON_SRC" "$STAGE/share/icons/hicolor/scalable/apps/lexaloud.svg"

SYSTEMD_TEMPLATE="$PROJECT_ROOT/src/lexaloud/templates/systemd.service.template"
[[ -f "$SYSTEMD_TEMPLATE" ]] && install -m 0644 "$SYSTEMD_TEMPLATE" "$STAGE/share/lexaloud/systemd.service.template"

for f in LICENSE THIRD_PARTY_LICENSES.md; do
  [[ -f "$PROJECT_ROOT/$f" ]] && install -m 0644 "$PROJECT_ROOT/$f" "$STAGE/share/doc/lexaloud/$f"
done

CONFIG_EXAMPLE="$PROJECT_ROOT/src/lexaloud/templates/config.example.toml"
[[ -f "$CONFIG_EXAMPLE" ]] && install -m 0644 "$CONFIG_EXAMPLE" "$STAGE/share/lexaloud/config.example.toml"

echo "--- Verifying staged binary ---"
if command -v ldd >/dev/null 2>&1; then
  LDD_OUT="$(env -u LD_LIBRARY_PATH ldd "$STAGE/bin/lexaloud" 2>&1 || true)"
  echo "$LDD_OUT" | sed 's/^/  /'
  if echo "$LDD_OUT" | grep -qiE 'libQt|Qt6'; then
    echo "error: lexaloud must not link Qt" >&2
    exit 1
  fi
  if echo "$LDD_OUT" | grep -q "not found"; then
    echo "error: missing shared libraries:" >&2
    echo "$LDD_OUT" | grep "not found" >&2
    exit 1
  fi
fi

MANIFEST="$PROJECT_ROOT/build/staged-files.txt"
find "$STAGE" -type f | sort > "$MANIFEST"
echo
echo "=== build-native complete ==="
echo "stage:    $STAGE"
echo "binary:   $STAGE/bin/lexaloud"
echo "manifest: $MANIFEST"
