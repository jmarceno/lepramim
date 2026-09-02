# Architecture

Lexaloud is two native processes loosely coupled by a Unix domain socket:

1. A **Rust daemon** (`lexaloud` binary, Tokio + Axum) running as a `systemd --user` unit that owns
   the TTS provider, the playback state machine, and the audio sink.
2. A **Qt 6 UI** (`lexaloud-ui` binary, C++/Qt) providing a tray indicator + control window
   for visual state feedback and voice/hotkey configuration. It connects via `QLocalSocket`.
3. A **CLI** (`lexaloud` binary subcommands) that captures selection text and sends
   HTTP requests to the daemon over the same socket.

The binding between them is intentionally thin so each piece can be
tested or replaced in isolation. Two executables share one socket.

## Component diagram

```mermaid
flowchart TD
    subgraph GNOME[GNOME Shell]
        HK[Custom Shortcut<br/>Ctrl+0 / Ctrl+9]
        TI[Tray Indicator<br/>lexaloud-ui]
    end

    subgraph CLI[lexaloud CLI]
        CS[speak-selection]
        CL[speak-clipboard]
        PS[pause / resume / skip / stop / back]
        ST[status]
    end

    subgraph Daemon[lexaloud daemon<br/>systemd --user<br/>Rust / Tokio / Axum]
        API[Axum HTTP API<br/>Unix domain socket<br/>$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock]
        PL[Player<br/>job lifecycle<br/>bounded ready queue]
        PP[Preprocessor<br/>PDF cleanup<br/>citations<br/>segmentation]
        PV[KokoroProvider<br/>ONNX Runtime<br/>CUDA EP / CPU]
        SNK[AudioSink<br/>CPAL / PortAudio]
    end

    subgraph UI[lexaloud-ui<br/>Qt 6]
        QLS[QLocalSocket<br/>HTTP client]
        CW[Control Window]
        TRAY[Tray Icon]
    end

    subgraph Files[Filesystem]
        CFG[~/.config/lexaloud/config.toml]
        CACHE[~/.cache/lexaloud/models/]
        UNIT[~/.config/systemd/user/lexaloud.service]
    end

    HK --> CS
    HK --> CL
    HK --> PS
    TI --> PS
    TI --> ST

    CS -->|POST /speak| API
    CL -->|POST /speak| API
    PS -->|POST /pause etc.| API
    ST -->|GET /state| API
    QLS -->|GET /state<br/>poll| API
    QLS --> CW
    QLS --> TRAY

    API --> PL
    PL --> PP
    PP --> PV
    PV -->|AudioChunk| PL
    PL --> SNK

    Daemon -.reads.-> CFG
    Daemon -.reads.-> CACHE
    UNIT -.runs.-> Daemon

    style Daemon fill:#e1f5ff
    style UI fill:#e8ffe1
    style Files fill:#fff4e1
```

## Data flow: a single `/speak` request

1. User selects text, presses the hotkey.
2. GNOME spawns `lexaloud speak-selection`.
3. CLI calls `wl-paste --primary` (or `xclip -o -selection primary`),
   reads the bytes, UTF-8-safe-truncates to `capture.max_bytes`.
4. CLI opens a native HTTP client bound to the Unix domain socket and POSTs
   `{"text": ..., "mode": "replace"}` to `/speak`.
5. Daemon's `/speak` handler:
   - Rejects null bytes → 400.
   - Rejects text > `capture.max_bytes` → 413.
   - Runs `preprocess()` to clean PDF artifacts, expand Latin
     abbreviations, and split into sentences.
   - Rejects any sentence > `MAX_SENTENCE_CHARS` → 400.
   - Calls `player.speak(sentences, mode="replace")`.
6. `Player.speak`:
   - Bumps the monotonic job ID (cancels any in-flight job).
   - Flushes the audio sink.
   - Resets pending queue, starts fresh producer + consumer tasks.
7. **Producer task**: iterates pending sentences, calls
   `provider.synthesize()` for each sentence, pushes `AudioChunk`s to the bounded `ready_queue`.
8. **Consumer task**: pulls `AudioChunk`s, writes them to the sink in
   `SUB_CHUNK_SECONDS`-long blocks, checking pause/cancel between
   blocks. Inserts a ~180 ms silence pad between sentences.
9. When pending is empty, producer pushes a sentinel; consumer
   flushes the sink and transitions to `idle`.

## Key design choices

### Sentence granularity, not sample granularity

Streaming at the sentence level gives us clean pause/skip/back
semantics — we can't cleanly cancel a mid-word sample since Kokoro
emits a whole-sentence waveform in one call. Sub-chunk playback lets
us pause with ~100 ms latency without touching the synthesis pipeline.

### Bounded ready queue

Bounded async channel (`ready_queue_depth = 3` by default) between producer and consumer bounds
memory. When the user pauses, the producer blocks on send after
the queue fills up, so any-length pause uses bounded RAM.

### Cooperative cancellation via job IDs

The provider takes `(sentence, job_id, is_current_job)` and checks
`is_current_job(job_id)` at key points. On `stop`/`skip`/`back` we
bump the job ID, and any in-flight provider calls return `None` after
their task completes. This is robust under concurrent
HTTP requests without mid-call cancellation.

### Unix domain socket, not TCP loopback

The daemon binds `$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` via
systemd's `RuntimeDirectory=lexaloud` + `RuntimeDirectoryMode=0700`.
Only the owner user's processes can reach it. There's no port to
firewall, no cross-user attack surface, and no local "anyone on
127.0.0.1 can spam /speak" footgun.

### ONNX Runtime with CUDA EP

The provider verifies execution providers after session construction and
reports `session_providers` in `/state`. Silent fallback to CPU is
detected and surfaced. The daemon logs a warning if CUDA was requested
but only CPU is available.

### Qt UI via QLocalSocket

`lexaloud-ui` uses `QLocalSocket` to issue `GET /state` polls and
`POST` control commands over the same UDS. No extra port, no
authentication — socket permissions restrict access to the owner user.
The UI is a separate process so a UI crash never takes down the daemon.

## What's NOT in the daemon

- **No audio mixing** — we own the stream, single producer, single
  sink. Other processes get exclusive access to the audio device
  only when the daemon is actively playing; the sink opens on first
  write and closes on sentinel.
- **No session persistence** — `/state` is ephemeral. Restart the
  daemon and you lose your current queue. This is intentional for
  v0.1.0; v0.2 may add resume-on-restart.
- **No remote control** — the daemon only listens on UDS.
- **No multi-user** — the daemon is a `systemd --user` service, one
  instance per user.

## See also

- `docs/design-rationale.md` — why these design choices.
- `docs/http-api.md` — HTTP endpoint reference.
- `docs/models.md` — Kokoro model provenance.
