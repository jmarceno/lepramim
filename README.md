# Lepramim

Local, private Linux text-to-speech with a system-tray app.

Lepramim reads your highlighted text aloud using the Kokoro neural voice model,
with preprocessing tuned for academic PDFs, citations, math symbols, and Markdown.
Everything runs on your machine — no accounts, no cloud, no telemetry.

## How it works

Lepramim is a single graphical app. It lives in your system tray and handles
everything itself:

1. You highlight text anywhere — a PDF, a web page, an editor, a terminal.
2. You press **Meta+R** (or pick **Speak highlighted selection** in the tray menu).
3. Lepramim captures the highlight, cleans it up (strips citations, expands
   abbreviations, reads math symbols as words), and speaks it.
4. **Meta+P** pauses and resumes. A floating overlay can show the current
   sentence with pause, skip, and stop buttons.

There is nothing else to start, stop, or configure by hand. Opening the app
starts the speech engine as part of the app; quitting the tray icon stops it.

## Quick start

Download the AppImage, make it executable, and open it — from your file manager
or by running it with no arguments:

```bash
chmod +x Lepramim-*-x86_64.AppImage
./Lepramim-*-x86_64.AppImage
```

That is the whole install:

- missing speech models download automatically on first launch
- the speech engine starts inside the app (no system services to enable)
- the tray icon appears

Quit from the tray menu to stop everything. Tick **Start with desktop** in the
tray menu if you want Lepramim waiting for you at login.

> Opening Lepramim a second time never opens a duplicate: the running copy
> simply brings its control window forward.

## First launch

If the speech models are not on your machine yet, a welcome window opens and
downloads them with a progress bar (about 340 MB, one time only):

- **Continue** starts the download in the background and carries on as soon as
  the models are ready.
- **Skip** closes the welcome window; the app will offer the download again
  from the **Models** tab when you need it.

The download is verified by checksum before first use. If it is ever
interrupted, just relaunch the app — it resumes from a clean state.

## Everyday use

### Reading highlighted text

1. Highlight the text you want to hear.
2. Press **Meta+R**, or right-click the tray icon and choose
   **Speak highlighted selection**.

On X11 the highlight itself is read. On Wayland, Lepramim briefly presses
Ctrl+C for you so the highlight lands on the clipboard, then reads that —
clipboard snapshots before and after are compared so your existing clipboard
content is never spoken by mistake.

If nothing is highlighted, a desktop notification tells you to select text
first. Very long selections are read up to a size limit and a notification
tells you the rest was skipped.

### Pause, resume, stop

- Press **Meta+P** to pause or resume.
- Use the tray menu (**Pause / resume**, **Stop current playback**) or the
  floating overlay buttons (previous, pause, next, stop) for the same controls.

### The tray icon

The icon tells you what the speech engine is doing at a glance:

| Look | Meaning |
|------|---------|
| Dim blue | Engine stopped |
| Breathing blue | Warming up / preparing speech |
| Solid green | Speaking (stays green while paused, so you know where you left off) |

Hovering shows a tooltip such as `Lepramim: running`. Right-clicking opens the
menu:

| Menu item | What it does |
|-----------|--------------|
| Shortcut reminder (`Meta+R`) | Informational — shows the speak hotkey |
| CPU notice (when shown) | Informational — appears when hardware acceleration is unavailable |
| Start / Stop daemon | Starts or stops the speech engine without quitting the app |
| Speak highlighted selection | Reads the current highlight (same as Meta+R) |
| Pause / resume | Pauses or resumes playback (same as Meta+P) |
| Stop current playback | Stops and clears the current reading |
| Control window… | Opens the settings window |
| Start with desktop (✓) | Toggles launching Lepramim at login |
| Quit Lepramim | Shuts down the engine and quits the app |

Left-clicking the tray icon opens the control window.

## Control window

Open it from the tray menu or by clicking the tray icon. The window uses a
sidebar for navigation and a dark card layout for settings.

### Voice

- **Voice**: 54 bundled Kokoro voices across 9 languages (American and British
  English, Spanish, French, Hindi, Italian, Japanese, Brazilian Portuguese,
  Mandarin Chinese). Default is Heart, an American female voice.
- **Language**: the voice's language. Tick **Filter voices by language** to
  narrow the voice list to the selected language.
- **Speed slider** (0.50×–2.00×): 0.85×–1.30× is the safe range for dense
  reading; faster is fine for familiar material but hurts comprehension on new
  academic text.
- **Test voice** reads a short sample with your current settings so you can
  audition voices.
- **Read selection** captures the current highlight (same as Meta+R).
- **Floating overlay** toggle lives here: an always-on-top bar with the current
  sentence plus previous, pause, next, and stop. Off by default to keep
  Lepramim discreet.
- **Apply settings** saves; the new voice and speed apply from the next
  playback start.

### Preprocessor

How captured text is cleaned before speaking:

- Deduplicate MathJax selection (browser selections on math pages capture each
  expression twice — the duplicate is removed)
- Strip Markdown (headings, lists, emphasis, tables, code blocks, links)
- Strip numeric bracket citations (`[3]`, `[1–4]`)
- Expand Latin abbreviations (`i.e.`, `etc.`)
- Normalize numbers (`50%` → “fifty percent”, `$100` → “one hundred dollars”)

