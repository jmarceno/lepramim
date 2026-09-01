# Cached builder image for Lexaloud AppImage.
# Contains all system dependencies so every AppImage build doesn't pay
# apt-get update + install (2-3 min) and re-download of tools.
# Build once:  podman build -t lexaloud-builder:24.04 -f packaging/Containerfile.builder .
# Then use:    podman run --rm -v $PWD:/workspace:Z -w /workspace lexaloud-builder:24.04 ./scripts/build-appimage.sh
# The image is intentionally not auto-rebuilt; bump the tag when release.yml apt list changes.
FROM docker.io/library/ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-pip python3-venv python3.12 python3.12-venv python3.12-dev \
    patchelf curl ca-certificates file binutils \
    wl-clipboard xclip libnotify-bin libportaudio2 \
    libfontconfig1 libfreetype6 libdbus-1-3 libglib2.0-0 \
    libx11-6 libxext6 libxrender1 libxcb1 \
    libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 \
    libxcb-randr0 libxcb-render-util0 libxcb-shape0 libxcb-shm0 \
    libxcb-sync1 libxcb-xfixes0 libxcb-xkb1 libxcb-xinerama0 \
    libxkbcommon-x11-0 libxkbcommon0 libgl1 libegl1 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Pre-create pip cache dir for mounted volume
RUN mkdir -p /root/.cache/pip
