# Building Lepramim from source

## Portable single-file release (default)

The release artifact is one self-extracting executable,
`build/portable/Lepramim-<version>-x86_64-portable.run`, with the binary and
its shared-library closure (Qt, ALSA, GL, helpers) embedded. ONNX Runtime is
statically linked, so it is inside the binary itself.

The bundle MUST be built on the old-glibc baseline (Ubuntu 22.04, glibc
2.35) or it will not run on older distros. The container path handles that;
it only needs docker on the host:

```bash
./scripts/build-portable.sh --container
./scripts/smoke-portable.sh build/portable/Lepramim-*.run
```

After code changes, just re-run the same two commands (cargo caches in
docker volumes make rebuilds incremental).

## Native development builds

For iteration without docker, install system dependencies (Debian/Ubuntu
example) and build natively. A native bundle works, but inherits this
machine's glibc floor — release from the container instead.

```bash
sudo apt install build-essential clang lld cmake pkg-config patchelf \
  libasound2-dev libssl-dev libdbus-1-dev libgl-dev \
  qt6-base-dev qt6-declarative-dev qt6-svg-dev \
  qml6-module-qtquick qml6-module-qtquick-controls \
  qml6-module-qtquick-layouts qml6-module-qtquick-window \
  wl-clipboard xclip libfontconfig1-dev espeak-ng
```

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/build-portable.sh
./scripts/smoke-portable.sh build/portable/Lepramim-*.run
```

`lld` (or another non-bfd linker) is required: `qt-build-utils` refuses GNU
ld.bfd and falls back to ld.gold, which mislinks cxx-qt's generated module
initializers.

## CUDA

CUDA is supported only for source installs with `--backend cuda12`. The CPU
portable bundle never includes CUDA; advanced users can point a CUDA-capable
`libonnxruntime` at it via `ORT_DYLIB_PATH`.
