# AppImage installation

The CPU AppImage contains the Lexaloud application, a Python 3.12 runtime,
the CPU ONNX Runtime, Kokoro's Python bindings, NumPy, FastAPI, PortAudio,
and the clipboard helper programs used by the selection and clipboard
commands.  It does not install Python packages on the host.

Download the `Lexaloud-...-x86_64.AppImage` asset from the releases page of
the Gitea repository, then run:

```bash
chmod +x Lexaloud-*-x86_64.AppImage
./Lexaloud-*-x86_64.AppImage
```

That is the whole installation. The app opens as a tray icon and:

- downloads the Kokoro model on first start (one-time, ~350 MB, with a
  progress window),
- creates default global shortcuts where the desktop allows it
  (GNOME/XFCE write them automatically; KDE Plasma registers them
  through the GlobalShortcuts portal when the daemon starts),
- asks whether to start automatically at login ("Start with desktop",
  changeable any time from the tray menu),
- runs the speech daemon as part of the app — no systemd unit, no
  terminal commands, and no host Python.

`lexaloud setup` (advanced) still exists for users who prefer a
systemd `--user` service instead of app-managed lifecycle; when such a
unit is installed and running, the app adopts the running daemon.

To remove the optional systemd integration, run:

```bash
./Lexaloud-*-x86_64.AppImage uninstall
```

The model is intentionally external to the AppImage so the image stays
small and a future model update does not require replacing the executable.
The first CPU build uses the same Kokoro model as the other installation
path and does not require CUDA or an NVIDIA driver.

The image includes `wl-paste`, `xclip`, and `notify-send` when the release is
built, so the generic image does not depend on the host distribution's
package names.  It still requires a running graphical session and its host
Wayland/X11 and audio services; an AppImage cannot bundle or replace the
compositor, clipboard ownership, PipeWire/PulseAudio, or ALSA session.

The Qt 6 runtime (PySide6) is bundled, so the tray icon works out of
the box: the tray menu offers start/stop daemon, playback controls, the
control window, a "Start with desktop" autostart toggle, and shows the
global shortcut in use.

On Wayland, the most reliable workflow for applications that do not publish a
PRIMARY selection is:

1. Select the text and press `Ctrl+C`.
2. Bind and press the `speak-clipboard` command printed by setup.

The AppImage is currently CPU-only.  The CUDA lockfile and source/venv
installer remain available for a separate GPU build later.
