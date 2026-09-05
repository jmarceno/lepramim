# Building Lepramim from source

## System dependencies

Install system dependencies (Debian/Ubuntu example):

```bash
sudo apt install build-essential clang cmake pkg-config \
  libasound2-dev libssl-dev libdbus-1-dev libgl-dev \
  qt6-base-dev qt6-declarative-dev qt6-svg-dev \
  qml6-module-qtquick qml6-module-qtquick-controls \
  qml6-module-qtquick-layouts qml6-module-qtquick-window \
  wl-clipboard xclip libfontconfig1-dev
```

## Quality gate and packaging

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lepramim-*.AppImage
```

After code changes, rebuild the stage before packaging:

```bash
rm -rf build/stage
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
```

## CUDA

CUDA is supported only for source installs with `--backend cuda12`. The CPU
AppImage never bundles CUDA.
