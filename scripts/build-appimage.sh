#!/usr/bin/env bash
# Build the CPU AppImage from the native Rust stage (single lepramim binary).
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck source=scripts/lib/sanitize-host-appimage-env.sh
source "$PROJECT_ROOT/scripts/lib/sanitize-host-appimage-env.sh"
sanitize_host_appimage_env

BUILD_ROOT="${LEPRAMIM_APPIMAGE_BUILD_DIR:-$PROJECT_ROOT/build/appimage}"
APPDIR="${LEPRAMIM_APPDIR:-$PROJECT_ROOT/build/appdir}"
STAGE="${LEPRAMIM_STAGE:-$PROJECT_ROOT/build/stage}"
OUTPUT_DIR="${LEPRAMIM_APPIMAGE_OUTPUT_DIR:-$BUILD_ROOT}"
VERSION="${LEPRAMIM_VERSION:-}"
APPIMAGE_TOOL="${APPIMAGETOOL:-}"
APPIMAGE_TOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"
LINUXDEPLOY="${LINUXDEPLOY:-}"
LINUXDEPLOY_PLUGIN_QT="${LINUXDEPLOY_PLUGIN_QT:-}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
LINUXDEPLOY_PLUGIN_QT_URL="${LINUXDEPLOY_PLUGIN_QT_URL:-https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage}"
PATCHELF="${PATCHELF:-}"

die() { echo "build-appimage: $*" >&2; exit 1; }

if [[ -z "$VERSION" ]]; then
  VERSION="$(grep -E '^version\s*=' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed -E 's/.*"(.*)".*/\1/' || true)"
  VERSION="${VERSION:-0.2.0}"
fi

echo "=== Lepramim AppImage build ==="
echo "stage:   $STAGE"
echo "appdir:  $APPDIR"
echo "version: $VERSION"
echo

if [[ ! -x "$STAGE/bin/lepramim" ]]; then
  "$PROJECT_ROOT/scripts/build-native.sh" --release --stage "$STAGE"
fi
[[ -x "$STAGE/bin/lepramim" ]] || die "staged binary missing at $STAGE/bin/lepramim"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/icons/hicolor/512x512/apps" \
         "$APPDIR/usr/share/doc/lepramim" \
         "$APPDIR/usr/share/lepramim"

install -m 0755 "$STAGE/bin/lepramim" "$APPDIR/usr/bin/lepramim"

for f in share/applications/lepramim.desktop share/icons/hicolor/scalable/apps/lepramim.svg share/icons/hicolor/512x512/apps/lepramim.png; do
  [[ -f "$STAGE/$f" ]] && cp -a "$STAGE/$f" "$APPDIR/usr/$f"
done
[[ -f "$APPDIR/usr/share/applications/lepramim.desktop" ]] || \
  install -m 0644 "$PROJECT_ROOT/packaging/appimage/lepramim.desktop" "$APPDIR/usr/share/applications/lepramim.desktop" 2>/dev/null || true
