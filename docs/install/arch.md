# Installing Lexaloud on Arch / Manjaro

Tier 2. The maintainer does not CI-test on Arch, but the installer
auto-detects Arch via `/etc/os-release` and uses the correct package
names. This guide is for Arch rolling as of early 2026.

## 1. System packages

```bash
sudo pacman -S \
    wl-clipboard \
    xclip \
    portaudio \
    libnotify \
    qt6-base qt6-svg cmake ninja clang
# Rust via rustup:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85
```

For AppImage-only installs, you only need `wl-clipboard`, `xclip`, `portaudio`, `libnotify`.

The tray indicator and control window use Qt 6, which is
available as system packages for source builds and bundled in the AppImage.

## 2. NVIDIA GPU (optional)

```bash
sudo pacman -S nvidia nvidia-utils
# Reboot
```

The installer's `--backend auto` detects `nvidia-smi` and picks
`cuda12`. On CPU-only machines or to force CPU:

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

See [`ubuntu-debian.md`](ubuntu-debian.md) from step 3 onward.

## 4. Bind a hotkey

See the hotkey guide for your desktop:
- GNOME: [`../hotkeys/gnome.md`](../hotkeys/gnome.md)
- KDE Plasma: [`../hotkeys/kde.md`](../hotkeys/kde.md)
- Sway / Hyprland: [`../hotkeys/sway-hyprland.md`](../hotkeys/sway-hyprland.md)
- XFCE / Cinnamon: [`../hotkeys/xfce-cinnamon.md`](../hotkeys/xfce-cinnamon.md)

## 5. Troubleshooting

See [`../troubleshooting.md`](../troubleshooting.md). Quick bug report:

```bash
lexaloud bug-report > /tmp/lexaloud-bug.md
```

## Arch-specific notes

- Arch moves quickly. The native build is pinned to Rust 1.85 and
  Qt 6.4+; if you hit a toolchain issue, file an issue with
  `rustc --version` and `qmake6 --version`.
- There is no AUR package yet. If someone volunteers to maintain one,
  we'll link to it here.
- Manjaro, EndeavourOS, Garuda, Artix, and CachyOS inherit from Arch
  and should work with the same `pacman` commands. The installer
  classifies them all as `arch` family via `/etc/os-release` ID_LIKE.

## Reporting issues

Please include the full output of `lexaloud bug-report` when filing an
Arch-specific issue.
