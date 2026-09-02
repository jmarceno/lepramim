#!/usr/bin/env bash
# Build the CPU AppImage from the native Rust stage (single lexaloud binary).
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILD_ROOT="${LEXALOUD_APPIMAGE_BUILD_DIR:-$PROJECT_ROOT/build/appimage}"
APPDIR="${LEXALOUD_APPDIR:-$PROJECT_ROOT/build/appdir}"
STAGE="${LEXALOUD_STAGE:-$PROJECT_ROOT/build/stage}"
OUTPUT_DIR="${LEXALOUD_APPIMAGE_OUTPUT_DIR:-$BUILD_ROOT}"
VERSION="${LEXALOUD_VERSION:-}"
APPIMAGE_TOOL="${APPIMAGETOOL:-}"
APPIMAGE_TOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"
LINUXDEPLOY="${LINUXDEPLOY:-}"
PATCHELF="${PATCHELF:-}"

die() { echo "build-appimage: $*" >&2; exit 1; }

if [[ -z "$VERSION" ]]; then
  VERSION="$(grep -E '^version\s*=' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed -E 's/.*"(.*)".*/\1/' || true)"
  VERSION="${VERSION:-0.2.0}"
fi

echo "=== Lexaloud AppImage build ==="
echo "stage:   $STAGE"
echo "appdir:  $APPDIR"
echo "version: $VERSION"
echo

if [[ ! -x "$STAGE/bin/lexaloud" ]]; then
  "$PROJECT_ROOT/scripts/build-native.sh" --release --stage "$STAGE"
fi
[[ -x "$STAGE/bin/lexaloud" ]] || die "staged binary missing at $STAGE/bin/lexaloud"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/doc/lexaloud" \
         "$APPDIR/usr/share/lexaloud"

install -m 0755 "$STAGE/bin/lexaloud" "$APPDIR/usr/bin/lexaloud"

for f in share/applications/lexaloud.desktop share/icons/hicolor/scalable/apps/lexaloud.svg; do
  [[ -f "$STAGE/$f" ]] && cp -a "$STAGE/$f" "$APPDIR/usr/$f"
done
[[ -f "$APPDIR/usr/share/applications/lexaloud.desktop" ]] || \
  install -m 0644 "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" "$APPDIR/usr/share/applications/lexaloud.desktop" 2>/dev/null || true
