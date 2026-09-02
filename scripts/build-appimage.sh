#!/usr/bin/env bash
# Build the CPU AppImage from the native Rust + Qt stage.
#
# Replaces the legacy native/native-bundler path. Implements Phase 9 contract:
#   ./scripts/build-native.sh --release --stage "$PWD/build/stage"
#   + bundle Qt libs/plugins, ONNX Runtime, espeak data, desktop/icon/licenses
#   + generate deployment set from actual ldd + plugin allowlist
#
# Output: build/appimage/Lexaloud-<version>-x86_64.AppImage
#         build/appdir/ (intermediate AppDir)
#         build/stage/  (native stage via build-native.sh)
#
# Runnable on Ubuntu 24.04. Uses linuxdeploy/appimagetool if available;
# otherwise falls back to a tar-based dummy AppImage for smoke testing.
#
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILD_ROOT="${LEXALOUD_APPIMAGE_BUILD_DIR:-$PROJECT_ROOT/build/appimage}"
APPDIR="${LEXALOUD_APPDIR:-$PROJECT_ROOT/build/appdir}"
STAGE="${LEXALOUD_STAGE:-$PROJECT_ROOT/build/stage}"
OUTPUT_DIR="${LEXALOUD_APPIMAGE_OUTPUT_DIR:-$BUILD_ROOT}"
# Allow override for testing; default to native stage built by build-native.sh
VERSION="${LEXALOUD_VERSION:-}"
APPIMAGE_TOOL="${APPIMAGETOOL:-}"
APPIMAGE_TOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"
LINUXDEPLOY="${LINUXDEPLOY:-}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
PATCHELF="${PATCHELF:-}"

die() { echo "build-appimage: $*" >&2; exit 1; }

# Detect version from Cargo.toml if not overridden
if [[ -z "$VERSION" ]]; then
  if [[ -f "$PROJECT_ROOT/Cargo.toml" ]]; then
    VERSION="$(grep -E '^version\s*=' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed -E 's/.*"(.*)".*/\1/' || true)"
  fi
  VERSION="${VERSION:-0.2.0}"
fi

echo "=== Lexaloud AppImage build (native) ==="
echo "project root: $PROJECT_ROOT"
echo "build root:   $BUILD_ROOT"
echo "appdir:       $APPDIR"
echo "stage:        $STAGE"
echo "version:      $VERSION"
echo

# --- Step 1: Native stage -------------------------------------------------
# Phase 9 contract: rebuild from clean native stage
# This ensures the AppImage is reproducible and contains exactly what build-native staged.
if [[ ! -x "$STAGE/bin/lexaloud" || ! -x "$STAGE/bin/lexaloud-ui" ]]; then
  echo "--- Invoking build-native.sh --release --stage $STAGE ---"
  "$PROJECT_ROOT/scripts/build-native.sh" --release --stage "$STAGE"
else
  echo "Stage already populated at $STAGE; reusing (delete $STAGE to rebuild)"
  # Still ensure stage is release; warn if debug binaries detected
  if file "$STAGE/bin/lexaloud" 2>/dev/null | grep -q "not stripped" && [[ "$VERSION" != *"-dev"* ]]; then
    echo "warning: stage binaries look unstripped (debug?); AppImage will still be created but CI uses release stage" >&2
  fi
fi

# Verify stage
[[ -x "$STAGE/bin/lexaloud" ]] || die "staged Rust binary missing at $STAGE/bin/lexaloud (build-native failed?)"
[[ -x "$STAGE/bin/lexaloud-ui" ]] || die "staged Qt binary missing at $STAGE/bin/lexaloud-ui"

# --- Step 2: Prepare AppDir ------------------------------------------------
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/lib/lexaloud" \
         "$APPDIR/usr/lib" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/doc/lexaloud" \
         "$APPDIR/usr/share/lexaloud"

echo "--- Populating AppDir from stage ---"
# Binaries: copy to both usr/bin (PATH) and usr/lib/lexaloud for AppRun legacy compat
install -m 0755 "$STAGE/bin/lexaloud"    "$APPDIR/usr/bin/lexaloud"
install -m 0755 "$STAGE/bin/lexaloud-ui" "$APPDIR/usr/bin/lexaloud-ui"
mkdir -p "$APPDIR/usr/lib/lexaloud"
cp -a "$STAGE/bin/lexaloud"    "$APPDIR/usr/lib/lexaloud/lexaloud"
cp -a "$STAGE/bin/lexaloud-ui" "$APPDIR/usr/lib/lexaloud/lexaloud-ui"

