# Rust Rewrite Baseline Measurements

> Phase 1 / Phase 9 gate artifact. Record measured values for the native
> baseline (`fcd9d01`) and the native Rust+Qt build on the same reference
> machine. All measurements are per the plan: 30 warm runs for latency, cold
> start from clean checkout, idle RSS via `/usr/bin/time -v` or
> `/proc/<pid>/status`, synthesis RTF on a fixed paragraph.

## Reference Machine

| Field | Value |
|-------|-------|
| Host | <!-- e.g. ThinkPad P1 Gen 6, i7-13800H, 32 GB, Ubuntu 24.04, kernel 6.8 --> |
| Date | <!-- YYYY-MM-DD --> |
| Commit (native baseline) | `fcd9d01` |
| Commit (Rust native) | <!-- rust-qt-rewrite branch tip --> |
| Model artifacts | `kokoro-v1.0.onnx` `7d5df8e…`, `voices-v1.0.bin` `bca610b…` (from `src/models.rs`) |
| Audio backend | <!-- PipeWire 1.0.x / PulseAudio 16.x / ALSA --> |
| Qt version | <!-- Qt 6.4.x (minimum) --> |
| Rust toolchain | <!-- stable 1.85 pinned in rust-toolchain.toml --> |
| ONNX Runtime | <!-- 1.24.x CPU, CUDA 12.x if applicable --> |
| Cargo profile | `lto="thin" codegen-units=1 strip="symbols" panic="abort"` |

## 1. Artifact Sizes

| Artifact | native baseline | Rust+Qt native | Delta |
|----------|-----------------|----------------|-------|
| `dist/Lexaloud-*.AppImage` | ~336 MB (frozen native runtime before model cache, Qt6/Qt ~86 MB included) | <!-- measured --> | <!-- --> |
| `build/appdir` (uncompressed AppDir) | — | <!-- measured via `du -sh build/appdir` --> | <!-- --> |
| `build/stage` (native stage) | — | <!-- `du -sh build/stage` --> | <!-- --> |
| `target/release/lexaloud` (stripped) | — | <!-- `ls -lh target/release/lexaloud` --> | <!-- --> |
| `build/ui-release/lexaloud-ui` (stripped) | — | <!-- `ls -lh build/ui-release/lexaloud-ui` --> | <!-- --> |
| Staged `bin/lexaloud` + `bin/lexaloud-ui` | — | <!-- `du -sh build/stage/bin` --> | <!-- --> |
| Model cache (`~/.cache/lexaloud/models`) | <!-- `du -sh ~/.cache/lexaloud/models` --> | same (external, not bundled) | — |

**Gate:** AppDir ≤ 200 MB before external models (≥40% smaller than 336 MB baseline). Any miss requires itemized `build/staged-files.txt` + `du` report and explicit acceptance before merge.

<details><summary>Itemized size report (example)</summary>

```
$ ./scripts/build-native.sh --release --stage "$PWD/build/stage"
$ find build/stage -type f | sort > /tmp/staged-files.txt
$ du -sh build/stage build/appdir build/appimage/Lexaloud-*.AppImage
$ cat build/staged-files.txt | xargs ls -lh | sort -k5 -h
```

Paste full report here when AppDir exceeds target.

</details>

## 2. Latency — `lexaloud --help` (CLI overhead)

30 warm runs, same machine, no daemon required.

```bash
# Example harness
for i in $(seq 1 30); do /usr/bin/time -f "%e" ./target/release/lexaloud --help >/dev/null; done | sort -n
# Or: hyperfine --warmup 5 './target/release/lexaloud --help'
```

| Metric | native baseline | Rust native | Target |
|--------|-----------------|-------------|--------|
| p50 | 240–260 ms (measured) | <!-- ms --> | ≤ 50 ms p95 |
| p95 | ~260 ms | <!-- ms --> | ≤ 50 ms p95 |
| mean | <!-- --> | <!-- --> | — |
| min / max | <!-- --> | <!-- --> | — |

**Gate:** `lexaloud --help` ≤ 50 ms p95 over 30 warm runs.

## 3. Latency — `lexaloud status` (daemon-backed UDS round-trip)

Daemon warm (started once), 30 warm runs, same machine.

```bash
# Daemon must be running: systemctl --user start lexaloud.service  or  lexaloud daemon &
for i in $(seq 1 30); do /usr/bin/time -f "%e" ./target/release/lexaloud status >/dev/null; done | sort -n
```

| Metric | native baseline | Rust native | Target |
|--------|-----------------|-------------|--------|
| p50 | 363–372 ms (measured) | <!-- ms --> | ≤ 100 ms p95 |
| p95 | ~372 ms | <!-- ms --> | ≤ 100 ms p95 |
| mean | <!-- --> | <!-- --> | — |

