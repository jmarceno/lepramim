# Security policy

## Reporting a vulnerability

Please report security issues through
[GitHub private vulnerability reporting][advisory]. This is the preferred
channel and is monitored actively.

[advisory]: https://github.com/Gustavjiversen01/lexaloud/security/advisories/new

As a fallback, email **lexaloud-conduct@proton.me**.

Please allow up to seven days for an initial response. For critical issues
affecting a running Lexaloud deployment, also indicate in the advisory
whether embargo coordination is needed.

## Supported versions

Security fixes are backported to the latest minor version only.

| Version | Supported |
|---------|-----------|
| 0.2.x   | ✅        |
| < 0.2   | ❌        |

## Scope

Lexaloud runs entirely on the user's local machine. It does not make
outbound network requests except for:

1. First-run model downloads from the `kokoro-onnx` GitHub releases page
   (`https://github.com/thewh1teagle/kokoro-onnx/releases`), SHA256-pinned
   in `src/models.rs`.
2. Nothing else. No telemetry. No usage reporting. No crash reporting.

The daemon binds a **Unix domain socket** at
`$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` with mode 0700 via
`systemd.service`'s `RuntimeDirectory=` and `RuntimeDirectoryMode=`
directives. Only the owner user's processes can reach the socket.
The daemon is a native Rust binary (Axum + Tokio) with no embedded interpreter.

### In scope
- Privilege escalation from a local unprivileged process through the
  daemon's HTTP API
- Untrusted-input crashes, resource exhaustion, or infinite loops via
  `/speak`, `/pause`, `/resume`, `/stop`, `/skip`, `/back`, `/toggle`
- Concerns about pinned native dependencies (report through this channel
  so we can coordinate with upstream)
- Issues in the model-download integrity check or the ONNX Runtime
  environment guard in `src/models.rs`

### Out of scope
- Vulnerabilities in third-party TTS models, phonemizers (eSpeak NG), or CUDA
  runtime libraries (report those to their upstreams: `kokoro-onnx`,
  eSpeak NG, NVIDIA)
- Social engineering against a contributor
- Physical access attacks against the user's machine

## Disclosure preference

We follow standard coordinated disclosure. Upon report, we will acknowledge
receipt within seven days and work with the reporter on a remediation
timeline. When a fix is shipped, the reporter is credited in `CHANGELOG.md`
unless they prefer to remain anonymous.

## Hardening notes

- The daemon validates `Content-Length` before reading the body (cap at
  `capture.max_bytes + 4096`).
- Sentences exceeding `MAX_SENTENCE_CHARS` are rejected with 400.
- Log lines that mention user text replace content with a SHA-1 fingerprint + length.
- The Qt UI communicates via `QLocalSocket` over the same UDS; no TCP port is opened.
