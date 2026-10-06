#!/usr/bin/env bash
# Lepramim portable single-file build.
#
# Produces one self-extracting executable:
#   build/portable/Lepramim-<version>-x86_64-portable.run
#
# The .run file embeds the release binary plus its full shared-library
# closure (Qt, ONNX Runtime, ALSA, ...). On first launch it extracts to
# $XDG_CACHE_HOME/lepramim-portable/ and execs the app; later launches
# reuse the extracted tree. No install, no root, no system dependencies
# beyond a kernel + libc + GPU stack.
#
# For "runs everywhere" the bundle MUST be built on an old glibc baseline
# (Ubuntu 22.04, glibc 2.35). Use --container for that (requires docker).
# A native build works but inherits this machine's glibc floor.
#
# Usage:
#   ./scripts/build-portable.sh [--container] [--rebuild-image]
#                               [--stage <abs-path>] [--output-dir <abs-path>]
set -euo pipefail

PROJECT_ROOT="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck source=scripts/lib/sanitize-host-env.sh
source "$PROJECT_ROOT/scripts/lib/sanitize-host-env.sh"
sanitize_host_env

CONTAINER=0
REBUILD_IMAGE=0
STAGE=""
OUTPUT_DIR=""
IMAGE="${LEPRAMIM_PORTABLE_IMAGE:-lepramim-portable-builder:22.04}"

usage() {
  cat <<EOF
Usage: $0 [--container] [--rebuild-image] [--stage <abs-path>] [--output-dir <abs-path>]

  --container          Build inside the Ubuntu 22.04 (glibc 2.35) container.
                       Required for a bundle that runs on older distros.
  --rebuild-image      Rebuild the container image even if present.
  --stage <path>       Absolute path to the native stage dir
                       (default: \$PROJECT_ROOT/build/stage).
  --output-dir <path>  Absolute path for the .run output
                       (default: \$PROJECT_ROOT/build/portable).
  -h, --help           Show this help.
EOF
}

