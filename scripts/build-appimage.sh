#!/usr/bin/env bash
# Build the CPU AppImage.
#
# The build environment is disposable and exists only while producing the
# artifact.  The resulting AppImage contains a PyInstaller onedir bundle,
# including its own Python interpreter and the locked CPU TTS dependencies.
# Model weights remain in the user's XDG cache and are downloaded on demand.

set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
BUILD_ROOT="${LEXALOUD_APPIMAGE_BUILD_DIR:-$PROJECT_ROOT/build/appimage}"
OUTPUT_DIR="${LEXALOUD_APPIMAGE_OUTPUT_DIR:-$PROJECT_ROOT/dist}"
BUILD_PYTHON="${LEXALOUD_BUILD_PYTHON:-}"
PYINSTALLER_SPEC="$PROJECT_ROOT/packaging/lexaloud.spec"
APPIMAGE_TOOL="${APPIMAGETOOL:-}"
APPIMAGE_TOOL_URL="${APPIMAGETOOL_URL:-https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage}"
LINUXDEPLOY="${LINUXDEPLOY:-}"
LINUXDEPLOY_URL="${LINUXDEPLOY_URL:-https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage}"
PATCHELF="${PATCHELF:-}"

die() {
  echo "build-appimage: $*" >&2
  exit 1
}

if [[ -z "$BUILD_PYTHON" ]]; then
  for candidate in python3.12 python3; do
    if command -v "$candidate" >/dev/null 2>&1; then
      BUILD_PYTHON="$(command -v "$candidate")"
      break
    fi
  done
fi

[[ -n "$BUILD_PYTHON" ]] || die "Python 3.11+ is required for the build environment"
[[ -x "$BUILD_PYTHON" ]] || die "build Python is not executable: $BUILD_PYTHON"
if [[ -z "$PATCHELF" ]]; then
  PATCHELF="$(command -v patchelf || true)"
fi
[[ -x "$PATCHELF" ]] || die "patchelf is required to clear executable-stack flags from bundled Python libraries"

PY_VERSION="$($BUILD_PYTHON -c 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")')"
PY_MAJOR="${PY_VERSION%%.*}"
PY_MINOR="${PY_VERSION#*.}"
if (( PY_MAJOR < 3 || (PY_MAJOR == 3 && PY_MINOR < 11) )); then
  die "build Python $PY_VERSION is too old; Lexaloud requires Python >= 3.11"
fi

rm -rf "$BUILD_ROOT"
mkdir -p "$BUILD_ROOT" "$OUTPUT_DIR"

VENV="$BUILD_ROOT/venv"
APPDIR="$BUILD_ROOT/AppDir"
PYINSTALLER_DIST="$BUILD_ROOT/pyinstaller-dist"
PYINSTALLER_WORK="$BUILD_ROOT/pyinstaller-work"
LOCK="$PROJECT_ROOT/requirements-lock.cpu.txt"
LOCK_NO_KOKORO="$BUILD_ROOT/requirements-no-kokoro.txt"
KOKORO_REQ="$BUILD_ROOT/requirements-kokoro.txt"

echo "Creating build venv with Python $PY_VERSION"
"$BUILD_PYTHON" -m venv "$VENV"
VENV_PYTHON="$VENV/bin/python"

echo "Installing locked CPU runtime dependencies"
"$VENV_PYTHON" -m pip install --upgrade pip >/dev/null

# Keep the same kokoro-onnx installation shape as scripts/install.sh.  The
# lock file contains the complete, hash-pinned dependency set.  Install it
# without dependency resolution: the current lock has an intentionally
# complete set, and resolving it again would reject the pinned csvw/rfc3986
# pair before the application can even be packaged.
awk '/^kokoro-onnx==/{skip=1; next} /^[^ \t]/{skip=0} !skip' "$LOCK" > "$LOCK_NO_KOKORO"
awk '/^kokoro-onnx==/{found=1} found{print} found && !/\\$/{exit}' "$LOCK" > "$KOKORO_REQ"

"$VENV_PYTHON" -m pip install --no-deps --require-hashes -r "$LOCK_NO_KOKORO"
"$VENV_PYTHON" -m pip install --no-deps --require-hashes -r "$KOKORO_REQ"
"$VENV_PYTHON" -m pip install --no-deps "$PROJECT_ROOT"

echo "Installing PyInstaller build tool"
"$VENV_PYTHON" -m pip install 'pyinstaller>=6.10,<7'

echo "Freezing Python runtime and application"
"$VENV_PYTHON" -m PyInstaller \
  --clean \
  --noconfirm \
  --distpath "$PYINSTALLER_DIST" \
  --workpath "$PYINSTALLER_WORK" \
  "$PYINSTALLER_SPEC"

mkdir -p \
  "$APPDIR/usr/lib/lexaloud" \
  "$APPDIR/usr/bin" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
  "$APPDIR/usr/share/doc/lexaloud"
cp -a "$PYINSTALLER_DIST/lexaloud/." "$APPDIR/usr/lib/lexaloud/"

