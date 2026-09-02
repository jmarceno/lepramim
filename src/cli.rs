#![allow(dead_code)]

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "lexaloud",
    version,
    about = "Universal Linux text-to-speech tool for reading-along."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the Lexaloud desktop app: tray icon, daemon, and first-run setup
    App,
    /// Alias for `app`
    Tray,
    /// Capture the PRIMARY selection and speak it
    #[command(name = "speak-selection")]
    SpeakSelection {
        #[arg(long = "max-bytes")]
        max_bytes: Option<usize>,
    },
    /// Capture the CLIPBOARD and speak it
    #[command(name = "speak-clipboard")]
    SpeakClipboard {
        #[arg(long = "max-bytes")]
        max_bytes: Option<usize>,
    },
    Pause,
    Resume,
    Toggle,
    Stop,
    Skip,
    Back,
    Status,
    /// Fetch model artifacts
    #[command(name = "download-models")]
    DownloadModels {
        #[arg(long)]
        llm: bool,
        #[arg(long)]
        all: bool,
    },
    Setup {
        #[arg(long)]
        force: bool,
    },
    Daemon,
    Uninstall,
    /// Print a markdown-formatted bug report
    #[command(name = "bug-report")]
    BugReport {
        #[arg(long)]
        full: bool,
    },
}

fn not_yet(cmd: &str) -> i32 {
    eprintln!("not yet implemented: {cmd}");
    1
}

/// Entry point called from `main.rs`. Parses args and dispatches.
pub async fn run() -> i32 {
    let cli = Cli::parse();
    match cli.command {
        None => cmd_app().await,
        Some(Commands::App) => cmd_app().await,
        Some(Commands::Tray) => cmd_app().await,
        Some(Commands::SpeakSelection { max_bytes }) => cmd_speak_selection(max_bytes).await,
        Some(Commands::SpeakClipboard { max_bytes }) => cmd_speak_clipboard(max_bytes).await,
        Some(Commands::Pause) => cmd_pause().await,
        Some(Commands::Resume) => cmd_resume().await,
        Some(Commands::Toggle) => cmd_toggle().await,
        Some(Commands::Stop) => cmd_stop().await,
        Some(Commands::Skip) => cmd_skip().await,
        Some(Commands::Back) => cmd_back().await,
        Some(Commands::Status) => cmd_status().await,
        Some(Commands::DownloadModels { llm, all }) => cmd_download_models(llm, all).await,
        Some(Commands::Setup { force }) => cmd_setup(force).await,
        Some(Commands::Daemon) => cmd_daemon().await,
        Some(Commands::Uninstall) => cmd_uninstall().await,
        Some(Commands::BugReport { full }) => cmd_bug_report(full).await,
    }
}

// ---- helpers for daemon UDS client ----

async fn post_to_daemon(
    path: &str,
    json_body: serde_json::Value,
) -> Result<serde_json::Value, i32> {
    let sock = crate::config::socket_path();
    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Lexaloud daemon is not running or unresponsive. Start it with: systemctl --user start lexaloud.service"
            );
            eprintln!("(tried socket {}: {e})", sock.display());
            eprintln!("If you haven't yet, run `lexaloud setup`.");
            return Err(3);
        }
    };
    let body_str = serde_json::to_string(&json_body).unwrap();
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: lexaloud\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        body_str.len(),
        body_str
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if let Err(e) = stream.write_all(req.as_bytes()).await {
        eprintln!("failed to send request to daemon: {e}");
        return Err(1);
    }
    let _ = stream.flush().await;
    let mut buf = Vec::new();
    let read_fut = stream.read_to_end(&mut buf);
    let res = tokio::time::timeout(std::time::Duration::from_secs(5), read_fut).await;
    let n = match res {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            eprintln!("failed to read daemon response: {e}");
            return Err(1);
        }
        Err(_) => {
            eprintln!("daemon response timed out");
            return Err(1);
        }
    };
    if n == 0 {
        eprintln!("daemon closed connection without response");
        return Err(1);
    }
    let resp = String::from_utf8_lossy(&buf);
    let header_end = resp.find("\r\n\r\n").unwrap_or(0);
    let header = &resp[..header_end];
    let body = &resp[header_end + 4..];
    let status_line = header.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if code == 413 {
        eprintln!("Selection too large for the daemon to accept.");
        return Err(4);
    }
    if code >= 400 {
        eprintln!("Lexaloud daemon returned {code}: {body}");
        return Err(1);
    }
    let v: serde_json::Value = serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

