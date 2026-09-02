# Installing Lexaloud on Ubuntu / Debian

This is the Tier 1 supported install path. The maintainer develops on
Ubuntu 24.04 with GNOME. Debian 13 and Ubuntu-derived
distros (Linux Mint, Pop!_OS, elementary, Zorin, Kali) should work with
the same package names.

## 1. System packages

```bash
sudo apt install \
    wl-clipboard \
    xclip \
    libportaudio2 \
    libnotify-bin \
    qt6-base-dev qt6-tools-dev libqt6svg6-dev \
    cmake ninja-build pkg-config clang clang-format
# Rust via rustup (1.85+):
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85
```

For AppImage-only installs (no build from source), you only need the
first line (`wl-clipboard`, `xclip`, `libportaudio2`, `libnotify-bin`);
Qt and the toolchain are bundled in the AppImage.

The tray indicator and control window use Qt 6, which is available as
system packages for source builds and bundled in the AppImage.

## 2. Clone and build

```bash
git clone https://github.com/Gustavjiversen01/lexaloud.git
cd lexaloud
./scripts/build-native.sh --release
./scripts/install.sh --from-source
```

The installer auto-detects whether you have an NVIDIA GPU (via
`nvidia-smi -L`) and reports the backend. You can force with `./scripts/install.sh --backend cpu` or
`--backend cuda12`.

If you have a prebuilt AppImage:

```bash
./scripts/install.sh --appimage build/appimage/Lexaloud-*.AppImage
```

## 3. Run setup

```bash
lexaloud setup
```

This will:

1. Resolve the absolute path of the `lexaloud` binary.
2. Download the Kokoro model artifacts (`kokoro-v1.0.onnx` ~310 MB,
   `voices-v1.0.bin` ~28 MB) into `~/.cache/lexaloud/models/` and
   SHA256-verify them.
3. Write a systemd `--user` unit file to
   `~/.config/systemd/user/lexaloud.service`. If the file already
   exists, pass `--force` to overwrite it.
4. Print a hotkey-binding walkthrough for your session.

## 4. Start the daemon

```bash
systemctl --user daemon-reload
systemctl --user enable --now lexaloud.service
```

Verify it's running:

```bash
systemctl --user status lexaloud.service
lexaloud status
```

You should see `"state": "idle"` with `"session_providers":
["CUDAExecutionProvider", "CPUExecutionProvider"]` (or
`"CPUExecutionProvider"` alone if you installed the CPU backend or if
CUDA setup failed — in which case check `journalctl --user -u
lexaloud.service`).

The daemon listens on a Unix domain socket at
`$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` (mode 0700 enforced by
systemd). There is no TCP port to firewall.

## 5. Bind a global hotkey (GNOME)

See [`../hotkeys/gnome.md`](../hotkeys/gnome.md) for the walkthrough.

For KDE, see [`../hotkeys/kde.md`](../hotkeys/kde.md); for
Sway/Hyprland see [`../hotkeys/sway-hyprland.md`](../hotkeys/sway-hyprland.md);
for XFCE/Cinnamon see [`../hotkeys/xfce-cinnamon.md`](../hotkeys/xfce-cinnamon.md).

## 6. Test

Select some text in any application and press your hotkey. You should
hear it read aloud in the default `af_heart` voice.

For pause/skip/stop:

```bash
lexaloud pause
lexaloud resume
lexaloud toggle
lexaloud skip
lexaloud back
lexaloud stop
```

Bind those to additional hotkeys for single-keystroke transport. The
owner's setup uses `Ctrl+0` for `speak-selection` and `Ctrl+9` for
`toggle`.

## 7. Troubleshooting

See [`../troubleshooting.md`](../troubleshooting.md) for common symptoms
and fixes. The fastest way to produce a bug report is:

```bash
lexaloud bug-report > /tmp/lexaloud-bug.md
```

Paste `/tmp/lexaloud-bug.md` into a new issue.
