#!/usr/bin/env bash
# Lexaloud installer — native Rust + Iced (single binary).
#
# Installs lexaloud plus desktop, icon, and systemd template.
# icon, and systemd template. Supports:
#   - AppImage install (default for releases; no build tools required)
#   - Source build install (--from-source; requires Rust + GUI libs)
#
# This replaces the legacy venv installer. It must not create a
# venv, must not require an interpreter, and must preserve distro-specific
# runtime-dependency handling.
#
# Usage:
#   ./scripts/install.sh                              # auto-detect backend, prefer AppImage if present
#   ./scripts/install.sh --backend cpu                # force CPU
#   ./scripts/install.sh --backend cuda12             # force CUDA 12 (requires NVIDIA + CUDA runtime)
#   ./scripts/install.sh --backend auto               # auto via nvidia-smi
#   ./scripts/install.sh --prefix ~/.local            # install to prefix (default: ~/.local)
#   ./scripts/install.sh --system                     # install to /usr/local (requires sudo)
#   ./scripts/install.sh --from-source                # build from source (cargo)
#   ./scripts/install.sh --appimage dist/Lexaloud-*.AppImage  # install from AppImage
#   ./scripts/install.sh --with-math-speech           # also install speech-rule-engine (node >=18)
#
set -euo pipefail

REPO_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
PREFIX_DEFAULT="$HOME/.local"
PREFIX="${LEXALOUD_PREFIX:-$PREFIX_DEFAULT}"
BACKEND="auto"
WITH_MATH_SPEECH=0
FROM_SOURCE=0
APPIMAGE_PATH=""
SYSTEM=0

# --- parse arguments ----------------------------------------------------
while (( "$#" )); do
  case "$1" in
    --backend)
      BACKEND="$2"; shift 2 ;;
    --backend=*)
      BACKEND="${1#*=}"; shift ;;
    --prefix)
      PREFIX="$2"; shift 2 ;;
    --prefix=*)
      PREFIX="${1#*=}"; shift ;;
    --system)
      SYSTEM=1; PREFIX="/usr/local"; shift ;;
    --from-source)
      FROM_SOURCE=1; shift ;;
    --appimage)
      APPIMAGE_PATH="$2"; shift 2 ;;
    --appimage=*)
      APPIMAGE_PATH="${1#*=}"; shift ;;
    --with-math-speech)
      WITH_MATH_SPEECH=1; shift ;;
    -h|--help)
      sed -n '2,30p' "$0"
      echo
      echo "Examples:"
      echo "  $0 --backend cpu --prefix ~/.local"
      echo "  $0 --from-source --backend cpu"
      echo "  $0 --appimage dist/Lexaloud-0.2.0-x86_64.AppImage"
      echo "  $0 --system --backend auto"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Usage: $0 [--backend cpu|cuda12|auto] [--prefix <path>] [--system] [--from-source] [--appimage <path>] [--with-math-speech]" >&2
      exit 2
      ;;
  esac
done

if [[ "$BACKEND" != "cpu" && "$BACKEND" != "cuda12" && "$BACKEND" != "auto" ]]; then
  echo "Invalid --backend value: $BACKEND (must be cpu, cuda12, or auto)" >&2
  exit 2
fi

# Handle --system requires root for /usr/local
if [[ $SYSTEM -eq 1 && "$PREFIX" == "/usr/local" && $EUID -ne 0 ]]; then
  echo "Note: --system installs to /usr/local; you may need sudo" >&2
fi

echo "=== Lexaloud installer (native) ==="
echo "repo root: $REPO_ROOT"
echo "prefix:    $PREFIX"
echo "backend:   $BACKEND"
echo "from-source: $FROM_SOURCE"
if [[ -n "$APPIMAGE_PATH" ]]; then echo "appimage:  $APPIMAGE_PATH"; fi
echo

# --- distro detection ---------------------------------------------------
DISTRO_ID="unknown"
DISTRO_LIKE=""
if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  DISTRO_ID="$(. /etc/os-release && echo "${ID:-unknown}")"
  DISTRO_LIKE="$(. /etc/os-release && echo "${ID_LIKE:-}")"
fi
echo "distro: $DISTRO_ID (like: ${DISTRO_LIKE:-none})"