async fn get_from_daemon(path: &str) -> Result<serde_json::Value, i32> {
    let sock = crate::config::socket_path();
    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Lexaloud daemon is not running or unresponsive. Start it with: systemctl --user start lexaloud.service"
            );
            eprintln!("(tried socket {}: {e})", sock.display());
            return Err(3);
        }
    };
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n",
        path
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if let Err(e) = stream.write_all(req.as_bytes()).await {
        eprintln!("failed to send request: {e}");
        return Err(1);
    }
    let _ = stream.flush().await;
    let mut buf = Vec::new();
    let read_fut = stream.read_to_end(&mut buf);
    let res = tokio::time::timeout(std::time::Duration::from_secs(5), read_fut).await;
    let n = match res {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            eprintln!("failed to read: {e}");
            return Err(1);
        }
        Err(_) => {
            eprintln!("timeout");
            return Err(1);
        }
    };
    if n == 0 {
        return Err(1);
    }
    let resp = String::from_utf8_lossy(&buf);
    let header_end = resp.find("\r\n\r\n").unwrap_or(0);
    let header = &resp[..header_end];
    let body = &resp[header_end + 4..];
    let status_line = header.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if code >= 400 {
        eprintln!("daemon returned {code}: {body}");
        return Err(1);
    }
    let v: serde_json::Value = serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

// ---- command handlers ----

