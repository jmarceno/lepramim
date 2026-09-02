# Troubleshooting

The fastest way to get help is to run `lexaloud bug-report` and paste
the output into a GitHub issue. The rest of this file covers common
symptoms you can fix yourself.

## `lexaloud: command not found`

The installer puts `lexaloud` at
`~/.local/bin/lexaloud` (or `/usr/local/bin/lexaloud` for system installs).
Either invoke the full path or ensure the directory is on `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
which lexaloud
```

For AppImage installs, the wrapper is at `~/.local/bin/lexaloud` pointing to the AppImage.

## `exit 3: Lexaloud daemon is not running`

```bash
systemctl --user status lexaloud.service
journalctl --user -u lexaloud.service -n 50 --no-pager
```

Common causes:
- The daemon crashed on startup. Look at the tail of the journal for
  the Rust backtrace / log.
- systemd --user isn't running for your session (rare on modern GNOME
  but possible in minimal environments). Check with `systemctl --user`.
- The unit file is stale. Run `lexaloud setup --force` to regenerate it,
  then `systemctl --user daemon-reload && systemctl --user restart
  lexaloud.service`.

## `exit 2: Select text first` (but I DID select text)

You're probably on GNOME Wayland and the app you're selecting text in
doesn't publish to the PRIMARY selection. Workaround: use
`lexaloud speak-clipboard` and press `Ctrl+C` before the hotkey. See
[`gotchas.md`](gotchas.md) for the full list of known-bad apps.

## CUDA silently fell back to CPU

In the daemon logs:

```
Requested CUDAExecutionProvider but session reports ['CPUExecutionProvider'].
```

The CUDA runtime wasn't available. Check `lexaloud status` — `session_providers`
will show only `CPUExecutionProvider`. Verify NVIDIA driver:

```bash
nvidia-smi -L
ldconfig -p | grep -i cuda
```

If you installed the CPU build by mistake, rebuild with CUDA support or use the
CPU fallback (which works at ~10x real-time and is fine for reading along).

## Voice sounds robotic / glitchy

Check `lexaloud status`: the `provider_name` should be `kokoro`. If
it's anything else you fell back to a different state (that shouldn't
happen in v0.1.x but worth verifying).

If it's `kokoro` but the audio is choppy, your CPU is probably
struggling to keep up. Try lowering `speed` in `config.toml` to 1.0 or
below, or use the CUDA build if you have an NVIDIA GPU.

## Model download failed

```bash
rm -rf ~/.cache/lexaloud/models
lexaloud download-models
```

If the download hangs, the artifact URL may have moved. File an issue
with the error message — the pinned URL + SHA256 hash is in
`src/models.rs` and needs to be updated if upstream moves
the files.

## Pause / skip / back doesn't work

- The daemon must be running.
- Pause takes effect at the next sub-chunk boundary (~100 ms).
- `skip`/`back` only work while the state is `speaking` or `paused`
  (not `idle` or `warming`).

## Tray indicator doesn't appear

On GNOME 46+ you need the `ubuntu-appindicators` Shell Extension
(installed and enabled by default on Ubuntu, optional on Fedora / Arch).
Install it from https://extensions.gnome.org/extension/615/ if it's
missing.

To launch the indicator manually:

```bash
lexaloud-ui
# or
lexaloud app
```

Watch the terminal for errors. On Fedora/Arch, also check that the
desktop's tray extension (e.g. `ubuntu-appindicators` on GNOME) is
enabled. Qt 6 must be installed for the tray to render.

## `Selection too large for the daemon to accept` (exit 4)

The default selection cap is 200 KB. Raise `capture.max_bytes` in
`config.toml` and restart the daemon, or use
`lexaloud speak-selection --max-bytes 500000`.

## Absolutely nothing works

```bash
lexaloud bug-report > /tmp/lexaloud-bug.md
```

Open a GitHub issue and paste the output. Include what you tried.

## Native build fails

```bash
# Check toolchain
rustc --version  # should be 1.85+
cmake --version  # 3.21+
qmake6 --version || qmake --version
cargo --version
clang-format --version

# Clean rebuild
cargo clean
rm -rf build/ui-*
./scripts/build-native.sh --release
```