while (($#)); do
  case "$1" in
    --container) CONTAINER=1; shift ;;
    --rebuild-image) REBUILD_IMAGE=1; shift ;;
    --stage)
      [[ $# -ge 2 ]] || { echo "error: --stage requires an argument" >&2; exit 2; }
      STAGE="$2"; shift 2 ;;
    --stage=*) STAGE="${1#*=}"; shift ;;
    --output-dir)
      [[ $# -ge 2 ]] || { echo "error: --output-dir requires an argument" >&2; exit 2; }
      OUTPUT_DIR="$2"; shift 2 ;;
    --output-dir=*) OUTPUT_DIR="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

die() { echo "build-portable: $*" >&2; exit 1; }

# ---------------------------------------------------------------- container
if [[ "$CONTAINER" -eq 1 ]]; then
  command -v docker >/dev/null 2>&1 || die "docker not found (needed for --container)"
  if [[ "$REBUILD_IMAGE" -eq 1 ]] || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "--- Building container image $IMAGE ---"
    docker build -f "$PROJECT_ROOT/packaging/portable/Containerfile" \
      -t "$IMAGE" "$PROJECT_ROOT/packaging/portable/"
  fi
  INNER_ARGS=()
  if [[ -n "$STAGE" ]]; then
    [[ "$STAGE" == "$PROJECT_ROOT"/* ]] || die "--stage with --container must be under $PROJECT_ROOT"
    INNER_ARGS+=(--stage "/workspace/${STAGE#"$PROJECT_ROOT"/}")
  fi
  if [[ -n "$OUTPUT_DIR" ]]; then
    [[ "$OUTPUT_DIR" == "$PROJECT_ROOT"/* ]] || die "--output-dir with --container must be under $PROJECT_ROOT"
    INNER_ARGS+=(--output-dir "/workspace/${OUTPUT_DIR#"$PROJECT_ROOT"/}")
  fi
  echo "--- Building portable bundle inside $IMAGE ---"
  docker run --rm \
    -v "$PROJECT_ROOT:/workspace" \
    -v lepramim-portable-cargo:/opt/cargo \
    -v lepramim-portable-target:/workspace-target \
    -v lepramim-portable-home-cache:/root/.cache \
    -e CARGO_TARGET_DIR=/workspace-target \
    -e RUSTUP_TOOLCHAIN=1.85.0 \
    ${LEPRAMIM_VERSION:+-e LEPRAMIM_VERSION="$LEPRAMIM_VERSION"} \
    -e LEPRAMIM_CHOWN_TO="$(id -u):$(id -g)" \
    -w /workspace "$IMAGE" ./scripts/build-portable.sh "${INNER_ARGS[@]}"
  exit $?
fi

# ------------------------------------------------------------- native build
VERSION="${LEPRAMIM_VERSION:-}"
if [[ -z "$VERSION" ]]; then
  VERSION="$(grep -E '^version\s*=' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/' || true)"
  VERSION="${VERSION:-0.2.0}"
fi

[[ -z "$STAGE" ]] && STAGE="$PROJECT_ROOT/build/stage"
[[ -z "$OUTPUT_DIR" ]] && OUTPUT_DIR="$PROJECT_ROOT/build/portable"
[[ "$STAGE" == /* ]] || die "--stage must be absolute: $STAGE"
[[ "$OUTPUT_DIR" == /* ]] || die "--output-dir must be absolute: $OUTPUT_DIR"

PAYLOAD="$OUTPUT_DIR/payload"
OUTPUT="$OUTPUT_DIR/Lepramim-${VERSION}-x86_64-portable.run"

echo "=== Lepramim portable build ==="
echo "version: $VERSION"
echo "stage:   $STAGE"
echo "payload: $PAYLOAD"
echo "output:  $OUTPUT"
echo

for tool in patchelf ldd readelf tar gzip file strings sha256sum awk; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool missing: $tool"
done
QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake || true)}"
[[ -n "$QMAKE" ]] || die "qmake6/qmake not found"

# Always re-stage (never reuse a possibly stale stage: cargo's own
# incrementality makes a fresh build a no-op, so this costs seconds).
"$PROJECT_ROOT/scripts/build-native.sh" --release --stage "$STAGE"
[[ -x "$STAGE/bin/lepramim" ]] || die "staged binary missing at $STAGE/bin/lepramim"

rm -rf "$PAYLOAD"
mkdir -p "$PAYLOAD/bin" "$PAYLOAD/lib" "$PAYLOAD/plugins" "$PAYLOAD/qml" "$PAYLOAD/share"

install -m 0755 "$STAGE/bin/lepramim" "$PAYLOAD/bin/lepramim"
[[ -d "$STAGE/share" ]] && cp -a "$STAGE/share/." "$PAYLOAD/share/" 2>/dev/null || true

is_elf() {
  [[ -f "$1" ]] && file -b "$1" 2>/dev/null | grep -q ELF
}

# --- helper binaries (resolved via PATH at runtime; ours are appended) ---
echo "--- Helper binaries ---"
for helper in espeak-ng wl-paste xclip notify-send; do
  path="$(command -v "$helper" 2>/dev/null || true)"
  if [[ -n "$path" ]]; then
    cp -aL "$path" "$PAYLOAD/bin/$helper" 2>/dev/null || cp -a "$path" "$PAYLOAD/bin/$helper"
    chmod 0755 "$PAYLOAD/bin/$helper"
    echo "  bundled $helper"
  else
    echo "  WARNING: helper $helper not on build host; runtime falls back to system PATH"
  fi
done

# --- espeak-ng voice data (launcher exports ESPEAK_DATA_PATH) ---
ESPEAK_FOUND=""
for cand in /usr/share/espeak-ng-data /usr/share/espeak-data \
             /usr/lib/x86_64-linux-gnu/espeak-ng-data /usr/lib/espeak-ng-data; do
  if [[ -d "$cand" ]]; then
    mkdir -p "$PAYLOAD/share/espeak-ng-data"
    cp -a "$cand/." "$PAYLOAD/share/espeak-ng-data/" 2>/dev/null || true
    echo "--- eSpeak data: $cand ---"
    ESPEAK_FOUND="$cand"
    break
  fi
done
[[ -n "$ESPEAK_FOUND" ]] || echo "  WARNING: no espeak-ng data dir found; phonemizer needs system espeak-ng"

# --- ONNX Runtime (ort dlopens it; never in DT_NEEDED) ----------------------
# Must run BEFORE the closure loop so libonnxruntime's own deps get scanned.
if strings -a "$PAYLOAD/bin/lepramim" 2>/dev/null | grep -q 'libonnxruntime\.so'; then
  echo "--- ONNX Runtime ---"
  if compgen -G "$PAYLOAD/lib/libonnxruntime.so*" > /dev/null; then
    echo "  already in closure"
  else
    ORT_CAND=""
    if [[ -n "${ORT_DYLIB_PATH:-}" && -f "${ORT_DYLIB_PATH:-}" ]]; then
      ORT_CAND="$ORT_DYLIB_PATH"
    fi
    if [[ -z "$ORT_CAND" ]]; then
      ORT_CAND="$(find "${HOME:-/root}/.cache/ort.pyke.io" /root/.cache/ort.pyke.io \
        /usr/local/lib /usr/lib -name 'libonnxruntime.so.1*' -type f 2>/dev/null | head -n1 || true)"
    fi
    if [[ -z "$ORT_CAND" ]]; then
      ORT_CAND="$(ldconfig -p 2>/dev/null | grep -o '/[^ ]*libonnxruntime\.so[^ ]*' | head -n1 || true)"
    fi
    [[ -n "$ORT_CAND" ]] || die "binary needs libonnxruntime but none found (ORT_DYLIB_PATH / ort download cache / ldconfig all empty)"
    cp -L "$ORT_CAND" "$PAYLOAD/lib/libonnxruntime.so.1"
    cp -L "$ORT_CAND" "$PAYLOAD/lib/libonnxruntime.so"
    # ort resolves bare "libonnxruntime.so" against the executable's dir.
    ln -sfn ../lib/libonnxruntime.so.1 "$PAYLOAD/bin/libonnxruntime.so"
    echo "  bundled from $ORT_CAND"
  fi
fi

# --- Qt platform plugins + QML modules ---
echo "--- Qt plugins / QML ---"
QT_PLUGINS_DIR="$("$QMAKE" -query QT_INSTALL_PLUGINS 2>/dev/null || true)"
QT_QML_DIR="$("$QMAKE" -query QT_INSTALL_QML 2>/dev/null || true)"
[[ -n "$QT_PLUGINS_DIR" && -d "$QT_PLUGINS_DIR" ]] || die "QT_INSTALL_PLUGINS not found via $QMAKE"
[[ -n "$QT_QML_DIR" && -d "$QT_QML_DIR" ]] || die "QT_INSTALL_QML not found via $QMAKE"
echo "  plugins: $QT_PLUGINS_DIR"
echo "  qml:     $QT_QML_DIR"
for cat in platforms platforminputcontexts platformthemes imageformats iconengines \
           tls networkinformation wayland-decoration-client \
           wayland-graphics-integration-client wayland-shell-integration \
           xcbglintegrations egldeviceintegrations generic printsupport; do
  if [[ -d "$QT_PLUGINS_DIR/$cat" ]]; then
    mkdir -p "$PAYLOAD/plugins/$cat"
    cp -a "$QT_PLUGINS_DIR/$cat/." "$PAYLOAD/plugins/$cat/" 2>/dev/null || true
  fi
done
# Optional KDE image codecs: often linked against missing libs; Qt works fine
# with its built-in image plugins.
rm -f "$PAYLOAD/plugins/imageformats/kimg_"*
for mod in QtQuick QtQml; do
  if [[ -d "$QT_QML_DIR/$mod" ]]; then
    mkdir -p "$PAYLOAD/qml/$mod"
    cp -a "$QT_QML_DIR/$mod/." "$PAYLOAD/qml/$mod/" 2>/dev/null || true
  fi
done
cat > "$PAYLOAD/bin/qt.conf" <<'EOF'
[Paths]
Prefix = ./../
Plugins = plugins
Imports = qml
Qml2Imports = qml
EOF

# --- shared-library closure -------------------------------------------
# Bundle everything ldd resolves EXCEPT the true host ABI: the dynamic
# loader, the libc family, and NSS (must match host libc). The GL/EGL
# userspace (libglvnd dispatch + Mesa) IS bundled: minimal systems have no
# Mesa at all, and glvnd still loads the host's NVIDIA/vendor modules from
# system paths, so this is strictly more portable.
# Notably libstdc++/libgcc ARE bundled: a 22.04-built libstdc++ runs fine on
# newer hosts, while the reverse (system libstdc++ older than the bundled
# Qt needs) fails with missing GLIBCXX symbols.
echo "--- Shared-library closure ---"
EXCLUDE_RE='^(linux-vdso|ld-linux|libc\.so|libm\.so|libpthread\.so|libdl\.so|librt\.so|libresolv\.so|libutil\.so|libnss_)'
declare -A SCANNED=()
MISSING_DEPS=()
QUEUE=()
while IFS= read -r -d '' elf; do
  QUEUE+=("$elf")
done < <(find "$PAYLOAD/bin" "$PAYLOAD/plugins" "$PAYLOAD/qml" -type f -print0 2>/dev/null)
SCANNED_COUNT=0
while ((${#QUEUE[@]} > 0)); do
  f="${QUEUE[0]}"
  QUEUE=("${QUEUE[@]:1}")
  is_elf "$f" || continue
  [[ -n "${SCANNED[$f]:-}" ]] && continue
  SCANNED["$f"]=1
  SCANNED_COUNT=$((SCANNED_COUNT + 1))
  # Each file is scanned at most once, so the loop always drains; this bound
  # only guards against pathological ldd output.
  ((SCANNED_COUNT > 50000)) && die "closure loop exceeded 50000 files (queued: ${#QUEUE[@]})"
  while read -r name path; do
    [[ -z "$name" ]] && continue
    if [[ "$name" == "MISSING:" ]]; then
      MISSING_DEPS+=("$path (needed by $f)")
      continue
    fi
    if [[ "$name" =~ $EXCLUDE_RE ]]; then
      continue
    fi
    dest="$PAYLOAD/lib/$name"
    if [[ ! -e "$dest" ]]; then
      cp -L "$path" "$dest" 2>/dev/null || die "cannot copy $path -> $dest"
      QUEUE+=("$dest")
    fi
  done < <(env -u LD_LIBRARY_PATH ldd "$f" 2>/dev/null | awk '/=> not found/ {print "MISSING:", $1} /=> \// {print $1, $3}' || true)
done
if ((${#MISSING_DEPS[@]} > 0)); then
  printf 'error: unresolved shared libraries:\n' >&2
  printf '  %s\n' "${MISSING_DEPS[@]}" >&2
  exit 1
fi
echo "  bundled libs: $(find "$PAYLOAD/lib" -type f | wc -l)"

# --- RPATH -------------------------------------------------------------
echo "--- RPATH ---"
has_dynamic() {
  readelf -d "$1" 2>/dev/null | grep -qE 'NEEDED|SONAME|RUNPATH|RPATH' || return 1
}
while IFS= read -r -d '' elf; do
  is_elf "$elf" || continue
  has_dynamic "$elf" || continue
  patchelf --force-rpath --set-rpath '$ORIGIN/../lib' "$elf"
done < <(find "$PAYLOAD/bin" -type f -print0 2>/dev/null)
while IFS= read -r -d '' elf; do
  is_elf "$elf" || continue
  has_dynamic "$elf" || continue
  patchelf --force-rpath --set-rpath '$ORIGIN' "$elf"
done < <(find "$PAYLOAD/lib" -type f -print0 2>/dev/null)
while IFS= read -r -d '' elf; do
  is_elf "$elf" || continue
  has_dynamic "$elf" || continue
  patchelf --force-rpath --set-rpath '$ORIGIN:$ORIGIN/../lib:$ORIGIN/../../lib:$ORIGIN/../../../lib:$ORIGIN/../../../../lib:$ORIGIN/../../../../../lib' "$elf"
done < <(find "$PAYLOAD/plugins" "$PAYLOAD/qml" -type f -print0 2>/dev/null)

# --- glibc floor report --------------------------------------------------
max_sym() {
  local prefix="$1" best=""
  while IFS= read -r -d '' elf; do
    is_elf "$elf" || continue
    local v
    v="$(strings -a "$elf" 2>/dev/null | grep -o "${prefix}_[0-9][0-9.]*" | sort -Vu | tail -n1 || true)"
    if [[ -n "$v" ]]; then
      if [[ -z "$best" ]] || [[ "$(printf '%s\n%s\n' "$best" "$v" | sort -Vu | tail -n1)" == "$v" ]]; then
        best="$v"
      fi
    fi
  done < <(find "$PAYLOAD" -type f -print0 2>/dev/null)
  printf '%s' "$best"
}
GLIBC_FLOOR="$(max_sym GLIBC)"
GLIBCXX_FLOOR="$(max_sym GLIBCXX)"
echo "--- ABI floor ---"
echo "  max GLIBC required:   ${GLIBC_FLOOR:-none}"
echo "  max GLIBCXX required: ${GLIBCXX_FLOOR:-none}"
echo "  build host libc:      $(ldd --version 2>/dev/null | head -n1)"
if [[ -n "$GLIBC_FLOOR" && "$(printf 'GLIBC_2.35\n%s\n' "$GLIBC_FLOOR" | sort -Vu | tail -n1)" != "GLIBC_2.35" ]]; then
  echo "  WARNING: bundle needs $GLIBC_FLOOR (> glibc 2.35); it will NOT run on Ubuntu 22.04-era distros."
  echo "  WARNING: rebuild with --container for a portable floor."
fi

# --- verify ---------------------------------------------------------------
echo "--- Verifying payload ---"
VERIFY_FAILED=0
while IFS= read -r -d '' elf; do
  is_elf "$elf" || continue
  out="$(env -u LD_LIBRARY_PATH ldd "$elf" 2>&1 || true)"
  if echo "$out" | grep -q "not found"; then
    echo "error: missing libs for $elf:" >&2
    echo "$out" | grep "not found" | sed 's/^/  /' >&2
    VERIFY_FAILED=1
  fi
done < <(find "$PAYLOAD/bin" "$PAYLOAD/lib" "$PAYLOAD/plugins" "$PAYLOAD/qml" -type f -print0 2>/dev/null)
[[ "$VERIFY_FAILED" -eq 0 ]] || exit 1
env -u LD_LIBRARY_PATH "$PAYLOAD/bin/lepramim" --version || die "payload binary failed to run"
find "$PAYLOAD" -type f | sort > "$OUTPUT_DIR/payload-files.txt"

# --- pack single file ---------------------------------------------------------
echo "--- Packing single file ---"
TARBALL="$OUTPUT_DIR/payload.tar.gz"
rm -f "$TARBALL" "$OUTPUT"
tar -czf "$TARBALL" -C "$PAYLOAD" .
PAYLOAD_SHA="$(sha256sum "$TARBALL" | awk '{print $1}')"
cat > "$OUTPUT" <<'STUB_EOF'
#!/bin/sh
# Lepramim portable single-file bundle -- self-extracting launcher.
# Generated by scripts/build-portable.sh; do not edit.
set -eu

APP_ID="lepramim-__LEPRAMIM_VERSION__"
PAYLOAD_SHA256="__LEPRAMIM_SHA256__"

usage() {
  cat <<USAGE
Usage: $(basename "$0") [PORTABLE-OPTIONS] [APP-ARGS...]

Portable options (must come before app args):
  --portable-extract-to DIR   Extract to DIR instead of the cache and run there
  --portable-no-cache         Extract to a fresh temp dir on every launch
  --portable-re-extract       Re-extract even if the cache looks current
  --portable-root             Print the extracted tree path and exit
  --portable-help             Show this help
  --                          Stop option parsing; the rest goes to the app

Everything else is passed to Lepramim unchanged.
USAGE
}

EXTRACT_TO=""
RE_EXTRACT=0
NO_CACHE=0
PRINT_ROOT=0
while [ $# -gt 0 ]; do
  case "$1" in
    --portable-extract-to) EXTRACT_TO="${2:?--portable-extract-to needs DIR}"; shift 2 ;;
    --portable-extract-to=*) EXTRACT_TO="${1#*=}"; shift ;;
    --portable-no-cache) NO_CACHE=1; shift ;;
    --portable-re-extract) RE_EXTRACT=1; shift ;;
    --portable-root) PRINT_ROOT=1; shift ;;
    --portable-help) usage; exit 0 ;;
    --) shift; break ;;
    *) break ;;
  esac
done

SELF="$0"
case "$SELF" in
  */*) ;;
  *) SELF="$(command -v "$SELF")" ;;
esac

CACHE_BASE="${XDG_CACHE_HOME:-$HOME/.cache}/lepramim-portable"
DEST="$CACHE_BASE/$APP_ID"
CLEANUP_DEST=""
if [ "$NO_CACHE" -eq 1 ]; then
  DEST="$(mktemp -d "${TMPDIR:-/tmp}/lepramim-portable-XXXXXX")"
  CLEANUP_DEST="$DEST"
elif [ -n "$EXTRACT_TO" ]; then
  DEST="$EXTRACT_TO"
fi
if [ "$PRINT_ROOT" -eq 1 ]; then
  printf '%s\n' "$DEST"
  exit 0
fi

MARKER="$DEST/.lepramim-portable-marker"
CACHED=""
if [ -f "$MARKER" ]; then CACHED="$(cat "$MARKER" 2>/dev/null || true)"; fi
if [ "$RE_EXTRACT" -eq 1 ] || [ "$CACHED" != "$PAYLOAD_SHA256" ] || [ ! -x "$DEST/bin/lepramim" ]; then
  OFFSET="$(awk '/^__LEPRAMIM_PAYLOAD_FOLLOWS__$/{print NR+1; exit 0;}' "$SELF")"
  if [ -z "$OFFSET" ]; then echo "portable: payload marker not found in $SELF" >&2; exit 1; fi
  mkdir -p "$DEST"
  if ! tail -n +"$OFFSET" "$SELF" | gzip -dc | tar -x -C "$DEST"; then
    echo "portable: extraction failed" >&2; exit 1
  fi
  printf '%s\n' "$PAYLOAD_SHA256" > "$MARKER"
fi

# Bundled libs first; drop IDE mount leakage (e.g. Cursor terminals)
# so foreign Qt builds never shadow the bundled ones.
_CLEAN_LD="$(printf '%s' "${LD_LIBRARY_PATH:-}" | tr ':' '\n' | grep -v '^/tmp/\.mount_' | grep -v '^$' | paste -sd: - 2>/dev/null || true)"
export LD_LIBRARY_PATH="$DEST/lib${_CLEAN_LD:+:$_CLEAN_LD}"
export PATH="$PATH:$DEST/bin"
export QT_PLUGIN_PATH="$DEST/plugins${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}"
export QT_QPA_PLATFORM_PLUGIN_PATH="$DEST/plugins/platforms${QT_QPA_PLATFORM_PLUGIN_PATH:+:$QT_QPA_PLATFORM_PLUGIN_PATH}"
export QML2_IMPORT_PATH="$DEST/qml${QML2_IMPORT_PATH:+:$QML2_IMPORT_PATH}"
export QML_IMPORT_PATH="$DEST/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
if [ -d "$DEST/share/espeak-ng-data" ]; then
  export ESPEAK_DATA_PATH="$DEST/share/espeak-ng-data"
fi
# ort dlopens ONNX Runtime; point it at the bundled copy unless the user
# overrides (e.g. a CUDA-enabled libonnxruntime for --backend cuda12).
if [ -z "${ORT_DYLIB_PATH:-}" ] && [ -f "$DEST/lib/libonnxruntime.so.1" ]; then
  ORT_DYLIB_PATH="$DEST/lib/libonnxruntime.so.1"
  export ORT_DYLIB_PATH
fi
export LEPRAMIM_PORTABLE_ROOT="$DEST"
export LEPRAMIM_PORTABLE=1
# Stable launcher path for desktop entries (the extracted bin is versioned).
case "$SELF" in
  /*) LEPRAMIM_PORTABLE_EXE="$SELF" ;;
  *) LEPRAMIM_PORTABLE_EXE="$(pwd)/$SELF" ;;
esac
export LEPRAMIM_PORTABLE_EXE

if [ -n "$CLEANUP_DEST" ]; then
  "$DEST/bin/lepramim" "$@"
  _rc=$?
  rm -rf "$DEST"
  exit $_rc
fi
exec "$DEST/bin/lepramim" "$@"
__LEPRAMIM_PAYLOAD_FOLLOWS__
STUB_EOF
sed -i "s/__LEPRAMIM_VERSION__/$VERSION/; s/__LEPRAMIM_SHA256__/$PAYLOAD_SHA/" "$OUTPUT"
cat "$TARBALL" >> "$OUTPUT"
chmod 0755 "$OUTPUT"
rm -f "$TARBALL"

if [[ -n "${LEPRAMIM_CHOWN_TO:-}" ]]; then
  chown -R "$LEPRAMIM_CHOWN_TO" "$PROJECT_ROOT/build" 2>/dev/null || true
fi

echo
echo "=== build-portable complete ==="
echo "single file: $OUTPUT"
du -h "$OUTPUT" 2>/dev/null || true
echo "glibc floor: ${GLIBC_FLOOR:-none}"