DISTRO_FAMILY="unknown"
case "$DISTRO_ID" in
  ubuntu|debian|linuxmint|pop|elementary|kali|zorin) DISTRO_FAMILY="debian" ;;
  fedora|rhel|centos|rocky|almalinux) DISTRO_FAMILY="fedora" ;;
  arch|manjaro|endeavouros|garuda|artix) DISTRO_FAMILY="arch" ;;
  opensuse*|sles) DISTRO_FAMILY="suse" ;;
esac
if [[ "$DISTRO_FAMILY" == "unknown" && -n "$DISTRO_LIKE" ]]; then
  case "$DISTRO_LIKE" in
    *debian*|*ubuntu*) DISTRO_FAMILY="debian" ;;
    *fedora*|*rhel*)   DISTRO_FAMILY="fedora" ;;
    *arch*)            DISTRO_FAMILY="arch" ;;
    *suse*)            DISTRO_FAMILY="suse" ;;
  esac
fi
echo "distro family: $DISTRO_FAMILY"
echo

# --- backend auto-detection --------------------------------------------
if [[ "$BACKEND" == "auto" ]]; then
  if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -L >/dev/null 2>&1; then
    BACKEND="cuda12"
    echo "nvidia-smi detected NVIDIA GPU → backend=cuda12"
  else
    BACKEND="cpu"
    echo "no NVIDIA GPU found → backend=cpu"
  fi
  echo
fi

# --- system dependency check (distro-aware, native) --------------------
# For AppImage install: only runtime helpers (wl-clipboard, xclip, portaudio, notify)
# For source build: plus toolchain (cargo, GTK/X11 libs)
missing_runtime=()
missing_build=()

if ! command -v wl-paste >/dev/null 2>&1; then
  missing_runtime+=("wl-clipboard")
fi
if ! command -v xclip >/dev/null 2>&1; then
  missing_runtime+=("xclip")
fi
# PortAudio probe via ldconfig
LDCONFIG_OUT="$(ldconfig -p 2>/dev/null || true)"
if [[ "$LDCONFIG_OUT" != *libportaudio.so.2* ]]; then
  missing_runtime+=("libportaudio2")
fi
unset LDCONFIG_OUT
if ! command -v notify-send >/dev/null 2>&1; then
  missing_runtime+=("libnotify-bin")
fi

