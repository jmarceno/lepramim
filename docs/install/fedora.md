# Installing Lexaloud on Fedora

Tier 2. The maintainer does not CI-test on Fedora, but the installer
auto-detects Fedora via `/etc/os-release` and uses the correct package
names. This guide is for Fedora 41 Workstation.

## 1. System packages

```bash
sudo dnf install \
    wl-clipboard \
    xclip \
    portaudio \
    libnotify \
    qt6-qtbase-devel qt6-qtsvg-devel cmake ninja-build clang
# Rust via rustup:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85
```

For AppImage-only installs, you only need `wl-clipboard`, `xclip`, `portaudio`, `libnotify`.

The tray indicator and control window use Qt 6, which is
available as system packages for source builds and bundled in the AppImage.

## 2. NVIDIA GPU (optional)

If you have an NVIDIA card and want the CUDA backend:

```bash
# RPM Fusion nonfree has the NVIDIA driver
sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda
# Reboot or modprobe nvidia
```

The installer's `--backend auto` will detect the NVIDIA driver via
`nvidia-smi -L`. On CPU-only systems or if you want to force CPU:

```bash
./scripts/install.sh --from-source --backend cpu
```

## 3. Clone and build

```bash
git clone https://github.com/Gustavjiversen01/lexaloud.git
cd lexaloud
./scripts/build-native.sh --release
./scripts/install.sh --from-source
```

See [`ubuntu-debian.md`](ubuntu-debian.md) from step 3 onward for the
`lexaloud setup` flow — it is distro-agnostic.

## 4. Bind a hotkey

See the hotkey guide for your desktop:
- GNOME: [`../hotkeys/gnome.md`](../hotkeys/gnome.md)
- KDE Plasma: [`../hotkeys/kde.md`](../hotkeys/kde.md)
- Sway / Hyprland: [`../hotkeys/sway-hyprland.md`](../hotkeys/sway-hyprland.md)

## 5. Troubleshooting

See [`../troubleshooting.md`](../troubleshooting.md) for common symptoms
and fixes. The fastest way to file a bug:

```bash
lexaloud bug-report > /tmp/lexaloud-bug.md
```

## Known Fedora differences

- GNOME ships without `ubuntu-appindicators` by default. The tray
  requires the AppIndicator support extension from extensions.gnome.org.
- SELinux does not interfere with `systemd --user` services under
  normal targeted policy.

## Reporting issues

Please include the full output of `lexaloud bug-report` when filing a
Fedora-specific issue so the maintainer can add the distro to the CI
matrix.
