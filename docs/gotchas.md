# Known gotchas

Things that will trip users (or future-you). Recorded here so nobody has to
rediscover them.

## Pasting from MathJax / KaTeX pages duplicates math

KaTeX and MathJax render each equation as two overlapping DOM layers:
a visually-positioned stacked form (one atom per line) and a compact
inline form used by screen readers. Browser selection captures BOTH,
which caused every math symbol to be read twice before the dedupe
landed in the preprocessor.

The preprocessor's `dedupe_mathjax_selection` stage (on by default
under `[preprocessor] dedupe_mathjax_selection = true`) detects the
stacked-then-compact pattern and removes the stacked copy, also
stripping the U+200B zero-width spaces KaTeX injects around
subscripts. On pages where the heuristic misses (a rarer layout), copy
the selection to the clipboard with `Ctrl+C` first and trigger
`speak-clipboard` — some browsers/pages produce less duplication via
the clipboard than via the PRIMARY selection.

## GNOME Wayland + primary selection

On Ubuntu GNOME Wayland, `wl-paste --primary` may return empty for some
applications (notably Electron apps like VS Code, Obsidian, Slack). The
protocol support varies across Mutter releases.

**Workaround:** bind `lexaloud speak-clipboard` to your hotkey instead of
`lexaloud speak-selection` and use `Ctrl+C` before pressing the hotkey.
The `speak-clipboard` command intentionally never falls back to the
primary selection (and vice versa) so that empty sources never silently
read the wrong content.

## GNOME has no GlobalShortcuts portal

GNOME 46 does not implement the XDG `org.freedesktop.portal.GlobalShortcuts`
portal as of this writing. The v1 hotkey binding path is manual: use
Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts.
`lexaloud setup` prints the exact walkthrough.

## KDE Plasma differences

On KDE Plasma, the GlobalShortcuts portal is available, but v1 still uses
manual KDE Custom Shortcuts for consistency. A future spike may add the
portal path.

## Zathura requires config change

Zathura does not publish selections to the PRIMARY selection by default.
Add this line to `~/.config/zathura/zathurarc`:

```
set selection-clipboard primary
```

After restarting Zathura, `wl-paste --primary` (or xclip -sel primary)
will return the highlighted PDF text.

## Firefox as a Flatpak cannot see the clipboard

If Firefox is installed as a Flatpak, its sandbox restricts clipboard
access. Workaround: use the deb/apt version of Firefox, or grant the
clipboard portal via `flatpak override --user --talk-name=org.freedesktop.portal.Clipboard org.mozilla.firefox`.

## Environment pollution from sourced shells

If you source a shell rc that exports large environment variables, they
leak into the daemon when started from that shell. The systemd unit
is clean because systemd starts with a minimal environment. If you run
`lexaloud daemon` manually from a polluted shell, start it via systemd instead.

**Workaround:** run the daemon via `systemctl --user` rather than manually.

## CUDA cold start is ~30 seconds

The first synthesis call after session construction
takes ~30 seconds on an RTX 5080 because CUDA kernels are compiled on
first use. Subsequent calls for the same sentence length take ~1 second.

The daemon runs an explicit warmup synthesis as a background task during
startup. Any `/speak` request that arrives during warmup waits until warmup completes.

## CUDA runtime libraries

On Ubuntu 24.04 with a system-wide CUDA 12.8 install, `libcublasLt.so.12`
may not be in the default loader path. ONNX Runtime cannot find it
without help, and session construction silently falls back to
CPU (with only a log warning).

The native build links ONNX Runtime directly; ensure CUDA libs are on
the loader path if you use the CUDA backend. If the loader can't find them,
the daemon logs a warning and falls back to CPU cleanly. Check
`lexaloud status` `session_providers` to verify.

## Model weights are not shipped

`kokoro-v1.0.onnx` (~310 MB) and `voices-v1.0.bin` (~28 MB) live under
`~/.cache/lexaloud/models/`. They are downloaded on demand by
`lexaloud download-models` (called automatically by `lexaloud setup`) and
verified against SHA256 pins in `src/models.rs`. If a download
URL changes or a hash drifts, the daemon refuses to start until the user
re-runs `lexaloud download-models`.