# Build deps only if --from-source
if [[ $FROM_SOURCE -eq 1 ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    missing_build+=("cargo (rustup)")
  fi
  # GUI deps probe
  HAS_GUI=0
  if pkg-config --exists dbus-1 2>/dev/null; then HAS_GUI=1; fi
  if [[ $HAS_GUI -eq 0 ]]; then
    missing_build+=("libdbus-1-dev")
  fi
fi

# CUDA backend additional checks
if [[ "$BACKEND" == "cuda12" ]]; then
  if ! command -v nvidia-smi >/dev/null 2>&1; then
    echo "ERROR: --backend cuda12 selected but nvidia-smi not found. Install NVIDIA drivers or use --backend cpu." >&2
    exit 1
  fi
  # Check for CUDA runtime libs (best-effort via ldconfig)
  if ! ldconfig -p 2>/dev/null | grep -q "libcuda.so"; then
    echo "warning: CUDA backend selected but libcuda.so not in ldconfig; CUDA execution may fail at runtime" >&2
    echo "  Install CUDA 12 runtime from https://developer.nvidia.com/cuda-toolkit" >&2
  fi
fi

# Report missing deps with distro-specific install commands
all_missing=("${missing_runtime[@]}" "${missing_build[@]}")
if (( ${#all_missing[@]} > 0 )); then
  mapfile -t all_missing < <(printf "%s\n" "${all_missing[@]}" | awk '!seen[$0]++')
  echo "Missing system packages (conceptual names):" >&2
  for p in "${all_missing[@]}"; do echo "  - $p" >&2; done
  echo >&2
  case "$DISTRO_FAMILY" in
    debian)
      echo "Install runtime deps with:" >&2
      echo "  sudo apt install ${missing_runtime[*]:-wl-clipboard xclip libportaudio2 libnotify-bin}" >&2
      if (( ${#missing_build[@]} > 0 )); then
        echo "Install build deps with:" >&2
        echo "  sudo apt install libdbus-1-dev libasound2-dev pkg-config clang" >&2
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0" >&2
      fi
      ;;
    fedora)
      fedora_runtime=()
      for p in "${missing_runtime[@]}"; do
        case "$p" in
          wl-clipboard) fedora_runtime+=("wl-clipboard") ;;
          xclip)        fedora_runtime+=("xclip") ;;
          libportaudio2) fedora_runtime+=("portaudio") ;;
          libnotify-bin) fedora_runtime+=("libnotify") ;;
          *)            fedora_runtime+=("$p") ;;
        esac
      done
      echo "Install them with:" >&2
      echo "  sudo dnf install ${fedora_runtime[*]}" >&2
      if (( ${#missing_build[@]} > 0 )); then
        echo "  sudo dnf install dbus-devel clang" >&2
      fi
      ;;
    arch)
      arch_runtime=()
      for p in "${missing_runtime[@]}"; do
        case "$p" in
          wl-clipboard) arch_runtime+=("wl-clipboard") ;;
          xclip)        arch_runtime+=("xclip") ;;
          libportaudio2) arch_runtime+=("portaudio") ;;
          libnotify-bin) arch_runtime+=("libnotify") ;;
          *)            arch_runtime+=("$p") ;;
        esac
      done
      echo "Install them with:" >&2
      echo "  sudo pacman -S ${arch_runtime[*]}" >&2
      if (( ${#missing_build[@]} > 0 )); then
        echo "  sudo pacman -S dbus clang" >&2
      fi
      ;;
    suse)
      suse_runtime=()
      for p in "${missing_runtime[@]}"; do
        case "$p" in
          wl-clipboard) suse_runtime+=("wl-clipboard") ;;
          xclip)        suse_runtime+=("xclip") ;;
          libportaudio2) suse_runtime+=("portaudio") ;;
          libnotify-bin) suse_runtime+=("libnotify-tools") ;;
          *)            suse_runtime+=("$p") ;;
        esac
      done
      echo "Install them with:" >&2
      echo "  sudo zypper install ${suse_runtime[*]}" >&2
      ;;
    *)
      echo "Your distro ($DISTRO_ID) isn't in our table. The conceptual names above are what you need." >&2
      ;;
  esac
  echo >&2
  echo "Proceeding only if --from-source is not required; for AppImage installs, runtime deps are optional but recommended." >&2
  if [[ $FROM_SOURCE -eq 1 && ${#missing_build[@]} -gt 0 ]]; then
    exit 1
  fi
  if [[ ${#missing_runtime[@]} -gt 0 ]]; then
    echo "warning: some runtime helpers missing; clipboard and audio may not work until installed" >&2
  fi
  echo
fi

# --- Determine install source ------------------------------------------
STAGE="$REPO_ROOT/build/stage"
INSTALL_SRC=""
INSTALL_MODE=""

if [[ -n "$APPIMAGE_PATH" ]]; then
  # Explicit AppImage path (may be glob)
  # Expand glob if contains *
  if [[ "$APPIMAGE_PATH" == *"*"* ]]; then
    # shellcheck disable=SC2206
    EXPANDED=($APPIMAGE_PATH)
    APPIMAGE_PATH="${EXPANDED[0]}"
  fi
  if [[ ! -f "$APPIMAGE_PATH" ]]; then
    echo "ERROR: AppImage not found: $APPIMAGE_PATH" >&2
    exit 1
  fi
  INSTALL_SRC="$APPIMAGE_PATH"
  INSTALL_MODE="appimage"
elif [[ $FROM_SOURCE -eq 1 ]]; then
  INSTALL_MODE="source"
else
  # Auto: prefer existing AppImage in build/appimage or dist, else build from source
  CANDIDATE=""
  for pat in "$REPO_ROOT/build/appimage/Lexaloud-"*.AppImage "$REPO_ROOT/dist/Lexaloud-"*.AppImage "$REPO_ROOT/build/appimage/Lexaloud-"*.AppImage.tar.gz; do
    for f in $pat; do
      [[ -f "$f" && "$f" != *"*.AppImage"* ]] || continue
      CANDIDATE="$f"
      break 2
    done
  done
  if [[ -n "$CANDIDATE" ]]; then
    INSTALL_SRC="$CANDIDATE"
    INSTALL_MODE="appimage"
    echo "Found existing AppImage: $CANDIDATE (use --from-source to force source build)"
  else
    INSTALL_MODE="source"
    echo "No AppImage found; will build from source"
  fi
fi

echo "install mode: $INSTALL_MODE"
if [[ "$INSTALL_MODE" == "appimage" ]]; then
  echo "source: $INSTALL_SRC"
fi
echo

# --- Build from source if needed ---------------------------------------
if [[ "$INSTALL_MODE" == "source" ]]; then
  echo "--- Building native stage (source) ---"
  if [[ ! -x "$REPO_ROOT/scripts/build-native.sh" ]]; then
    echo "ERROR: scripts/build-native.sh not found or not executable" >&2
    exit 1
  fi
  "$REPO_ROOT/scripts/build-native.sh" --release --stage "$STAGE"
  if [[ ! -x "$STAGE/bin/lexaloud" ]]; then
    echo "ERROR: build-native failed to produce staged binary" >&2
    exit 1
  fi
  INSTALL_SRC="$STAGE"
  echo "Source build complete: $STAGE"
  echo
fi

# --- Install to prefix -------------------------------------------------
echo "--- Installing to $PREFIX ---"
mkdir -p "$PREFIX/bin"
mkdir -p "$PREFIX/share/applications"
mkdir -p "$PREFIX/share/icons/hicolor/scalable/apps"
mkdir -p "$PREFIX/share/doc/lexaloud"
mkdir -p "$PREFIX/share/lexaloud"

if [[ "$INSTALL_MODE" == "appimage" ]]; then
  # AppImage install: copy AppImage to prefix/bin and set up desktop integration
  APPIMAGE_NAME="$(basename "$INSTALL_SRC")"
  # If source is a dummy tar wrapper, still copy it
  TARGET_APPIMAGE="$PREFIX/bin/Lexaloud-x86_64.AppImage"
  # For release, use versioned name if available
  if [[ "$APPIMAGE_NAME" == Lexaloud-*.AppImage ]]; then
    TARGET_APPIMAGE="$PREFIX/bin/$APPIMAGE_NAME"
  fi
  echo "Copying AppImage to $TARGET_APPIMAGE"
  cp -a "$INSTALL_SRC" "$TARGET_APPIMAGE"
  chmod 0755 "$TARGET_APPIMAGE"
  # Create wrapper so `lexaloud` resolves to AppImage
  cat > "$PREFIX/bin/lexaloud" <<WRAP
#!/usr/bin/env bash
exec "$TARGET_APPIMAGE" "\$@"
WRAP
  chmod 0755 "$PREFIX/bin/lexaloud"
  echo "Created wrapper: $PREFIX/bin/lexaloud"

  # Try to extract desktop/icon from AppImage for prefix integration
  TMP_EXTRACT="$(mktemp -d)"
  trap 'rm -rf "$TMP_EXTRACT"' EXIT
  if "$TARGET_APPIMAGE" --appimage-extract 2>/dev/null; then
    # Dummy wrapper extracts to squashfs-root from current dir
    if [[ -d "squashfs-root" ]]; then
      mv squashfs-root "$TMP_EXTRACT/squashfs-root" 2>/dev/null || true
      rmdir squashfs-root 2>/dev/null || true
    fi
  fi
  # Fallback: check if AppImage is in AppDir mode already
  if [[ -d "./squashfs-root" ]]; then
    mv ./squashfs-root "$TMP_EXTRACT/" 2>/dev/null || true
  fi
  # If we have an extracted dir, copy desktop/icon
  EXTRACTED=""
  if [[ -d "$TMP_EXTRACT/squashfs-root" ]]; then
    EXTRACTED="$TMP_EXTRACT/squashfs-root"
  elif [[ -d "$TMP_EXTRACT" && -f "$TMP_EXTRACT/AppRun" ]]; then
    EXTRACTED="$TMP_EXTRACT"
  fi
  if [[ -n "$EXTRACTED" ]]; then
    if [[ -f "$EXTRACTED/usr/share/applications/lexaloud.desktop" ]]; then
      cp -a "$EXTRACTED/usr/share/applications/lexaloud.desktop" "$PREFIX/share/applications/lexaloud.desktop"
      # Fix Exec to point to installed wrapper
      sed -i "s|^Exec=.*|Exec=$PREFIX/bin/lexaloud|" "$PREFIX/share/applications/lexaloud.desktop" 2>/dev/null || true
      echo "Installed desktop file from AppImage"
    fi
    if [[ -f "$EXTRACTED/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" ]]; then
      cp -a "$EXTRACTED/usr/share/icons/hicolor/scalable/apps/lexaloud.svg" "$PREFIX/share/icons/hicolor/scalable/apps/lexaloud.svg"
      echo "Installed icon from AppImage"
    fi
    for f in LICENSE THIRD_PARTY_LICENSES.md; do
      if [[ -f "$EXTRACTED/usr/share/doc/lexaloud/$f" ]]; then
        cp -a "$EXTRACTED/usr/share/doc/lexaloud/$f" "$PREFIX/share/doc/lexaloud/$f"
      fi
    done
  fi
  rm -rf "$TMP_EXTRACT" 2>/dev/null || true
  trap - EXIT
  # Fallback if extraction failed: copy from repo
  if [[ ! -f "$PREFIX/share/applications/lexaloud.desktop" && -f "$REPO_ROOT/packaging/appimage/lexaloud.desktop" ]]; then
    cp -a "$REPO_ROOT/packaging/appimage/lexaloud.desktop" "$PREFIX/share/applications/lexaloud.desktop"
    echo "Installed desktop file from repo fallback"
  fi
  if [[ ! -f "$PREFIX/share/icons/hicolor/scalable/apps/lexaloud.svg" ]]; then
    for cand in "$REPO_ROOT/packaging/appimage/lexaloud.svg" "$REPO_ROOT/src/lexaloud/icons/lexaloud.svg"; do
      if [[ -f "$cand" ]]; then
        cp -a "$cand" "$PREFIX/share/icons/hicolor/scalable/apps/lexaloud.svg"
        echo "Installed icon from $cand"
        break
      fi
    done
  fi
else
  # Source stage install: copy binaries and assets directly
  echo "Copying staged binaries to $PREFIX/bin/"
  install -m 0755 "$STAGE/bin/lexaloud" "$PREFIX/bin/lexaloud"

  if [[ -f "$STAGE/share/applications/lexaloud.desktop" ]]; then
    install -m 0644 "$STAGE/share/applications/lexaloud.desktop" "$PREFIX/share/applications/lexaloud.desktop"
    # Fix Exec to installed path
    sed -i "s|^Exec=lexaloud.*|Exec=$PREFIX/bin/lexaloud|" "$PREFIX/share/applications/lexaloud.desktop" 2>/dev/null || true
  elif [[ -f "$REPO_ROOT/packaging/appimage/lexaloud.desktop" ]]; then
    install -m 0644 "$REPO_ROOT/packaging/appimage/lexaloud.desktop" "$PREFIX/share/applications/lexaloud.desktop"
    sed -i "s|^Exec=.*|Exec=$PREFIX/bin/lexaloud|" "$PREFIX/share/applications/lexaloud.desktop" 2>/dev/null || true
  fi

  if [[ -f "$STAGE/share/icons/hicolor/scalable/apps/lexaloud.svg" ]]; then
    install -m 0644 "$STAGE/share/icons/hicolor/scalable/apps/lexaloud.svg" "$PREFIX/share/icons/hicolor/scalable/apps/lexaloud.svg"
  fi
  if [[ -f "$REPO_ROOT/LICENSE" ]]; then
    install -m 0644 "$REPO_ROOT/LICENSE" "$PREFIX/share/doc/lexaloud/LICENSE" 2>/dev/null || true
  fi
  if [[ -f "$REPO_ROOT/THIRD_PARTY_LICENSES.md" ]]; then
    install -m 0644 "$REPO_ROOT/THIRD_PARTY_LICENSES.md" "$PREFIX/share/doc/lexaloud/THIRD_PARTY_LICENSES.md" 2>/dev/null || true
  fi
  if [[ -f "$STAGE/share/lexaloud/config.example.toml" ]]; then
    install -m 0644 "$STAGE/share/lexaloud/config.example.toml" "$PREFIX/share/lexaloud/config.example.toml"
  elif [[ -f "$REPO_ROOT/src/lexaloud/templates/config.example.toml" ]]; then
    install -m 0644 "$REPO_ROOT/src/lexaloud/templates/config.example.toml" "$PREFIX/share/lexaloud/config.example.toml"
  fi
  if [[ -f "$STAGE/share/lexaloud/systemd.service.template" ]]; then
    install -m 0644 "$STAGE/share/lexaloud/systemd.service.template" "$PREFIX/share/lexaloud/systemd.service.template"
  elif [[ -f "$REPO_ROOT/src/lexaloud/templates/systemd.service.template" ]]; then
    install -m 0644 "$REPO_ROOT/src/lexaloud/templates/systemd.service.template" "$PREFIX/share/lexaloud/systemd.service.template"
  fi
fi

# Update desktop database and icons if tools available (optional, not fatal)
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
fi

echo "Installed binaries:"
ls -lh "$PREFIX/bin/lexaloud" 2>/dev/null | sed 's/^/  /'

# --- optional: Speech Rule Engine for LaTeX-to-speech ------------------
if (( WITH_MATH_SPEECH == 1 )); then
  echo
  echo "--- installing Speech Rule Engine (SRE) for LaTeX-to-speech ---"
  if ! command -v node >/dev/null 2>&1; then
    cat >&2 <<NODE_MISSING
ERROR: --with-math-speech requires node (>=18).
Install it with one of:
  sudo apt install nodejs npm          # Debian/Ubuntu
  sudo dnf install nodejs npm          # Fedora
  sudo pacman -S nodejs npm            # Arch
NODE_MISSING
    exit 1
  fi
  if ! command -v npm >/dev/null 2>&1; then
    cat >&2 <<NPM_MISSING
ERROR: --with-math-speech requires npm.
Install it with one of:
  sudo apt install npm
  sudo dnf install npm
  sudo pacman -S npm
NPM_MISSING
    exit 1
  fi
  NODE_MAJOR="$(node -p 'Number(process.versions.node.split(".")[0])' 2>/dev/null || echo 0)"
  if (( NODE_MAJOR < 18 )); then
    echo "ERROR: --with-math-speech requires Node.js >= 18 (found major=$NODE_MAJOR)" >&2
    exit 1
  fi
  SRE_PREFIX="$PREFIX/share/lexaloud/sre"
  echo "installing speech-rule-engine@4.1.3 into $SRE_PREFIX"
  mkdir -p "$SRE_PREFIX"
  npm install --prefix "$SRE_PREFIX" speech-rule-engine@4.1.3
  SRE_BIN="$SRE_PREFIX/node_modules/.bin/sre"
  if [[ ! -x "$SRE_BIN" ]]; then
    echo "ERROR: expected sre binary not found at $SRE_BIN" >&2
    exit 1
  fi
  ln -sf "$SRE_BIN" "$PREFIX/bin/sre"
  echo "symlinked: $PREFIX/bin/sre -> $SRE_BIN"
  echo
  echo "Enable via config.toml:"
  echo "  [sre_latex]"
  echo "  enabled = true"
  echo "  domain = \"clearspeak\"    # or \"mathspeak\""
fi

echo
echo "=== install complete ==="
echo "prefix:  $PREFIX"
echo "backend: $BACKEND"
if (( WITH_MATH_SPEECH == 1 )); then
  echo "math-speech: speech-rule-engine@4.1.3 installed"
fi
echo
# Safety: ensure prefix bin is in PATH or tell user
if [[ ":$PATH:" != *":$PREFIX/bin:"* ]]; then
  echo "Add to your PATH if needed:"
  echo "  export PATH=\"\$PATH:$PREFIX/bin\""
  echo "Or symlink:"
  echo "  ln -sf $PREFIX/bin/lexaloud ~/.local/bin/lexaloud"
  echo
fi
echo "Next:"
echo "  $PREFIX/bin/lexaloud setup        # download models, create systemd unit"
echo "  $PREFIX/bin/lexaloud app          # tray UI (requires display server)"
echo
if [[ "$PREFIX" == "$HOME/.local" ]]; then
  echo "Systemd unit will be at ~/.config/systemd/user/lexaloud.service"
  echo "Activate with:"
  echo "  systemctl --user daemon-reload"
  echo "  systemctl --user enable --now lexaloud.service"
fi