# Some Python distributions ship libpython with an executable-stack flag.
# Recent kernels reject loading that library from an AppImage, even though
# the application itself does not need an executable stack.  Clear the flag
# before linuxdeploy inspects the bundle; this is also applied to extension
# modules defensively.
while IFS= read -r -d '' elf; do
  "$PATCHELF" --clear-execstack "$elf"
done < <(find "$APPDIR/usr/lib/lexaloud" -type f \( -name '*.so' -o -name '*.so.*' -o -name 'lexaloud' \) -print0)

install -m 0755 "$PROJECT_ROOT/packaging/appimage/AppRun" "$APPDIR/AppRun"
install -m 0644 "$PROJECT_ROOT/packaging/appimage/lexaloud.desktop" \
  "$APPDIR/usr/share/applications/lexaloud.desktop"
install -m 0644 "$PROJECT_ROOT/packaging/appimage/lexaloud.svg" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps/lexaloud.svg"
ln -s usr/share/applications/lexaloud.desktop "$APPDIR/lexaloud.desktop"
cp "$PROJECT_ROOT/LICENSE" "$APPDIR/usr/share/doc/lexaloud/LICENSE"
cp "$PROJECT_ROOT/THIRD_PARTY_LICENSES.md" "$APPDIR/usr/share/doc/lexaloud/THIRD_PARTY_LICENSES.md"

# Bundle the display/audio client tools.  The AppImage must not depend on a
# distribution-specific package install for the two clipboard paths: wl-paste
# is mandatory on Wayland and xclip keeps the same image usable on X11.  The
# tools still talk to the host's Wayland/X11 and audio sessions at runtime,
# which is the desktop integration boundary an AppImage cannot replace.
copy_required_helper() {
  local name="$1"
  local path
  if ! path="$(command -v "$name")"; then
    die "required helper '$name' is missing from the build host (install wl-clipboard and xclip)"
  fi
  cp -L "$path" "$APPDIR/usr/bin/$name"
}

copy_required_helper wl-paste
copy_required_helper xclip

# Notifications are best-effort in the application, so the image remains
# useful on builders without libnotify.  The release runner installs it and
# therefore includes it in published images.
if command -v notify-send >/dev/null 2>&1; then
  cp -L "$(command -v notify-send)" "$APPDIR/usr/bin/notify-send"
else
  echo "warning: notify-send is missing; desktop notifications will be disabled"
fi

if [[ -z "$LINUXDEPLOY" ]]; then
  LINUXDEPLOY="$BUILD_ROOT/linuxdeploy-x86_64.AppImage"
fi
if [[ ! -x "$LINUXDEPLOY" ]]; then
  echo "Downloading linuxdeploy"
  curl --fail --location --retry 3 --silent --show-error \
    "$LINUXDEPLOY_URL" -o "$LINUXDEPLOY"
  chmod 0755 "$LINUXDEPLOY"
fi

LINUXDEPLOY_ARGS=(
  --appdir "$APPDIR"
  --executable "$APPDIR/usr/lib/lexaloud/lexaloud"
)
for helper in wl-paste xclip notify-send; do
  if [[ -x "$APPDIR/usr/bin/$helper" ]]; then
    LINUXDEPLOY_ARGS+=(--executable "$APPDIR/usr/bin/$helper")
  fi
done

# Include PortAudio when available so the Python sounddevice binding does not
# require libportaudio from the host.  PipeWire/PulseAudio/ALSA session
# backends remain host services and are intentionally not copied wholesale.
LDCONFIG_CACHE="$BUILD_ROOT/ldconfig.txt"
ldconfig -p > "$LDCONFIG_CACHE" 2>/dev/null || true
PORTAUDIO="$(awk '/libportaudio\.so\.2/{print $NF; exit}' "$LDCONFIG_CACHE")"
if [[ -n "$PORTAUDIO" && -f "$PORTAUDIO" ]]; then
  LINUXDEPLOY_ARGS+=(--library "$PORTAUDIO")
fi

echo "Collecting AppImage runtime libraries"
APPIMAGE_EXTRACT_AND_RUN=1 "$LINUXDEPLOY" "${LINUXDEPLOY_ARGS[@]}"

if [[ -z "$APPIMAGE_TOOL" ]]; then
  APPIMAGE_TOOL="$BUILD_ROOT/appimagetool-x86_64.AppImage"
fi
if [[ ! -x "$APPIMAGE_TOOL" ]]; then
  echo "Downloading appimagetool"
  curl --fail --location --retry 3 --silent --show-error \
    "$APPIMAGE_TOOL_URL" -o "$APPIMAGE_TOOL"
  chmod 0755 "$APPIMAGE_TOOL"
fi

VERSION="$($VENV_PYTHON -c 'import lexaloud; print(lexaloud.__version__)')"
OUTPUT="$OUTPUT_DIR/Lexaloud-${VERSION}-x86_64.AppImage"
rm -f "$OUTPUT"

echo "Creating $OUTPUT"
APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_TOOL" "$APPDIR" "$OUTPUT"
chmod 0755 "$OUTPUT"

echo "AppImage created: $OUTPUT"
du -h "$OUTPUT"
