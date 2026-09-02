# Design rationale

This document distills the key decisions behind Lexaloud's
architecture, including the "why" behind choices that might look
overbuilt for a solo side project. If you want the "what", see
[`architecture.md`](architecture.md) and the code. If you want the
"how it might fail", see [`gotchas.md`](gotchas.md).

## The problem

Academic reading-along on Linux has no first-class tool. The industry
reference — macOS VoiceOver + an OS-level "Services" menu — relies on
hooks that Linux simply does not have at the toolkit layer. The
workable alternative is:

- Select text in any application
- Press a global hotkey
- Hear the selection read by a local neural voice

That sentence describes the happy path. Every other design choice is
downstream of making it robust.

## Why a native daemon, not a library or a script?

A CLI-only design would face three blocking problems:

1. **Model load cost**: Kokoro's first synthesis after
   session construction takes ~30 seconds on an RTX 5080
   because kernels are compiled on first use. A hotkey
   workflow that pays this cost every time is unusable. A daemon
   amortizes it across the whole session.

2. **Cancellation**: pause, skip, and back need to interact with
   in-flight synthesis + in-flight audio playback + the producer that
   pre-fetches future sentences. You can't express that cleanly in a
   per-invocation subprocess — the subprocess has no durable state to
   cancel.

3. **Hotkey ergonomics**: GNOME Custom Shortcuts fork a fresh
   subprocess per keystroke. That subprocess must exit cleanly, fast.
   Putting the audio loop inside the subprocess would hang the keygrab until audio finishes.

So we have a daemon that owns all the expensive state, and a thin CLI
that speaks HTTP to it over a Unix socket.

## Why HTTP over UDS? Why not a bespoke socket protocol?

- **Axum + native client give us well-tested JSON, timeouts, and error
  handling** for free.
- **The daemon can be curl'd or socat'd** during debugging — no custom client
  needed to poke `/state`.
- **HTTP over Unix socket** means we get all the HTTP niceness over a Unix domain socket, closing the attack surface
  without rewriting the protocol.

## Why Unix domain socket instead of TCP loopback?

The daemon binds `$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` via
systemd's `RuntimeDirectory=lexaloud` + `RuntimeDirectoryMode=0700`.
Only the owner user's processes can reach it. There's no port to
firewall, no cross-user attack surface.

The systemd user unit uses `RuntimeDirectory=lexaloud` +
`RuntimeDirectoryMode=0700`, so `$XDG_RUNTIME_DIR/lexaloud/` is
created with mode 0700 before the daemon starts and automatically
removed on service stop. Only the owner user can traverse into that
directory, which makes the socket file inside it unreachable to
anyone else regardless of the socket's own permissions.

## Why sentence-granularity streaming?

Three alternatives were considered:

1. **Whole-selection synthesis** — synthesize the entire pasted text,
   then play. Dead simple, but blocks audio output for 5-30 seconds
   on long passages and makes cancellation impossible until the whole
   job finishes.

2. **Sample-granularity streaming** — emit audio samples as they're
   produced. Kokoro doesn't work this way: synthesis returns the
   whole sentence as an audio buffer, not a stream.

3. **Sentence-granularity streaming** (chosen) — split the text into
   sentences, synthesize each in the background, play them in order
   with a bounded pre-fetch queue. First audio arrives as soon as the
   first sentence is synthesized (~1 s), and pause/skip/back have
   clean semantic points to act on.

Inside the consumer, we additionally write the audio in
`SUB_CHUNK_SECONDS`-long blocks and check the pause event between
blocks. This gives mid-sentence pause with ~100 ms latency while
keeping the synthesis pipeline at sentence granularity.

## Why a bounded ready queue?

Bounded channel (`capacity 3`) between the producer and the consumer is
what makes "pause for 15 minutes mid-article, then resume" work
without unbounded memory. When the consumer stops pulling, the
producer blocks on send after 3 chunks have queued. Memory during a
long pause is bounded at 3 sentences of audio + 1 in-flight synthesis
result.

## Why ONNX Runtime with CUDA EP, not some other inference backend?

The native provider uses ONNX Runtime directly (linked via the `ort` crate or
system lib). The install detects NVIDIA via `nvidia-smi`; if present, the
session is built with CUDA EP, otherwise CPU. Silent fallback is detected
via `session_providers` in `/state`.

## Why pinned lockfiles and toolchain?

`Cargo.lock` is committed for reproducible builds; `cargo build --locked`
enforces exact dependencies. The toolchain is pinned to Rust 1.85 via
`rust-toolchain.toml` and `Cargo.toml` `rust-version`. Qt 6.4+ is the minimum
supported, as provided by Ubuntu 24.04 LTS.

## Why sentence segmentation with custom rules + pulldown-cmark?

The preprocessor uses a markdown-aware pipeline (pulldown-cmark) paired
with custom normalization: academic abbreviations, number-to-words, URL/email
handling, and Unicode math symbol expansion. It's tuned for academic prose
including citations and math.

## Why Qt 6 instead of GTK?

- **Qt's `QSystemTrayIcon` + `QSystemTrayIcon::isSystemTrayAvailable()`** speaks the StatusNotifierItem protocol out of the box, which GNOME (via the AppIndicator
  extension), KDE, XFCE, Cinnamon, and MATE all support — one code
  path for every tray.
- **Qt 6 ships as native C++** — no runtime interpreter needed. The AppImage bundles
  only the needed Qt plugins (platform, imageformats, iconengines) via an allowlist,
  keeping size controlled.
- **GTK4** dropped AppIndicator support, and writing a GNOME Shell
  extension to replace the tray is a much larger scope.
- **Licensing**: Qt 6 is LGPLv3 / GPLv3, which permits distribution with
  proper attribution; see `THIRD_PARTY_LICENSES.md`.

The trade-off is bundle size: the AppImage grows to carry the Qt runtime,
but native system installs use the host Qt.

## Why no overlay / karaoke / browser extension?

All three are genuinely useful. They're deferred to v0.2+ because
each requires design work the maintainer wants to do right, not fast:

- **Floating overlay**: mouse-through + stays-on-top behavior is
  compositor-dependent (Wayland layer protocol + X11 override-redirect
  + Mutter quirks). Getting this right takes a dedicated spike.
- **Karaoke word-level**: Kokoro doesn't expose word timings. A forced
  aligner has its own model, licensing, and integration work.
- **Browser extension**: three store listings, cross-origin messaging,
  Manifest v3, clipboard vs. selection API differences.

See `ROADMAP.md` for the full deferred list.

## What would you change if you were starting over?

- **Settle on UDS from day one.** Early TCP loopback designs left
  config fields that took a migration to clean up.
- **Start with the distro-neutral installer.** Hardcoding apt made
  Tier 2 support (Fedora / Arch) a bigger rewrite.

Most of this document is "here's what I'd tell v0-me". Lexaloud itself
is small enough that you can read all of it in an afternoon — the
design is not the moat, the taste in integration details is.