[[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lepramim.svg" ]] || \
  install -m 0644 "$PROJECT_ROOT/src/lepramim/icons/lepramim.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/lepramim.svg" 2>/dev/null || true
if [[ ! -f "$APPDIR/usr/share/icons/hicolor/512x512/apps/lepramim.png" ]]; then
  for cand in "$PROJECT_ROOT/packaging/appimage/lepramim.png" "$STAGE/share/icons/hicolor/512x512/apps/lepramim.png"; do
    if [[ -f "$cand" ]]; then
      install -m 0644 "$cand" "$APPDIR/usr/share/icons/hicolor/512x512/apps/lepramim.png" 2>/dev/null || true
      break
    fi
  done
fi

ln -sf usr/share/applications/lepramim.desktop "$APPDIR/lepramim.desktop"
[[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lepramim.svg" ]] && \
  ln -sf usr/share/icons/hicolor/scalable/apps/lepramim.svg "$APPDIR/lepramim.svg" 2>/dev/null || true
# PNG fallback at AppDir root for toolkits without SVG support.
if [[ -f "$APPDIR/usr/share/icons/hicolor/512x512/apps/lepramim.png" ]]; then
  ln -sf usr/share/icons/hicolor/512x512/apps/lepramim.png "$APPDIR/lepramim.png" 2>/dev/null || true
elif [[ -f "$PROJECT_ROOT/packaging/appimage/lepramim.png" ]]; then
  install -m 0644 "$PROJECT_ROOT/packaging/appimage/lepramim.png" "$APPDIR/lepramim.png" 2>/dev/null || true
fi
# .DirIcon makes file managers show the logo for the AppImage file itself.
if [[ -f "$APPDIR/usr/share/icons/hicolor/512x512/apps/lepramim.png" ]]; then
  cp -a "$APPDIR/usr/share/icons/hicolor/512x512/apps/lepramim.png" "$APPDIR/.DirIcon" 2>/dev/null || true
elif [[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lepramim.svg" ]]; then
  cp -a "$APPDIR/usr/share/icons/hicolor/scalable/apps/lepramim.svg" "$APPDIR/.DirIcon" 2>/dev/null || true
fi

[[ -f "$PROJECT_ROOT/LICENSE" ]] && cp -a "$PROJECT_ROOT/LICENSE" "$APPDIR/usr/share/doc/lepramim/LICENSE" 2>/dev/null || true
[[ -f "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" ]] && cp -a "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" "$APPDIR/usr/share/doc/lepramim/" 2>/dev/null || true
[[ -f "$STAGE/share/lepramim/config.example.toml" ]] && cp -a "$STAGE/share/lepramim/config.example.toml" "$APPDIR/usr/share/lepramim/" 2>/dev/null || true

install -m 0755 "$PROJECT_ROOT/packaging/appimage/AppRun" "$APPDIR/AppRun"
ln -sf AppRun "$APPDIR/lepramim" 2>/dev/null || true

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
    esac
    cp -aL "$lib" "$APPDIR/usr/lib/" 2>/dev/null || cp -a "$lib" "$APPDIR/usr/lib/" 2>/dev/null || true
  done
}

echo "--- Bundling shared libraries (ldd) ---"
bundle_ldd_libs "$APPDIR/usr/bin/lepramim"

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
  done < <(find "$APPDIR/usr" -type f \( -name '*.so' -o -name '*.so.*' -o -name 'lepramim' \) -print0 2>/dev/null)
fi

if [[ -z "$LINUXDEPLOY" ]]; then LINUXDEPLOY="$BUILD_ROOT/linuxdeploy-x86_64.AppImage"; fi
if [[ ! -x "$LINUXDEPLOY" ]]; then
  mkdir -p "$(dirname "$LINUXDEPLOY")"
  curl -fsSL "$LINUXDEPLOY_URL" -o "$LINUXDEPLOY"
  chmod 0755 "$LINUXDEPLOY"
fi
command -v linuxdeploy >/dev/null 2>&1 && LINUXDEPLOY="$(command -v linuxdeploy)"

if [[ -z "$LINUXDEPLOY_PLUGIN_QT" ]]; then
  LINUXDEPLOY_PLUGIN_QT="$BUILD_ROOT/linuxdeploy-plugin-qt-x86_64.AppImage"
fi
if [[ ! -x "$LINUXDEPLOY_PLUGIN_QT" ]]; then
  mkdir -p "$(dirname "$LINUXDEPLOY_PLUGIN_QT")"
  curl -fsSL "$LINUXDEPLOY_PLUGIN_QT_URL" -o "$LINUXDEPLOY_PLUGIN_QT"
  chmod 0755 "$LINUXDEPLOY_PLUGIN_QT"
fi

# linuxdeploy discovers plugins next to itself or on PATH.
PLUGIN_DIR="$(dirname "$LINUXDEPLOY_PLUGIN_QT")"
export PATH="$PLUGIN_DIR:$PATH"
ln -sfn "$LINUXDEPLOY_PLUGIN_QT" "$PLUGIN_DIR/linuxdeploy-plugin-qt" 2>/dev/null || true

echo "--- linuxdeploy + Qt plugin ---"
REAL_QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake || true)}"
[[ -n "$REAL_QMAKE" ]] || die "qmake6/qmake not found"
QT_PLUGINS_DIR="$("$REAL_QMAKE" -query QT_INSTALL_PLUGINS 2>/dev/null || true)"
# Arch/KDE drops kimageformats (kimg_*) next to Qt's image plugins; those
# optional codecs often miss libs (libjxrglue) and abort the Qt deploy plugin.
FILTERED_QT_PLUGINS="$BUILD_ROOT/qt-plugins-filtered"
if [[ -n "$QT_PLUGINS_DIR" && -d "$QT_PLUGINS_DIR" ]]; then
  rm -rf "$FILTERED_QT_PLUGINS"
  mkdir -p "$FILTERED_QT_PLUGINS"
  cp -a "$QT_PLUGINS_DIR"/. "$FILTERED_QT_PLUGINS"/
  rm -f "$FILTERED_QT_PLUGINS"/imageformats/kimg_*
  QMAKE_WRAPPER="$BUILD_ROOT/qmake-filtered"
  cat > "$QMAKE_WRAPPER" <<EOF
#!/bin/sh
# linuxdeploy-plugin-qt parses \`qmake -query\` (no key) for QT_INSTALL_PLUGINS.
if [ "\$1" = "-query" ]; then
  if [ "\$2" = "QT_INSTALL_PLUGINS" ]; then
    printf '%s\\n' "$FILTERED_QT_PLUGINS"
    exit 0
  fi
  if [ -z "\$2" ]; then
    "$REAL_QMAKE" -query | sed "s|^QT_INSTALL_PLUGINS:.*|QT_INSTALL_PLUGINS:$FILTERED_QT_PLUGINS|"
    exit 0
  fi
fi
exec "$REAL_QMAKE" "\$@"
EOF
  chmod 0755 "$QMAKE_WRAPPER"
  export QMAKE="$QMAKE_WRAPPER"
else
  export QMAKE="$REAL_QMAKE"
fi
# Qt 6.11+ ships libqwayland.so; older trees used libqwayland-generic/egl.
SCAN_PLUGINS="${FILTERED_QT_PLUGINS:-$QT_PLUGINS_DIR}"
EXTRA_PLUGINS=""
for p in libqxcb.so libqwayland.so libqwayland-generic.so libqwayland-egl.so; do
  if [[ -n "$SCAN_PLUGINS" && -f "$SCAN_PLUGINS/platforms/$p" ]]; then
    EXTRA_PLUGINS="${EXTRA_PLUGINS:+$EXTRA_PLUGINS;}$p"
  fi
done
export EXTRA_PLATFORM_PLUGINS="$EXTRA_PLUGINS"
export QML_SOURCES_PATHS="${QML_SOURCES_PATHS:-$PROJECT_ROOT/qml}"
# cxx-qt registers app.lepramim inside the binary; give qmlimportscanner a stub.
QML_STUBS="$BUILD_ROOT/qml-stubs"
mkdir -p "$QML_STUBS/app/lepramim"
cat > "$QML_STUBS/app/lepramim/qmldir" <<'EOF'
module app.lepramim
singleton Theme 1.0 Theme.qml
AppController 1.0 AppController.qml
EOF
printf '%s\n' 'import QtQuick; QtObject {}' > "$QML_STUBS/app/lepramim/Theme.qml"
printf '%s\n' 'import QtQuick; QtObject {}' > "$QML_STUBS/app/lepramim/AppController.qml"
export QML_MODULES_PATHS="${QML_MODULES_PATHS:+$QML_MODULES_PATHS:}$QML_STUBS"
export EXTRA_QT_MODULES="${EXTRA_QT_MODULES:-svg}"
# linuxdeploy's bundled strip cannot handle Arch RELR (.relr.dyn) objects.
export NO_STRIP="${NO_STRIP:-true}"
echo "  QMAKE=$QMAKE"
echo "  EXTRA_PLATFORM_PLUGINS=$EXTRA_PLATFORM_PLUGINS"
echo "  QML_SOURCES_PATHS=$QML_SOURCES_PATHS"
echo "  QML_MODULES_PATHS=$QML_MODULES_PATHS"
echo "  NO_STRIP=$NO_STRIP"
set +e
APPIMAGE_EXTRACT_AND_RUN=1 "$LINUXDEPLOY" \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/lepramim" \
  --plugin qt \
  2>&1 | sed 's/^/  /'
ld_rc="${PIPESTATUS[0]}"
set -e
[[ "$ld_rc" -eq 0 ]] || die "linuxdeploy --plugin qt failed (exit $ld_rc)"

if [[ -z "$APPIMAGE_TOOL" ]]; then APPIMAGE_TOOL="$BUILD_ROOT/appimagetool-x86_64.AppImage"; fi
if [[ ! -x "$APPIMAGE_TOOL" ]]; then
  mkdir -p "$(dirname "$APPIMAGE_TOOL")"
  curl -fsSL "$APPIMAGE_TOOL_URL" -o "$APPIMAGE_TOOL"
  chmod 0755 "$APPIMAGE_TOOL"
fi
command -v appimagetool >/dev/null 2>&1 && APPIMAGE_TOOL="$(command -v appimagetool)"

mkdir -p "$OUTPUT_DIR"
OUTPUT="$OUTPUT_DIR/Lepramim-${VERSION}-x86_64.AppImage"
rm -f "$OUTPUT"
APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_TOOL" "$APPDIR" "$OUTPUT"
chmod 0755 "$OUTPUT"

echo "AppImage: $OUTPUT"
du -h "$OUTPUT" 2>/dev/null || true