**Gate:** `lexaloud status` ≤ 100 ms p95 over 30 warm runs.

## 4. Daemon Cold Start & Warmup

| Metric | native baseline | Rust native |
|--------|-----------------|-------------|
| Cold start (fork to UDS bind, `lexaloud daemon` to listening) | <!-- ms via systemd `ActiveEnterTimestamp` or log timestamps --> | <!-- ms --> |
| Kokoro warmup (first synthesis after start) | <!-- ms / s --> | <!-- ms / s --> |
| First audio latency (speak request to first audio callback) | <!-- ms --> | <!-- ms --> |

## 5. Synthesis Real-Time Factor (RTF)

Fixed paragraph, same model, same hardware, warm (after warmup).

| Text | native CPU RTF | Rust CPU RTF | native CUDA RTF | Rust CUDA RTF |
|------|---------------|-------------|----------------|--------------|
| 1000 chars academic | <!-- 0.15 etc. --> | <!-- --> | <!-- --> | <!-- --> |
| 5000 chars mixed | <!-- --> | <!-- --> | <!-- --> | <!-- --> |

RTF = synthesis_seconds / audio_seconds (<1.0 real-time).  
**Gate:** Warm synthesis not >5% slower than baseline on same model/backend/text/hardware.

```bash
# Example harness (stub)
cargo run --release -- speak --benchmark "Lorem ... 1000 chars" --json | jq .rtf
```

## 6. Idle RSS

| Process | native baseline | Rust native |
|---------|-----------------|-------------|
| Daemon idle (no job, after warmup) | <!-- MB via `ps -o rss -p $PID` or `smem` --> | <!-- MB --> |
| UI idle (tray + control window open, no playback) | <!-- MB --> | <!-- MB --> |
| Daemon + UI combined idle | <!-- MB --> | <!-- MB --> |

```bash
# Example
lexaloud daemon & DAEMON_PID=$!
sleep 2
ps -o pid,rss,comm -p $DAEMON_PID
# or: cat /proc/$DAEMON_PID/status | grep VmRSS
```

## 7. Throughput — End-to-End Speak

| Metric | native | Rust |
|--------|--------|------|
| `speak` request to `speaking` state (preprocessing + queue) | <!-- ms --> | <!-- ms --> |
| Pause latency (request to audio silence, ~100 ms blocks) | <!-- ms --> | <!-- ms --> |
| Skip / Back latency | <!-- ms --> | <!-- ms --> |

## 8. Repro Steps

```bash
# Clean checkout baseline (native)
git checkout fcd9d01
./scripts/build-appimage.sh  # old path; records native sizes
du -sh dist/*.AppImage build/appimage/AppDir

# Clean checkout native (Rust+Qt)
git checkout codex/rust-qt-rewrite  # or current branch
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
env -u LD_LIBRARY_PATH cmake --preset release
env -u LD_LIBRARY_PATH cmake --build --preset release --parallel
QT_QPA_PLATFORM=offscreen ctest --preset release --output-on-failure
./scripts/build-native.sh --release --stage "$PWD/build/stage"
./scripts/build-appimage.sh
./scripts/smoke-appimage.sh build/appimage/Lexaloud-*.AppImage

# Benchmarks
hyperfine --warmup 5 --runs 30 './target/release/lexaloud --help'
hyperfine --warmup 5 --runs 30 './target/release/lexaloud status'
# Fill tables above with p50/p95 from hyperfine JSON or /usr/bin/time
```

## 9. Appendix — Raw Logs

Paste `cargo build --release` timings, `ctest` output, `ldd` reports, `build/staged-files.txt`, and `hyperfine` JSON here for audit.

- `build/staged-files.txt`: <!-- attach or `cat build/staged-files.txt` -->
- `build/appdir` manifest: <!-- `find build/appdir -type f | sort` -->
- `ldd target/release/lexaloud`:
  ```
  <!-- paste ldd lexaloud -->
  ```
- `ldd build/ui-release/lexaloud-ui`:
  ```
  <!-- paste ldd lexaloud-ui -->
  ```
- `hyperfine --export-json results.json`:
  ```json
  <!-- paste -->
  ```

---

## How to Update

1. Run measurements on the reference machine (Ubuntu 24.04 or Debian 13 tier-1).
2. Fill every `<!-- -->` placeholder with a measured value and units.
3. Keep the native baseline row unchanged; add the Rust row after each rewrite milestone.
4. Commit this file; CI does not auto-generate it. The final `spikes/rust-rewrite-results.md` will be a copy with the completed native column.

