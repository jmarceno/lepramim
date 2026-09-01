# AppImage installation

The CPU AppImage contains the Lexaloud application, a Python 3.12 runtime,
the CPU ONNX Runtime, Kokoro's Python bindings, NumPy, FastAPI, PortAudio,
and the clipboard helper programs used by the selection and clipboard
commands.  It does not install Python packages on the host.

Download the `Lexaloud-...-x86_64.AppImage` asset from the releases page of
the Gitea repository, then run:

```bash
chmod +x Lexaloud-*-x86_64.AppImage
./Lexaloud-*-x86_64.AppImage setup
```

`setup` downloads the Kokoro model and voices into
`$XDG_CACHE_HOME/lexaloud/models` (or `~/.cache/lexaloud/models`), writes a
user-level systemd unit whose `ExecStart` points to the AppImage itself, and
prints the hotkey commands.  Activate the service exactly as printed:

```bash
systemctl --user daemon-reload
systemctl --user enable --now lexaloud.service
```

When you want to remove the integration, run the same image with:

```bash
./Lexaloud-*-x86_64.AppImage uninstall
```

That stops/disables the user service, removes the rendered unit, reloads the
user manager, and removes the optional per-user desktop launcher. It keeps the
model cache and configuration; delete the AppImage itself separately.

The model is intentionally external to the AppImage so the image stays
small and a future model update does not require replacing the executable.
The first CPU build uses the same Kokoro model as the other installation
path and does not require CUDA or an NVIDIA driver.

The image includes `wl-paste`, `xclip`, and `notify-send` when the release is
built, so the generic image does not depend on the host distribution's
package names.  It still requires a running graphical session and its host
Wayland/X11 and audio services; an AppImage cannot bundle or replace the
compositor, clipboard ownership, PipeWire/PulseAudio, or ALSA session.

The Qt 6 runtime (PySide6) is bundled, so `lexaloud tray` works out of
the box: the tray icon offers start/stop daemon, playback controls, the
control window, service reinstall/remove actions, and shows the global
shortcut in use.

On Wayland, the most reliable workflow for applications that do not publish a
PRIMARY selection is:

1. Select the text and press `Ctrl+C`.
2. Bind and press the `speak-clipboard` command printed by setup.

The AppImage is currently CPU-only.  The CUDA lockfile and source/venv
installer remain available for a separate GPU build later.
