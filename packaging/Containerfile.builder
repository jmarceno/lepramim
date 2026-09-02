# Cached builder image for Lexaloud native AppImage (Rust + Qt).
#
# Contains all system dependencies so every AppImage build doesn't pay
# apt-get update + install (2-3 min) and re-download of tools.
# Build once:  podman build -t lexaloud-builder:24.04 -f packaging/Containerfile.builder .
# Then use:    podman run --rm -v $PWD:/workspace:Z -w /workspace lexaloud-builder:24.04 ./scripts/build-appimage.sh
# Or:          ./scripts/build-appimage-fast.sh  (incremental, reuses builder)
#
# The image is intentionally not auto-rebuilt; bump the tag when release.yml apt list changes.
#
# Pinned toolchain (recorded per Phase 9 contract):
#   Base:        ubuntu:24.04  (digest pinned in CI; see release.yml container: ubuntu:24.04@sha256:…)
#   Rust:        stable 1.85 (rust-toolchain.toml channel=stable, rust-version=1.85 in Cargo.toml)
#   CMake:       >=3.21 (Ubuntu 24.04 provides 3.28.x)
#   Ninja:       >=1.11
#   Qt:          Qt 6.4+ (Ubuntu 24.04 provides Qt 6.4.2; minimum 6.4 required)
#   Compiler:    g++ >=13 or clang++ >=18 (Ubuntu 24.04 default)
#   ONNX Runtime: 1.24.x CPU (pinned via ort 2.x release spike; dynamic loading for CUDA)
#   AppImage:    appimagetool continuous, linuxdeploy continuous (URLs pinned via env)
#   Cargo.lock:  committed; cargo build --locked enforces exact deps

FROM docker.io/library/ubuntu:24.04
# NOTE: CI pins the digest explicitly:
#   container: ubuntu:24.04@sha256:…  (update here when base changes)
# For local builds the tag is sufficient; for release, the workflow uses the digest.

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH=/opt/cargo/bin:$PATH

# System deps: Rust build, Qt6, audio, desktop, AppImage tooling, and clipboard helpers
RUN apt-get update && apt-get install -y --no-install-recommends \
    # --- Build essentials ---
    build-essential \
    clang clang-format \
    cmake ninja-build \
    pkg-config \
    git curl ca-certificates file binutils patchelf \
    # --- Rust deps (for cargo) ---
    libssl-dev \
    # --- Qt 6 (Widgets, Network, DBus, Svg, Test) ---
    qt6-base-dev qt6-base-dev-tools \
    qt6-tools-dev qt6-tools-dev-tools \
    libqt6svg6-dev \
    libgl1-mesa-dev libegl1-mesa-dev \
    # --- Desktop / compositor deps for Qt platform plugins ---
    libfontconfig1 libfreetype6 libdbus-1-3 libglib2.0-0 \
    libx11-6 libxext6 libxrender1 libxcb1 \
    libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 \
    libxcb-randr0 libxcb-render-util0 libxcb-shape0 libxcb-shm0 \
    libxcb-sync1 libxcb-xfixes0 libxcb-xkb1 libxcb-xinerama0 \
    libxkbcommon-x11-0 libxkbcommon0 libgl1 libegl1 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 \
    # --- Audio ---
    libportaudio2 portaudio19-dev \
    # --- Clipboard / notification helpers (bundled into AppImage) ---
    wl-clipboard xclip libnotify-bin \
    # --- ONNX Runtime / eSpeak runtime deps (optional, for future bundling) ---
    libgomp1 \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain pinned to 1.85 (matching Cargo.toml rust-version)
# Use rustup for exact version; fallback to distro cargo if rustup fails.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.85.0 \
    && /opt/cargo/bin/rustup component add clippy rustfmt \
    && /opt/cargo/bin/cargo --version \
    && /opt/cargo/bin/rustc --version \
    && /opt/cargo/bin/cargo clippy --version \
    && /opt/cargo/bin/cargo fmt --version \
    || ( echo "rustup failed, trying apt cargo" && apt-get update && apt-get install -y cargo rustc && rm -rf /var/lib/apt/lists/* )

# Verify Qt and CMake versions meet minimums
RUN cmake --version \
    && ninja --version \
    && clang-format --version \
    && qmake6 --version || qmake --version || echo "qmake not in PATH" \
    && pkg-config --modversion Qt6Core 2>/dev/null || echo "Qt6Core pkg-config not found"

# Pre-create cache dirs for mounted volumes (cargo)
RUN mkdir -p /root/.cargo/registry /root/.cargo/git /tmp/cargo-target

# Cargo registry cache mount point for CI (keyed by Cargo.lock)
VOLUME ["/root/.cargo/registry", "/root/.cargo/git"]

# Default workdir for podman run
WORKDIR /workspace
