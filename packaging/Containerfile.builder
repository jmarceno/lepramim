# Cached builder image for Lepramim native AppImage (Rust + Qt 6 QML).
FROM docker.io/library/ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH=/opt/cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential clang cmake pkg-config git curl ca-certificates file binutils patchelf \
    libssl-dev libasound2-dev espeak-ng libportaudio2 \
    libfontconfig1 libfreetype6 libdbus-1-3 libdbus-1-dev libglib2.0-0 \
    libx11-6 libxext6 libxrender1 libxcb1 \
    libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 \
    libxcb-randr0 libxcb-render-util0 libxcb-shape0 libxcb-shm0 \
    libxcb-sync1 libxcb-xfixes0 libxcb-xkb1 libxcb-xinerama0 \
    libxkbcommon-x11-0 libxkbcommon0 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 \
    libgl-dev libxdo-dev \
    qt6-base-dev qt6-base-dev-tools qt6-declarative-dev qt6-svg-dev \
    qml6-module-qtquick qml6-module-qtquick-controls \
    qml6-module-qtquick-layouts qml6-module-qtquick-templates \
    qml6-module-qtquick-window qml6-module-qtqml-workerscript \
    libqt6svg6 \
    wl-clipboard xclip libnotify-bin xvfb \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.85.0 \
    && /opt/cargo/bin/rustup component add clippy rustfmt

ENV APPIMAGETOOL_URL=https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
ENV LINUXDEPLOY_URL=https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
ENV LINUXDEPLOY_PLUGIN_QT_URL=https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage
RUN mkdir -p /opt/appimage-tools \
    && curl -fsSL "$APPIMAGETOOL_URL" -o /opt/appimage-tools/appimagetool-x86_64.AppImage \
    && curl -fsSL "$LINUXDEPLOY_URL" -o /opt/appimage-tools/linuxdeploy-x86_64.AppImage \
    && curl -fsSL "$LINUXDEPLOY_PLUGIN_QT_URL" -o /opt/appimage-tools/linuxdeploy-plugin-qt-x86_64.AppImage \
    && chmod 0755 /opt/appimage-tools/*.AppImage \
    && ln -sfn /opt/appimage-tools/linuxdeploy-plugin-qt-x86_64.AppImage /opt/appimage-tools/linuxdeploy-plugin-qt
ENV APPIMAGETOOL=/opt/appimage-tools/appimagetool-x86_64.AppImage
ENV LINUXDEPLOY=/opt/appimage-tools/linuxdeploy-x86_64.AppImage
ENV PATH=/opt/appimage-tools:$PATH

WORKDIR /workspace