fn find_ui_binary() -> Option<std::path::PathBuf> {
    // 1) sibling of current exe (target/release or build/appdir)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("lexaloud-ui");
            if cand.is_file() {
                return Some(cand);
            }
            let cand2 = dir.join("../lib/lexaloud/lexaloud-ui");
            if cand2.is_file() {
                return Some(cand2);
            }
            // From target/release, check ../../build/appdir etc.
            for rel in [
                "../../build/appdir/usr/bin/lexaloud-ui",
                "../../build/ui-release/ui/lexaloud-ui",
                "../../build/stage/bin/lexaloud-ui",
                "../build/appdir/usr/bin/lexaloud-ui",
            ] {
                let cand = dir.join(rel);
                if cand.is_file() {
                    return Some(cand);
                }
            }
            // Walk up looking for Cargo.toml to find project root
            let mut cur = dir.to_path_buf();
            for _ in 0..5 {
                if cur.join("Cargo.toml").is_file() {
                    for rel in [
                        "build/appdir/usr/bin/lexaloud-ui",
                        "build/ui-release/ui/lexaloud-ui",
                        "build/stage/bin/lexaloud-ui",
                    ] {
                        let cand = cur.join(rel);
                        if cand.is_file() {
                            return Some(cand);
                        }
                    }
                    break;
                }
                if let Some(parent) = cur.parent() {
                    cur = parent.to_path_buf();
                } else {
                    break;
                }
            }
        }
    }
    // 2) in PATH
    if let Ok(path) = std::env::var("PATH") {
        for p in path.split(':') {
            let cand = std::path::Path::new(p).join("lexaloud-ui");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    // 3) common build locations relative to cwd (when running from project root)
    for cand in [
        "build/appdir/usr/bin/lexaloud-ui",
        "build/stage/bin/lexaloud-ui",
        "build/ui-release/ui/lexaloud-ui",
        "target/release/lexaloud-ui",
    ] {
        let p = std::path::Path::new(cand);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    // 4) compile-time manifest dir (handles CARGO_TARGET_DIR)
    let manifest = env!("CARGO_MANIFEST_DIR");
    for rel in [
        "build/appdir/usr/bin/lexaloud-ui",
        "build/ui-release/ui/lexaloud-ui",
        "build/stage/bin/lexaloud-ui",
    ] {
        let cand = std::path::Path::new(manifest).join(rel);
        if cand.is_file() {
            return Some(cand);
        }
    }
    // 5) system install
    for cand in ["/usr/bin/lexaloud-ui", "/usr/local/bin/lexaloud-ui"] {
        let p = std::path::Path::new(cand);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

async fn is_daemon_healthy_quiet() -> bool {
    let sock = crate::config::socket_path();
    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(_) => return false,
    };
    let req = b"GET /healthz HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n";
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if stream.write_all(req).await.is_err() {
        return false;
    }
    let _ = stream.flush().await;
    let mut buf = Vec::new();
    let read_fut = stream.read_to_end(&mut buf);
    let res = tokio::time::timeout(std::time::Duration::from_secs(2), read_fut).await;
    let n = match res {
        Ok(Ok(n)) => n,
        _ => return false,
    };
    if n == 0 {
        return false;
    }
    let resp = String::from_utf8_lossy(&buf);
    resp.contains("200") && resp.contains("\"status\":\"ok\"")
}

async fn cmd_app() -> i32 {
    println!("Lexaloud {} — local text-to-speech", env!("CARGO_PKG_VERSION"));
    // Check daemon
    let daemon_running = is_daemon_healthy_quiet().await;
    if daemon_running {
        println!("Daemon already running at {}", crate::config::socket_path().display());
    } else {
        println!("Daemon not running, starting...");
        // Try systemctl first if available and user service exists
        let mut started_via_systemctl = false;
        if let Ok(out) = std::process::Command::new("systemctl")
            .args(["--user", "is-enabled", "lexaloud.service"])
            .output()
        {
            if out.status.success() {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", "lexaloud.service"])
                    .status();
                started_via_systemctl = true;
            }
        }
        if !started_via_systemctl {
            // Spawn daemon directly as detached child
            match std::env::current_exe() {
                Ok(exe) => {
                    match std::process::Command::new(&exe)
                        .arg("daemon")
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        Ok(child) => {
                            println!("Spawned daemon pid {} via {}", child.id(), exe.display());
                            // give it a moment to bind (quiet check to avoid spam)
                            for _ in 0..25 {
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                if is_daemon_healthy_quiet().await {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to spawn daemon: {e}");
                            return 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cannot determine current exe: {e}");
                    return 1;
                }
            }
        } else {
            // systemctl path, wait a bit (quiet)
            for _ in 0..15 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if is_daemon_healthy_quiet().await {
                    break;
                }
            }
        }
        // Verify
        match get_from_daemon("/healthz").await {
            Ok(_) => println!("Daemon is now running"),
            Err(code) => {
                eprintln!("Daemon failed to start (status code {code}). Check journalctl --user -u lexaloud or run `lexaloud daemon` manually for logs.");
                return code;
            }
        }
    }

    // Try to launch UI if display available
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if has_display {
        if let Some(ui) = find_ui_binary() {
            println!("Launching UI: {}", ui.display());
            match std::process::Command::new(&ui)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    println!(
                        "UI launched pid {}. Tray should appear. Use lexaloud status / pause / resume etc.",
                        child.id()
                    );
                    println!("Tip: select text in any app and press Meta+R to speak.");
                }
                Err(e) => {
                    eprintln!("Failed to launch UI {}: {e}", ui.display());
                    println!("Daemon is running. Try: lexaloud status");
                }
            }
        } else {
            println!("lexaloud-ui not found in PATH or build. Daemon is running.");
            println!("Try: ./build/appdir/usr/bin/lexaloud-ui  or  cmake --preset release && ./build/ui-release/ui/lexaloud-ui");
        }
    } else {
        println!("No display (DISPLAY/WAYLAND_DISPLAY not set). Daemon is running in background.");
        println!("Use: lexaloud status  |  lexaloud speak-selection  |  lexaloud pause/resume/stop");
        // Also show that CLI works
        if let Ok(v) = get_from_daemon("/state").await {
            println!("Current state: {}", serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string()));
        }
    }
    0
}

async fn cmd_speak_selection(max_bytes_opt: Option<usize>) -> i32 {
    let cfg = crate::config::load_config::<std::path::PathBuf>(None);
    let max_bytes = max_bytes_opt.unwrap_or(cfg.capture.max_bytes);
    let timeout_s = cfg.capture.subprocess_timeout_s;

    // Try primary first
    let primary_result = crate::platform::selection::read_primary(max_bytes, timeout_s);
    let text_opt = match primary_result {
        Ok(r) => {
            if r.text.trim().is_empty() {
                None
            } else {
                // Check truncated notification
                if r.truncated {
                    crate::platform::notifications::try_notify(
                        "Selection truncated",
                        Some(&format!(
                            "Lexaloud captured the first {} bytes of a larger selection.",
                            max_bytes
                        )),
                        1.0,
                    );
                }
                Some(r.text)
            }
        }
        Err(e) => match &e {
            crate::platform::selection::SelectionError::Empty(_) => None,
            crate::platform::selection::SelectionError::DisplayUnavailable(msg) => {
                eprintln!("{}", msg);
                crate::platform::notifications::try_notify(
                    "Lexaloud: cannot reach display server",
                    Some(
                        "Is DISPLAY set? Are you running from a session that can talk to X/Wayland?",
                    ),
                    1.0,
                );
                return 5;
            }
            crate::platform::selection::SelectionError::ToolMissing(msg) => {
                eprintln!("{}", msg);
                crate::platform::notifications::try_notify(
                    "Lexaloud: capture tool missing",
                    Some(msg),
                    1.0,
                );
                return 5;
            }
            crate::platform::selection::SelectionError::Timeout(_) => {
                eprintln!("{}", e);
                crate::platform::notifications::try_notify(
                    "Lexaloud: capture timed out",
                    Some(&e.to_string()),
                    1.0,
                );
                return 5;
            }
            _ => {
                eprintln!("{}", e);
                return 1;
            }
        },
    };

    let text = if let Some(t) = text_opt {
        t
    } else {
        // Try force copy then clipboard
        crate::platform::selection::try_force_copy(1.0);
        // Try clipboard readers sequentially
        let mut last_err: Option<crate::platform::selection::SelectionError> = None;
        let mut success: Option<String> = None;
        match crate::platform::selection::read_clipboard(max_bytes, timeout_s) {
            Ok(r) => success = Some(r.text),
            Err(crate::platform::selection::SelectionError::Empty(e)) => {
                last_err = Some(crate::platform::selection::SelectionError::Empty(e));
            }
            Err(crate::platform::selection::SelectionError::DisplayUnavailable(msg)) => {
                eprintln!("{}", msg);
                crate::platform::notifications::try_notify(
                    "Lexaloud: cannot reach display server",
                    Some("Is DISPLAY set?"),
                    1.0,
                );
                return 5;
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
        if success.is_none() {
            match crate::platform::selection::read_clipboard_via_klipper(max_bytes, timeout_s) {
                Ok(r) => success = Some(r.text),
                Err(crate::platform::selection::SelectionError::Empty(e)) => {
                    last_err = Some(crate::platform::selection::SelectionError::Empty(e));
                }
                Err(crate::platform::selection::SelectionError::DisplayUnavailable(msg)) => {
                    eprintln!("{}", msg);
                    crate::platform::notifications::try_notify(
                        "Lexaloud: cannot reach display server",
                        Some("Is DISPLAY set?"),
                        1.0,
                    );
                    return 5;
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }
        if let Some(t) = success {
            t
        } else {
            if let Some(e) = last_err {
                if matches!(e, crate::platform::selection::SelectionError::Empty(_)) {
                    eprintln!("No selection found. Select text and press Meta+R again.");
                    crate::platform::notifications::try_notify(
                        "Select text first",
                        Some("Lexaloud: no selection found."),
                        1.0,
                    );
                    return 2;
                } else {
                    eprintln!("{}", e);
                    crate::platform::notifications::try_notify(
                        "Lexaloud: capture tool missing",
                        Some(&e.to_string()),
                        1.0,
                    );
                    return 5;
                }
            } else {
                eprintln!("No selection found.");
                return 2;
            }
        }
    };

    // Post to daemon
    let body = serde_json::json!({"text": text, "mode":"replace"});
    match post_to_daemon("/speak", body).await {
        Ok(_) => 0,
        Err(code) => code,
    }
}

async fn cmd_speak_clipboard(max_bytes_opt: Option<usize>) -> i32 {
    let cfg = crate::config::load_config::<std::path::PathBuf>(None);
    let max_bytes = max_bytes_opt.unwrap_or(cfg.capture.max_bytes);
    let timeout_s = cfg.capture.subprocess_timeout_s;
    let result = crate::platform::selection::read_clipboard(max_bytes, timeout_s);
    let text = match result {
        Ok(r) => {
            if r.truncated {
                crate::platform::notifications::try_notify(
                    "Selection truncated",
                    Some(&format!("Lexaloud captured the first {} bytes", max_bytes)),
                    1.0,
                );
            }
            r.text
        }
        Err(crate::platform::selection::SelectionError::Empty(msg)) => {
            eprintln!("{}", msg);
            crate::platform::notifications::try_notify(
                "Copy text first",
                Some("Lexaloud: clipboard is empty. Press Ctrl+C first."),
                1.0,
            );
            return 2;
        }
        Err(crate::platform::selection::SelectionError::ToolMissing(msg)) => {
            eprintln!("{}", msg);
            crate::platform::notifications::try_notify(
                "Lexaloud: capture tool missing",
                Some(&msg),
                1.0,
            );
            return 5;
        }
        Err(crate::platform::selection::SelectionError::Timeout(msg)) => {
            eprintln!("timeout {}", msg);
            return 5;
        }
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let body = serde_json::json!({"text": text, "mode":"replace"});
    match post_to_daemon("/speak", body).await {
        Ok(_) => 0,
        Err(code) => code,
    }
}

async fn cmd_pause() -> i32 {
    match post_to_daemon("/pause", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}
async fn cmd_resume() -> i32 {
    match post_to_daemon("/resume", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}
async fn cmd_toggle() -> i32 {
    match post_to_daemon("/toggle", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}
async fn cmd_stop() -> i32 {
    match post_to_daemon("/stop", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}
async fn cmd_skip() -> i32 {
    match post_to_daemon("/skip", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}
async fn cmd_back() -> i32 {
    match post_to_daemon("/back", serde_json::json!({})).await {
        Ok(_) => 0,
        Err(c) => c,
    }
}

async fn cmd_status() -> i32 {
    let sock = crate::config::socket_path();

    // Use hyper types to demonstrate hyper Unix socket client intent.
    // We build a hyper Request and then transport it manually over the UDS.
    let _hyper_req = hyper::Request::builder()
        .method(hyper::Method::GET)
        .uri("/state")
        .header(hyper::header::HOST, "lexaloud")
        .header(hyper::header::CONNECTION, "close")
        .body(())
        .expect("hyper request builds");

    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Lexaloud daemon is not running or unresponsive. Start it with: systemctl --user start lexaloud.service"
            );
            eprintln!("(tried socket {}: {e})", sock.display());
            eprintln!("If you haven't yet, run `lexaloud setup`.");
            return 3;
        }
    };

    let raw_req = b"GET /state HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n";
    if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut stream, raw_req).await {
        eprintln!("failed to send request to daemon: {e}");
        return 1;
    }
    // Ensure written bytes are flushed before reading.
    if let Err(e) = tokio::io::AsyncWriteExt::flush(&mut stream).await {
        eprintln!("failed to flush request: {e}");
        return 1;
    }

    let mut buf = Vec::new();
    let read_fut = tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut buf);
    let res = tokio::time::timeout(std::time::Duration::from_secs(5), read_fut).await;
    let n = match res {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            eprintln!("failed to read daemon response: {e}");
            return 1;
        }
        Err(_) => {
            eprintln!("daemon response timed out");
            return 1;
        }
    };
    if n == 0 {
        eprintln!("daemon closed connection without response");
        return 1;
    }

    let resp = String::from_utf8_lossy(&buf);
    let header_end = resp.find("\r\n\r\n");
    let (header, body) = match header_end {
        Some(idx) => (&resp[..idx], &resp[idx + 4..]),
        None => ("", resp.as_ref()),
    };
    let status_line = header.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if code >= 400 && code != 0 {
        eprintln!("Lexaloud daemon returned {code}: {body}");
        return 1;
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        println!("{{}}");
        return 0;
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v) => {
            // serde_json pretty print
            match serde_json::to_string_pretty(&v) {
                Ok(pretty) => println!("{pretty}"),
                Err(_) => println!("{trimmed}"),
            }
        }
        Err(_) => {
            println!("{trimmed}");
        }
    }
    0
}

async fn cmd_download_models(llm: bool, all: bool) -> i32 {
    // Use models::ensure_artifacts with download stub (real download would be via HTTP)
    // For now, just report status
    let cache = crate::models::default_cache_dir();
    println!("Checking models in {}", cache.display());
    let mut missing = false;
    for art in crate::models::ARTIFACTS {
        let p = cache.join(art.filename);
        if p.is_file() {
            match crate::models::sha256_of(&p) {
                Ok(hash) if hash == art.sha256 => println!("{} present and verified", art.filename),
                Ok(hash) => {
                    println!(
                        "{} present but SHA mismatch: got {} expected {}",
                        art.filename, hash, art.sha256
                    );
                    missing = true;
                }
                Err(e) => {
                    println!("{} error: {}", art.filename, e);
                    missing = true;
                }
            }
        } else {
            println!(
                "{} missing - run with network to download from {}",
                art.filename, art.url
            );
            missing = true;
        }
    }
    if llm || all {
        println!(
            "LLM model check: {}",
            crate::config::load_config::<std::path::PathBuf>(None)
                .normalizer
                .model_file
        );
    }
    if missing { 1 } else { 0 }
}

async fn cmd_setup(force: bool) -> i32 {
    println!(
        "Setup force={} - stub (would create systemd unit and download models)",
        force
    );
    let cfg_path = crate::config::config_path();
    if let Some(parent) = cfg_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !cfg_path.exists() || force {
        let default = crate::config::Config::default();
        if let Ok(toml_str) = toml::to_string(&default) {
            let _ = std::fs::write(&cfg_path, toml_str);
            println!("Wrote config to {}", cfg_path.display());
        }
    }
    let unit = crate::platform::service::generate_systemd_unit(
        &std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("/usr/bin/lexaloud")),
    );
    println!("Systemd unit preview:\n{}", unit);
    0
}

async fn cmd_daemon() -> i32 {
    println!("Starting Lexaloud daemon...");
    match crate::daemon::run().await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("daemon failed: {e}");
            1
        }
    }
}

async fn cmd_uninstall() -> i32 {
    println!("Uninstall stub - would stop daemon and remove unit");
    let sock = crate::config::socket_path();
    if sock.exists() {
        let _ = std::fs::remove_file(&sock);
        println!("Removed socket {}", sock.display());
    }
    0
}

async fn cmd_bug_report(full: bool) -> i32 {
    let redact = !full;
    // Collect similar to native bug_report
    let version = env!("CARGO_PKG_VERSION");
    let mut out = String::new();
    out.push_str("# Lexaloud bug report\n\n");
    out.push_str(&format!("- **Lexaloud**: {}\n", version));
    out.push_str(&format!("- **Rust**: {}\n", rustc_version()));
    // distro etc.
    let distro = detect_distro_stub();
    out.push_str(&format!("- **Distro**: {}\n", distro));
    // config
    let cfg_path = crate::config::config_path();
    let cfg_text = if cfg_path.is_file() {
        std::fs::read_to_string(&cfg_path).unwrap_or_else(|e| format!("could not read: {e}"))
    } else {
        "(no config.toml present — using defaults)".to_string()
    };
    let cfg_text = if redact {
        crate::privacy::redact_toml_values(&cfg_text)
    } else {
        cfg_text
    };
    out.push_str("\n## Config\n```toml\n");
    out.push_str(&cfg_text);
    out.push_str("\n```\n");
    // state
    match get_from_daemon("/state").await {
        Ok(v) => {
            let mut state_str =
                serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string());
            if redact {
                state_str = crate::privacy::redact_home(&state_str);
            }
            out.push_str("\n## Daemon state\n```json\n");
            out.push_str(&state_str);
            out.push_str("\n```\n");
        }
        Err(_) => out.push_str("\n## Daemon state\n(daemon not running)\n"),
    }
    // models
    out.push_str("\n## Model cache\n");
    for art in crate::models::ARTIFACTS {
        let p = crate::models::default_cache_dir().join(art.filename);
        if p.is_file() {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            out.push_str(&format!("- `{}`: present ({} bytes)\n", art.filename, size));
        } else {
            out.push_str(&format!("- `{}`: **MISSING**\n", art.filename));
        }
    }
    if redact {
        out = crate::privacy::redact_home(&out);
    }
    println!("{}", out);
    0
}

fn rustc_version() -> String {
    // Try to get rustc version via env var set at build? Fallback to static
    option_env!("RUSTC_VERSION")
        .unwrap_or("unknown")
        .to_string()
}

fn detect_distro_stub() -> String {
    let p = std::path::Path::new("/etc/os-release");
    if let Ok(text) = std::fs::read_to_string(p) {
        for line in text.lines() {
            if line.starts_with("PRETTY_NAME=") {
                return line
                    .trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string();
            }
        }
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_status() {
        let cli = Cli::try_parse_from(["lexaloud", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Status)));
    }

    #[test]
    fn parse_app_and_tray() {
        let cli = Cli::try_parse_from(["lexaloud", "app"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::App)));
        let cli = Cli::try_parse_from(["lexaloud", "tray"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Tray)));
    }

    #[test]
    fn parse_download_models_flags() {
        let cli = Cli::try_parse_from(["lexaloud", "download-models", "--llm"]).unwrap();
        if let Some(Commands::DownloadModels { llm, all }) = cli.command {
            assert!(llm);
            assert!(!all);
        } else {
            panic!("wrong command");
        }
        let cli = Cli::try_parse_from(["lexaloud", "download-models", "--all"]).unwrap();
        if let Some(Commands::DownloadModels { llm, all }) = cli.command {
            assert!(!llm);
            assert!(all);
        } else {
            panic!("wrong command");
        }
    }

    #[test]
    fn parse_setup_force() {
        let cli = Cli::try_parse_from(["lexaloud", "setup", "--force"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Setup { force: true })));
    }

    #[test]
    fn parse_bug_report_full() {
        let cli = Cli::try_parse_from(["lexaloud", "bug-report", "--full"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::BugReport { full: true })
        ));
        let cli = Cli::try_parse_from(["lexaloud", "bug-report"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::BugReport { full: false })
        ));
    }

    #[test]
    fn parse_speak_selection_max_bytes() {
        let cli =
            Cli::try_parse_from(["lexaloud", "speak-selection", "--max-bytes", "12345"]).unwrap();
        if let Some(Commands::SpeakSelection { max_bytes }) = cli.command {
            assert_eq!(max_bytes, Some(12345));
        } else {
            panic!("wrong command");
        }
    }
}