### Advanced

Extra cleanup options that are also stored in `config.toml`:

- Strip parenthetical citations
- Expand academic abbreviations
- Normalize URLs
- Normalize math symbols
- PDF cleanup
- Speech Rule Engine for LaTeX (optional)

### Models

Shows each speech model file with its size and whether it is present or
missing, plus **Refresh** and **Download missing models** buttons. This is
where you repair a corrupt or partial download: delete the model's folder
(see [Files](#files)) and press **Download missing models**.

## Hotkeys

While the app is running, two global shortcuts are registered:

| Shortcut | Action |
|----------|--------|
| **Meta+R** | Speak highlighted selection |
| **Meta+P** | Pause / resume playback |

On KDE Plasma they are registered through KGlobalAccel and work everywhere.
On other desktops the app additionally exposes a `org.lepramim.App` service on
the session bus (`SpeakSelection` / `Toggle`) so you can bind keys of your
choice in **Settings → Keyboard → Custom Shortcuts**.

Wayland note: injecting the Ctrl+C that captures a highlight needs one of
`ydotool`, `wtype`, `xdotool`, or `dotool` on your system. Without any of them,
copy the text yourself (Ctrl+C) and press Meta+R — Lepramim reads the
clipboard content.

## Configuration

Lepramim works out of the box, but every setting is also stored as plain text
at `~/.config/lepramim/config.toml` (created on first launch with defaults).
The control window edits voice, speed, overlay, and the main preprocessor
switches for you; the file additionally holds capture limits, audio queue
depth, and the optional LaTeX speech section, which is off by default.

If you edit the file by hand, your changes apply the next time the speech
engine starts (toggle it in the tray menu, or relaunch the app). Unknown keys
are ignored, so an old config never breaks a new version.

## Files

| Location | Contents |
|----------|----------|
| `~/.config/lepramim/config.toml` | Your settings |
| `~/.cache/lepramim/models/` | Speech models (`kokoro-v1.0.onnx`, `voices-v1.0.bin`; ~340 MB total) |
| `$XDG_RUNTIME_DIR/lepramim/lepramim.sock` | App-to-engine channel (user-private) |
| `$XDG_RUNTIME_DIR/lepramim/daemon.log` | Speech engine log for troubleshooting |
| `~/.config/autostart/lepramim.desktop` | Created when **Start with desktop** is ticked |

Models live outside the app on purpose: reinstalling or updating Lepramim never
re-downloads them.

## Requirements

- A Linux desktop with a system-tray host (StatusNotifier). The app refuses to
  start without one and says so.
- A graphical session (`DISPLAY` or `WAYLAND_DISPLAY`).
- For highlight capture: `xclip` (X11) or `wl-paste` from `wl-clipboard`
  (Wayland) — both are bundled in the AppImage.
- For one-keypress capture on Wayland: one of `ydotool`, `wtype`, `xdotool`,
  or `dotool`; otherwise copy first and press Meta+R.

## Build from source

Install system dependencies (Debian/Ubuntu example):

```bash
sudo apt install build-essential clang cmake pkg-config \
  libasound2-dev libssl-dev libdbus-1-dev libgl-dev \
  qt6-base-dev qt6-declarative-dev qt6-svg-dev \
  qml6-module-qtquick qml6-module-qtquick-controls \
  qml6-module-qtquick-layouts qml6-module-qtquick-window \
  wl-clipboard xclip libfontconfig1-dev
```

Quality gate and packaging:

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

CUDA is supported only for source installs with `--backend cuda12`. The CPU
AppImage never bundles CUDA.

## Troubleshooting

- **No tray icon**: your desktop has no StatusNotifier host running. Start one
  (e.g. Plasma's system tray, `stalonetray`, or your bar's tray module) and
  relaunch.
- **App exits when opening a window** (control window, overlay): the AppImage
  defaults to Qt Quick software rendering (`QT_QUICK_BACKEND=software`) so it
  does not depend on the host's `wayland-egl` integration. If you overrode the
  backend, unset `QT_QUICK_BACKEND` / `QSG_RHI_BACKEND` and try again.
- **Meta+R does nothing**: on KDE, check that no other action grabbed Meta+R.
  Elsewhere, bind your preferred keys to the `org.lepramim.App` bus service in
  keyboard settings. On Wayland without a key-injection tool, copy first.
- **“Select text first” notification**: nothing was highlighted and the
  clipboard held no new text. Highlight, or copy, then try again.
- **No sound**: check your system volume and output device, then stop and
  restart the engine from the tray menu.
- **Speech never starts after an update**: open the **Models** tab. If a file
  shows missing, press **Download missing models**. If trouble persists, quit,
  delete `~/.cache/lepramim/models`, relaunch, and let the welcome window
  download again.
- **App says the engine log has errors**: open
  `$XDG_RUNTIME_DIR/lepramim/daemon.log` and include the relevant lines when
  asking for help.

## Voices and model licensing

The Kokoro-82M weights and bundled voices are Apache-2.0 (see
`docs/models.md`); the `kokoro-onnx` wrapper is MIT. Lepramim ships them
unmodified. See `THIRD_PARTY_LICENSES.md` in releases for the full list.

## License

MIT — see [`LICENSE`](LICENSE).