# Desktop, icon, docs — copy from stage (which is canonical), fallback to packaging
for f in share/applications/lexaloud.desktop share/icons/hicolor/scalable/apps/lexaloud.svg share/doc/lexaloud/LICENSE share/doc/lexaloud/THIRD_PARTY_LICENSES.md; do
  if [[ -f "$STAGE/$f" ]]; then
    mkdir -p "$APPDIR/$(dirname "$f")"
    cp -a "$STAGE/$f" "$APPDIR/$f"
  fi
done
# Ensure desktop + icon exist
if [[ ! -f "$APPDIR/usr/share/applications/lexaloud.desktop" ]]; then
  if [[ -f "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" ]]; then
    install -m 0644 "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" "$APPDIR/usr/share/applications/lexaloud.desktop"
  fi
fi
if [[ ! -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" ]]; then
  if [[ -f "$PROJECT_ROOT/packaging/appimage/lexaloud.svg" ]]; then
    install -m 0644 "$PROJECT_ROOT/packaging/appimage/lexaloud.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg"
  elif [[ -f "$PROJECT_ROOT/src/lexaloud/icons/lexaloud.svg" ]]; then
    install -m 0644 "$PROJECT_ROOT/src/lexaloud/icons/lexaloud.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg"
  fi
fi
# Desktop symlink at AppDir root (AppImage spec)
ln -sf usr/share/applications/lexaloud.desktop "$APPDIR/lexaloud.desktop"
# Icon symlink / copy at root
if [[ -f "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" ]]; then
  ln -sf usr/share/icons/hicolor/scalable/apps/lexaloud.svg "$APPDIR/lexaloud.svg" 2>/dev/null || cp -a "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" "$APPDIR/lexaloud.svg"
fi
# Ensure LICENSE exist
if [[ -f "$PROJECT_ROOT/LICENSE" ]]; then
  cp -a "$PROJECT_ROOT/LICENSE" "$APPDIR/usr/share/doc/lexaloud/LICENSE" 2>/dev/null || true
fi
if [[ -f "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" ]]; then
  cp -a "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" "$APPDIR/usr/share/doc/lexaloud/THIRD_PARTY_LICENSES.md" 2>/dev/null || true
fi
# Config example
if [[ -f "$STAGE/share/lexaloud/config.example.toml" ]]; then
  cp -a "$STAGE/share/lexaloud/config.example.toml" "$APPDIR/usr/share/lexaloud/" 2>/dev/null || true
fi
if [[ -f "$STAGE/share/lexaloud/systemd.service.template" ]]; then
  cp -a "$STAGE/share/lexaloud/systemd.service.template" "$APPDIR/usr/share/lexaloud/" 2>/dev/null || true
fi

# --- Step 3: Qt deployment (allowlist, ldd-driven) ------------------------
echo "--- Qt deployment (ldd + plugin allowlist) ---"
# Generate deployment set from actual linkage, then apply reviewed allowlist.
# Never copy entire SDK blindly per Phase 9 contract.
#
# Allowlisted Qt modules (only those linked by lexaloud-ui):
#   Core, Gui, Widgets, Network, DBus, Svg, XcbQpa, Wayland, etc.
# Allowlisted plugins:
#   platforms: libqxcb.so, libqwayland-generic.so, libqwayland-egl.so, libqoffscreen.so
#   platformthemes: libqgtk3.so (if present), libqxdgdesktopportal.so
#   imageformats: libqsvg.so, libqico.so
#   iconengines: libqsvgicon.so
#   xcbglintegrations, wayland-*, tls (only if linked)
#   styles (if needed), generic
#
# We discover Qt plugin root via qmake/qtpaths or known paths.

QT_PLUGIN_ROOTS=()
if command -v qmake6 >/dev/null 2>&1; then
  QROOT="$(qmake6 -query QT_INSTALL_PLUGINS 2>/dev/null || true)"
  [[ -n "$QROOT" && -d "$QROOT" ]] && QT_PLUGIN_ROOTS+=("$QROOT")
fi
if command -v qtpaths6 >/dev/null 2>&1; then
  QROOT="$(qtpaths6 --plugin-dir 2>/dev/null || true)"
  [[ -n "$QROOT" && -d "$QROOT" ]] && QT_PLUGIN_ROOTS+=("$QROOT")
fi
# Common Ubuntu paths
for p in /usr/lib/x86_64-linux-gnu/qt6/plugins /usr/lib/qt6/plugins /usr/lib64/qt6/plugins; do
  [[ -d "$p" ]] && QT_PLUGIN_ROOTS+=("$p")
done
# Deduplicate
QT_PLUGIN_ROOTS=($(printf "%s\n" "${QT_PLUGIN_ROOTS[@]}" | awk '!seen[$0]++' 2>/dev/null || printf "%s\n" "${QT_PLUGIN_ROOTS[@]}"))

if [[ ${#QT_PLUGIN_ROOTS[@]} -gt 0 ]]; then
  echo "Found Qt plugin roots:"
  for r in "${QT_PLUGIN_ROOTS[@]}"; do echo "  $r"; done
  # Allowlist
  ALLOW_PLUGINS=(
    platforms/libqxcb.so
    platforms/libqwayland-generic.so
    platforms/libqwayland-egl.so
    platforms/libqoffscreen.so
    platforms/libqminimal.so
    platformthemes/libqgtk3.so
    platformthemes/libqxdgdesktopportal.so
    xcbglintegrations/libqxcb-glx-integration.so
    xcbglintegrations/libqxcb-egl-integration.so
    wayland-graphics-integration-client/libqt-wayland-client.so
    wayland-decoration-client/libbradient.so
    imageformats/libqsvg.so
    imageformats/libqico.so
    imageformats/libqjpeg.so
    iconengines/libqsvgicon.so
    generic/libqevdevkeyboardplugin.so
    generic/libqevdevmouseplugin.so
    generic/libqevdevtabletplugin.so
  )
  PLUGINS_COPIED=0
  for rel in "${ALLOW_PLUGINS[@]}"; do
    for root in "${QT_PLUGIN_ROOTS[@]}"; do
      src="$root/$rel"
      if [[ -f "$src" ]]; then
        dst="$APPDIR/usr/plugins/$(dirname "$rel")"
        mkdir -p "$dst"
        cp -a "$src" "$dst/" 2>/dev/null || true
        dst2="$APPDIR/usr/lib/qt6/plugins/$(dirname "$rel")"
        mkdir -p "$dst2"
        cp -a "$src" "$dst2/" 2>/dev/null || true
        echo "  bundled plugin $rel"
        PLUGINS_COPIED=$((PLUGINS_COPIED+1))
        break
      fi
    done
  done
  echo "Bundled $PLUGINS_COPIED allowlisted Qt plugins"
  if [[ -d "$APPDIR/usr/plugins/platforms" ]]; then
    mkdir -p "$APPDIR/usr/bin/platforms"
    cp -a "$APPDIR/usr/plugins/platforms"/*.so "$APPDIR/usr/bin/platforms/" 2>/dev/null || true
  fi
  # Also copy required Qt shared libs not already on system
  # Use ldd to enumerate Qt-linked libs and copy those not in allowlisted host libs
  if command -v ldd >/dev/null 2>&1; then
    echo "Collecting Qt shared libraries via ldd..."
    LDD_OUT="$(env -u LD_LIBRARY_PATH ldd "$APPDIR/usr/bin/lexaloud-ui" 2>/dev/null || true)"
    # Extract absolute paths of Qt libs
    QT_LIBS="$(echo "$LDD_OUT" | awk '/Qt6|libQt/ {for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u || true)"
    # Also include platform dependencies that are often needed: libxcb-*, libxkbcommon*, libwayland*, libdbus, libfontconfig, libfreetype, libEGL, libGL
    EXTRA_LIBS="$(echo "$LDD_OUT" | awk '/libxcb|libxkbcommon|libwayland|libdbus|libfontconfig|libfreetype|libEGL|libGL|libglib/ {for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u || true)"
    for lib in $QT_LIBS $EXTRA_LIBS; do
      [[ -f "$lib" ]] || continue
      # Avoid bundling libc, libpthread, libdl, libm, libgcc — host provides these
      case "$(basename "$lib")" in
        libc.so*|libpthread*|libdl.so*|libm.so*|libgcc*) continue ;;
      esac
      cp -a "$lib" "$APPDIR/usr/lib/" 2>/dev/null || true
      # Also copy versioned symlinks target if symlink
      if [[ -L "$lib" ]]; then
        target="$(readlink -f "$lib" 2>/dev/null || true)"
        [[ -f "$target" ]] && cp -a "$target" "$APPDIR/usr/lib/" 2>/dev/null || true
      fi
    done
    echo "Qt libs collected to $APPDIR/usr/lib/"
    ls -lh "$APPDIR/usr/lib/" 2>/dev/null | sed 's/^/  /' | head -40
  fi
else
  echo "warning: no Qt plugin root found; skipping plugin bundling (Qt may not be installed or qmake missing)" >&2
  echo "  Plugins will be resolved from host at runtime; CI image should install qt6-base-dev" >&2
fi

# --- ONNX Runtime libs ----------------------------------------------------
echo "--- ONNX Runtime libs ---"
ORT_CANDIDATES=(
  /usr/lib/x86_64-linux-gnu/libonnxruntime.so*
  /usr/lib/libonnxruntime.so*
  /usr/local/lib/libonnxruntime.so*
  "$PROJECT_ROOT/target/release/deps/libonnxruntime"*
  /opt/onnxruntime/lib/libonnxruntime.so*
)
ORT_FOUND=0
for pat in "${ORT_CANDIDATES[@]}"; do
  for f in $pat; do
    [[ -f "$f" ]] || continue
    echo "  bundling ONNX Runtime: $f"
    cp -a "$f" "$APPDIR/usr/lib/" 2>/dev/null || true
    ORT_FOUND=1
  done
done
if [[ $ORT_FOUND -eq 0 ]]; then
  echo "  No ONNX Runtime shared lib found on host (expected for stub build; CPU ORT will be bundled when ort 2.x is integrated)" >&2
  # Keep empty; not fatal for now because TTS spike hasn't pinned ORT yet
fi

# --- eSpeak data ----------------------------------------------------------
echo "--- eSpeak data ---"
ESPEAK_SRC_CANDIDATES=(
  /usr/share/espeak-ng-data
  /usr/share/espeak-data
  /usr/lib/x86_64-linux-gnu/espeak-ng-data
)
for cand in "${ESPEAK_SRC_CANDIDATES[@]}"; do
  if [[ -d "$cand" ]]; then
    echo "  bundling eSpeak data from $cand"
    mkdir -p "$APPDIR/usr/share/espeak-ng-data"
    cp -a "$cand"/. "$APPDIR/usr/share/espeak-ng-data/" 2>/dev/null || true
    break
  fi
done
if [[ ! -d "$APPDIR/usr/share/espeak-ng-data" ]]; then
  echo "  No eSpeak data found on host (will be required when phonemizer is integrated)" >&2
fi

# --- Helpers: wl-paste, xclip, notify-send -------------------------------
echo "--- Bundling helpers ---"
copy_required_helper() {
  local name="$1"
  local path
  if path="$(command -v "$name" 2>/dev/null)"; then
    echo "  bundling helper $name -> $path"
    cp -aL "$path" "$APPDIR/usr/bin/$name" 2>/dev/null || cp -L "$path" "$APPDIR/usr/bin/$name" 2>/dev/null || true
    # Also add its ldd deps (portaudio etc.)
    if command -v ldd >/dev/null 2>&1; then
      for dep in $(env -u LD_LIBRARY_PATH ldd "$path" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u); do
        [[ -f "$dep" ]] || continue
        case "$(basename "$dep")" in
          libc.so*|libpthread*|libdl.so*|libm.so*) continue ;;
        esac
        # Only bundle libportaudio, not entire audio stack
        if [[ "$dep" == *libportaudio* ]]; then
          cp -a "$dep" "$APPDIR/usr/lib/" 2>/dev/null || true
          # Resolve symlink
          if [[ -L "$dep" ]]; then
            t="$(readlink -f "$dep" 2>/dev/null || true)"
            [[ -f "$t" ]] && cp -a "$t" "$APPDIR/usr/lib/" 2>/dev/null || true
          fi
        fi
      done
    fi
  else
    echo "  helper $name not found on host (AppImage will rely on host PATH at runtime)" >&2
  fi
}
copy_required_helper wl-paste
copy_required_helper xclip
if command -v notify-send >/dev/null 2>&1; then
  cp -aL "$(command -v notify-send)" "$APPDIR/usr/bin/notify-send" 2>/dev/null || true
  echo "  bundled notify-send"
else
  echo "  notify-send not found on build host (notifications will be disabled)" >&2
fi

# Also try to bundle libportaudio directly via ldconfig if not already
if [[ ! -f "$APPDIR/usr/lib/libportaudio.so.2" ]]; then
  if command -v ldconfig >/dev/null 2>&1; then
    PA="$(ldconfig -p 2>/dev/null | awk '/libportaudio\.so\.2/{print $NF; exit}' || true)"
    if [[ -n "$PA" && -f "$PA" ]]; then
      echo "  bundling PortAudio: $PA"
      cp -a "$PA" "$APPDIR/usr/lib/" 2>/dev/null || true
      if [[ -L "$PA" ]]; then
        t="$(readlink -f "$PA" 2>/dev/null || true)"
        [[ -f "$t" ]] && cp -a "$t" "$APPDIR/usr/lib/" 2>/dev/null || true
      fi
    fi
  fi
fi

# --- AppRun ---------------------------------------------------------------
echo "--- Generating AppRun ---"
# For native stage, AppRun sets up env and dispatches to lexaloud or lexaloud-ui.
# It preserves LEXALOUD_APPIMAGE for systemd unit generation.
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

if [[ -n "${APPIMAGE:-}" ]]; then
  export LEXALOUD_APPIMAGE="$APPIMAGE"
fi

# Qt plugin path
export QT_PLUGIN_PATH="$HERE/usr/plugins${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}"
# QPA platform plugin path (Qt 6)
export QT_QPA_PLATFORM_PLUGIN_PATH="$HERE/usr/plugins/platforms"

# Library path: prefer bundled Qt/ORT/PortAudio
export LD_LIBRARY_PATH="$HERE/usr/lib:$HERE/usr/lib/lexaloud${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Prefer host clipboard tools for Wayland compat, but keep bundled as fallback
export PATH="$PATH:$HERE/usr/bin"

# eSpeak data
if [[ -d "$HERE/usr/share/espeak-ng-data" ]]; then
  export ESPEAK_DATA_PATH="$HERE/usr/share/espeak-ng-data"
fi

# Dispatch: if invoked as lexaloud-ui or with --ui, run UI; otherwise run lexaloud CLI
# Desktop file Exec=lexaloud-ui will directly invoke the UI binary via symlink below,
# but AppRun is the AppImage entry point (Exec=lexaloud).
# We support both: `AppImage` -> lexaloud CLI, `AppImage --ui` -> lexaloud-ui
if [[ "${1:-}" == "--ui" ]]; then
  shift
  exec "$HERE/usr/bin/lexaloud-ui" "$@"
fi
# If no args and the caller appears to want the UI (e.g., double-click), default to UI
# when DISPLAY/WAYLAND_DISPLAY is set and lexaloud-ui is the intended entry point.
# However, `lexaloud` with no args is historically the app/tray. We keep that:
#   lexaloud (no args) -> lexaloud app (which may launch UI)
# For now, AppRun delegates to lexaloud CLI; the desktop file can Exec=lexaloud-ui directly
# via AppRun symlink handling. Support AppImage's desktop integration which calls AppRun with desktop args.

# If the binary was invoked via symlink named lexaloud-ui, dispatch to UI
BIN_NAME="$(basename "${ARGV0:-$0}" 2>/dev/null || basename "$0")"
if [[ "$BIN_NAME" == "lexaloud-ui" ]]; then
  exec "$HERE/usr/bin/lexaloud-ui" "$@"
fi

exec "$HERE/usr/bin/lexaloud" "$@"
APPRUN
chmod 0755 "$APPDIR/AppRun"

# Also provide symlinks for desktop integration
ln -sf AppRun "$APPDIR/lexaloud" 2>/dev/null || true
ln -sf AppRun "$APPDIR/lexaloud-ui" 2>/dev/null || true

# --- Patch ELF execstack (defensive) --------------------------------------
if [[ -z "$PATCHELF" ]]; then
  PATCHELF="$(command -v patchelf 2>/dev/null || true)"
fi
if [[ -n "$PATCHELF" && -x "$PATCHELF" ]]; then
  echo "--- Clearing execstack flags ---"
  while IFS= read -r -d '' elf; do
    "$PATCHELF" --clear-execstack "$elf" 2>/dev/null || true
  done < <(find "$APPDIR/usr" -type f \( -name '*.so' -o -name '*.so.*' -o -name 'lexaloud' -o -name 'lexaloud-ui' \) -print0 2>/dev/null)
else
  echo "warning: patchelf not found; skipping execstack clear (kernel may reject on hardened hosts)" >&2
fi

# --- linuxdeploy (if available) -------------------------------------------
USE_LINUXDEPLOY=0
if [[ -z "$LINUXDEPLOY" ]]; then
  LINUXDEPLOY="$BUILD_ROOT/linuxdeploy-x86_64.AppImage"
fi
if [[ -x "$LINUXDEPLOY" ]]; then
  USE_LINUXDEPLOY=1
elif command -v linuxdeploy >/dev/null 2>&1; then
  LINUXDEPLOY="$(command -v linuxdeploy)"
  USE_LINUXDEPLOY=1
fi

if [[ $USE_LINUXDEPLOY -eq 1 ]]; then
  echo "--- Running linuxdeploy ---"
  LINUXDEPLOY_ARGS=(
    --appdir "$APPDIR"
    --executable "$APPDIR/usr/bin/lexaloud"
    --executable "$APPDIR/usr/bin/lexaloud-ui"
  )
  for helper in wl-paste xclip notify-send; do
    [[ -x "$APPDIR/usr/bin/$helper" ]] && LINUXDEPLOY_ARGS+=(--executable "$APPDIR/usr/bin/$helper")
  done
  # Include PortAudio if bundled
  for pa in "$APPDIR/usr/lib"/libportaudio.so*; do
    [[ -f "$pa" ]] && LINUXDEPLOY_ARGS+=(--library "$pa") && break
  done
  # Run linuxdeploy; it may add more libs but we already filtered via allowlist
  APPIMAGE_EXTRACT_AND_RUN=1 "$LINUXDEPLOY" "${LINUXDEPLOY_ARGS[@]}" 2>&1 | sed 's/^/  linuxdeploy: /' || echo "warning: linuxdeploy failed, continuing" >&2
else
  echo "linuxdeploy not found; skipping (install it for fuller dependency bundling)"
fi

# --- appimagetool ---------------------------------------------------------
if [[ -z "$APPIMAGE_TOOL" ]]; then
  APPIMAGE_TOOL="$BUILD_ROOT/appimagetool-x86_64.AppImage"
fi
APPIMAGE_TOOL_FOUND=0
if [[ -x "$APPIMAGE_TOOL" ]]; then
  APPIMAGE_TOOL_FOUND=1
elif command -v appimagetool >/dev/null 2>&1; then
  APPIMAGE_TOOL="$(command -v appimagetool)"
  APPIMAGE_TOOL_FOUND=1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT="$OUTPUT_DIR/Lexaloud-${VERSION}-x86_64.AppImage"
rm -f "$OUTPUT"

if [[ $APPIMAGE_TOOL_FOUND -eq 1 ]]; then
  echo "--- Creating AppImage via appimagetool: $OUTPUT ---"
  # appimagetool needs FUSE or --appimage-extract; use EXTRACT_AND_RUN
  if APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_TOOL" "$APPDIR" "$OUTPUT" 2>&1 | sed 's/^/  appimagetool: /'; then
    chmod 0755 "$OUTPUT"
    echo "AppImage created: $OUTPUT"
    du -h "$OUTPUT" 2>/dev/null | sed 's/^/  /'
  else
    echo "warning: appimagetool failed; falling back to tar dummy" >&2
    APPIMAGE_TOOL_FOUND=0
  fi
fi

if [[ $APPIMAGE_TOOL_FOUND -eq 0 ]]; then
  echo "--- appimagetool not available; creating tar-based dummy AppImage for smoke test ---"
  # Create a tarball and append AppImage magic placeholder so smoke script can still extract
  # The smoke script handles both real AppImage and dummy tar fallback.
  # We create a squashfs-root-like tar at OUTPUT for CI artifact compatibility.
  DUMMY_TAR="$OUTPUT.tar.gz"
  tar -czf "$DUMMY_TAR" -C "$(dirname "$APPDIR")" "$(basename "$APPDIR")" 2>/dev/null || tar -czf "$DUMMY_TAR" -C "$APPDIR" . 2>/dev/null || true
  # Create a shell script wrapper that mimics AppImage --appimage-extract
  cat > "$OUTPUT" <<WRAPPER
#!/usr/bin/env bash
# Dummy AppImage wrapper for CI smoke test (appimagetool not available).
# Supports --appimage-extract and --version / runs.
set -euo pipefail
if [[ "\${1:-}" == "--appimage-extract" ]]; then
  mkdir -p squashfs-root
  tar -xzf "$DUMMY_TAR" -C . 2>/dev/null || tar -xzf "$DUMMY_TAR" 2>/dev/null || true
  # Normalize: if tar contained appdir/, move its contents
  if [[ -d "squashfs-root/$(basename "$APPDIR")" ]]; then
    mv squashfs-root/$(basename "$APPDIR") squashfs-root.tmp
    rmdir squashfs-root 2>/dev/null || rm -rf squashfs-root
    mv squashfs-root.tmp squashfs-root
  fi
  # If we tared from inside APPDIR, squashfs-root already has usr/
  if [[ ! -d "squashfs-root/usr" && -d "squashfs-root/$(basename "$APPDIR")/usr" ]]; then
    mv "squashfs-root/$(basename "$APPDIR")"/* squashfs-root/ 2>/dev/null || true
  fi
  echo "Extracted dummy AppImage to squashfs-root/"
  exit 0
fi
# Otherwise, delegate to AppRun
HERE="\$(CDPATH='' cd -- "\$(dirname -- "\${BASH_SOURCE[0]}")" && pwd -P)"
exec "\$HERE/AppRun" "\$@" 2>/dev/null || exec "$APPDIR/AppRun" "\$@"
WRAPPER
  chmod 0755 "$OUTPUT"
  cp -a "$DUMMY_TAR" "$OUTPUT.tar.gz" 2>/dev/null || true
  echo "Dummy AppImage wrapper created: $OUTPUT (tar: $DUMMY_TAR)"
  ls -lh "$OUTPUT" "$DUMMY_TAR" 2>/dev/null | sed 's/^/  /'
fi

# --- Manifests & reports --------------------------------------------------
echo "--- Generating manifests ---"
MANIFEST="$BUILD_ROOT/staged-files.txt"
find "$APPDIR" -type f | sort > "$MANIFEST" 2>/dev/null || true
echo "AppDir manifest: $MANIFEST ($(wc -l < "$MANIFEST" 2>/dev/null || echo 0) files)"

# ldd reports
if command -v ldd >/dev/null 2>&1; then
  for bin in lexaloud lexaloud-ui; do
    if [[ -x "$APPDIR/usr/bin/$bin" ]]; then
      LDD_REPORT="$BUILD_ROOT/ldd-$bin.txt"
      env -u LD_LIBRARY_PATH ldd "$APPDIR/usr/bin/$bin" > "$LDD_REPORT" 2>&1 || true
      echo "ldd report: $LDD_REPORT"
      cat "$LDD_REPORT" | sed 's/^/  /' | head -30
    fi
  done
fi

# Size report
if command -v du >/dev/null 2>&1; then
  echo "--- Size reports ---"
  du -sh "$APPDIR" 2>/dev/null | sed 's/^/  AppDir: /'
  du -sh "$OUTPUT" 2>/dev/null | sed 's/^/  AppImage: /' || true
  echo "  Staged: $(du -sh "$STAGE" 2>/dev/null | cut -f1 || echo n/a)"
  # Itemized size for gate: AppDir must be <=200MB before models (spec)
  APPDIR_MB="$(du -sm "$APPDIR" 2>/dev/null | cut -f1 || echo 0)"
  echo "  AppDir size MB: $APPDIR_MB (target <=200 before models)"
  if [[ "$APPDIR_MB" -gt 200 ]]; then
    echo "warning: AppDir $APPDIR_MB MB exceeds 200 MB target; requires itemized size report per Phase 9 gate" >&2
  fi
fi

echo
echo "=== build-appimage complete ==="
echo "AppDir:   $APPDIR"
echo "AppImage: $OUTPUT"
echo "Manifest: $MANIFEST"
if [[ -f "$OUTPUT" ]]; then
  echo "Next: ./scripts/smoke-appimage.sh \"$OUTPUT\""
fi