[[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" ]] || \
  install -m 0644 "$PROJECT_ROOT/src/lexaloud/icons/lexaloud.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" 2>/dev/null || true

ln -sf usr/share/applications/lexaloud.desktop "$APPDIR/lexaloud.desktop"
[[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" ]] && \
  ln -sf usr/share/icons/hicolor/scalable/apps/lexaloud.svg "$APPDIR/lexaloud.svg" 2>/dev/null || true

[[ -f "$PROJECT_ROOT/LICENSE" ]] && cp -a "$PROJECT_ROOT/LICENSE" "$APPDIR/usr/share/doc/lexaloud/LICENSE" 2>/dev/null || true
[[ -f "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" ]] && cp -a "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" "$APPDIR/usr/share/doc/lexaloud/" 2>/dev/null || true
[[ -f "$STAGE/share/lexaloud/config.example.toml" ]] && cp -a "$STAGE/share/lexaloud/config.example.toml" "$APPDIR/usr/share/lexaloud/" 2>/dev/null || true

install -m 0755 "$PROJECT_ROOT/packaging/appimage/AppRun" "$APPDIR/AppRun"
ln -sf AppRun "$APPDIR/lexaloud" 2>/dev/null || true

bundle_ldd_libs() {
  local bin="$1"
  [[ -x "$bin" ]] || return 0
  command -v ldd >/dev/null 2>&1 || return 0
  local libs
  libs="$(env -u LD_LIBRARY_PATH ldd "$bin" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u || true)"
  for lib in $libs; do
    [[ -f "$lib" ]] || continue
    case "$(basename "$lib")" in
      libc.so*|libpthread*|libdl.so*|libm.so*|libgcc*|libstdc++*) continue ;;
      libQt*|libQt6*) continue ;;
    esac
    cp -aL "$lib" "$APPDIR/usr/lib/" 2>/dev/null || cp -a "$lib" "$APPDIR/usr/lib/" 2>/dev/null || true
  done
}

echo "--- Bundling shared libraries (ldd) ---"
bundle_ldd_libs "$APPDIR/usr/bin/lexaloud"

echo "--- ONNX Runtime ---"
for pat in /usr/lib/x86_64-linux-gnu/libonnxruntime.so* /usr/lib/libonnxruntime.so*; do
  for f in $pat; do
    [[ -f "$f" ]] && cp -a "$f" "$APPDIR/usr/lib/" 2>/dev/null && echo "  $f"
  done
done

echo "--- eSpeak data ---"
for cand in /usr/share/espeak-ng-data /usr/share/espeak-data; do
  if [[ -d "$cand" ]]; then
    mkdir -p "$APPDIR/usr/share/espeak-ng-data"
    cp -a "$cand"/. "$APPDIR/usr/share/espeak-ng-data/" 2>/dev/null || true
    break
  fi
done

copy_helper() {
  local name="$1"
  local path
  path="$(command -v "$name" 2>/dev/null || true)"
  [[ -n "$path" ]] || { echo "  helper $name not on host"; return 0; }
  cp -aL "$path" "$APPDIR/usr/bin/$name" 2>/dev/null || true
  echo "  bundled $name"
}
copy_helper wl-paste
copy_helper xclip
command -v notify-send >/dev/null 2>&1 && cp -aL "$(command -v notify-send)" "$APPDIR/usr/bin/notify-send" 2>/dev/null || true

if [[ -z "$PATCHELF" ]]; then PATCHELF="$(command -v patchelf 2>/dev/null || true)"; fi
if [[ -n "$PATCHELF" && -x "$PATCHELF" ]]; then
  while IFS= read -r -d '' elf; do
    "$PATCHELF" --clear-execstack "$elf" 2>/dev/null || true
  done < <(find "$APPDIR/usr" -type f \( -name '*.so' -o -name '*.so.*' -o -name 'lexaloud' \) -print0 2>/dev/null)
fi

if [[ -z "$LINUXDEPLOY" ]]; then LINUXDEPLOY="$BUILD_ROOT/linuxdeploy-x86_64.AppImage"; fi
if [[ -x "$LINUXDEPLOY" ]] || command -v linuxdeploy >/dev/null 2>&1; then
  [[ -x "$LINUXDEPLOY" ]] || LINUXDEPLOY="$(command -v linuxdeploy)"
  echo "--- linuxdeploy ---"
  APPIMAGE_EXTRACT_AND_RUN=1 "$LINUXDEPLOY" --appdir "$APPDIR" --executable "$APPDIR/usr/bin/lexaloud" 2>&1 | sed 's/^/  /' || true
fi

if [[ -z "$APPIMAGE_TOOL" ]]; then APPIMAGE_TOOL="$BUILD_ROOT/appimagetool-x86_64.AppImage"; fi
if [[ ! -x "$APPIMAGE_TOOL" ]]; then
  mkdir -p "$(dirname "$APPIMAGE_TOOL")"
  curl -fsSL "$APPIMAGE_TOOL_URL" -o "$APPIMAGE_TOOL"
  chmod 0755 "$APPIMAGE_TOOL"
fi
command -v appimagetool >/dev/null 2>&1 && APPIMAGE_TOOL="$(command -v appimagetool)"

mkdir -p "$OUTPUT_DIR"
OUTPUT="$OUTPUT_DIR/Lexaloud-${VERSION}-x86_64.AppImage"
rm -f "$OUTPUT"
APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_TOOL" "$APPDIR" "$OUTPUT"
chmod 0755 "$OUTPUT"

if command -v ldd >/dev/null 2>&1; then
  LDD_OUT="$(env -u LD_LIBRARY_PATH ldd "$APPDIR/usr/bin/lexaloud" 2>&1 || true)"
  if echo "$LDD_OUT" | grep -qiE 'libQt|Qt6'; then
    die "AppDir binary must not link Qt"
  fi
fi

echo "AppImage: $OUTPUT"
du -h "$OUTPUT" 2>/dev/null || true
