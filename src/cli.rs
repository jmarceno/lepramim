use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "lepramim",
    version,
    about = "Universal Linux text-to-speech tool for reading-along."
)]
pub struct Cli {
    /// Show control window on start
    #[arg(long, global = true)]
    pub control: bool,
    /// Show floating overlay on start
    #[arg(long, global = true)]
    pub overlay: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the Lepramim desktop app: tray icon, daemon, and first-run setup
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
    DownloadModels,
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

/// Entry point for non-app subcommands (called from `main.rs` after clap parse).
pub async fn run_async(cli: Cli) -> i32 {
    match cli.command {
        None | Some(Commands::App) | Some(Commands::Tray) => {
            // App mode is handled in main via ui::run
            unreachable!("app mode handled in main")
        }
        Some(Commands::SpeakSelection { max_bytes }) => cmd_speak_selection(max_bytes).await,
        Some(Commands::SpeakClipboard { max_bytes }) => cmd_speak_clipboard(max_bytes).await,
        Some(Commands::Pause) => cmd_pause().await,
        Some(Commands::Resume) => cmd_resume().await,
        Some(Commands::Toggle) => cmd_toggle().await,
        Some(Commands::Stop) => cmd_stop().await,
        Some(Commands::Skip) => cmd_skip().await,
        Some(Commands::Back) => cmd_back().await,
        Some(Commands::Status) => cmd_status().await,
        Some(Commands::DownloadModels) => cmd_download_models().await,
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
                "Could not reach Lepramim ({}). Open the AppImage first.",
                sock.display()
            );
            eprintln!("({e})");
            return Err(3);
        }
    };
    let body_str = serde_json::to_string(&json_body).unwrap();
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: lepramim\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
        eprintln!("Lepramim daemon returned {code}: {body}");
        return Err(1);
    }
    let v: serde_json::Value = serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

async fn get_from_daemon(path: &str) -> Result<serde_json::Value, i32> {
    let sock = crate::config::socket_path();
    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(_) => {
            eprintln!(
                "Could not reach Lepramim ({}). Open the AppImage first.",
                sock.display()
            );
            return Err(3);
        }
    };
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: lepramim\r\nConnection: close\r\n\r\n",
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

async fn is_daemon_healthy_quiet() -> bool {
    let sock = crate::config::socket_path();
    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(_) => return false,
    };
    let req = b"GET /healthz HTTP/1.1\r\nHost: lepramim\r\nConnection: close\r\n\r\n";
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

fn ensure_user_files() -> Result<(), String> {
    let cfg_path = crate::config::config_path();
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("config dir: {e}"))?;
    }
    if !cfg_path.exists() {
        let default = crate::config::Config::default();
        let toml_str = toml::to_string(&default).map_err(|e| format!("serialize config: {e}"))?;
        std::fs::write(&cfg_path, toml_str).map_err(|e| format!("write config: {e}"))?;
    }
    crate::models::ensure_artifacts(None, true).map_err(|e| e.to_string())?;
    Ok(())
}

async fn request_daemon_shutdown(mut child: Option<std::process::Child>) {
    let sock = crate::config::socket_path();
    if let Ok(mut stream) = tokio::net::UnixStream::connect(&sock).await {
        let body = "{}";
        let req = format!(
            "POST /shutdown HTTP/1.1\r\nHost: lepramim\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        use tokio::io::AsyncWriteExt;
        let _ = stream.write_all(req.as_bytes()).await;
        let _ = stream.flush().await;
    }
    for _ in 0..25 {
        if !is_daemon_healthy_quiet().await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    if let Some(ref mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
}

async fn cmd_speak_selection(max_bytes_opt: Option<usize>) -> i32 {
    let cfg = crate::config::load_config::<std::path::PathBuf>(None);
    let max_bytes = max_bytes_opt.unwrap_or(cfg.capture.max_bytes);
    let timeout_s = cfg.capture.subprocess_timeout_s;

    let capture = match crate::ui::capture::capture_for_cli(max_bytes, timeout_s) {
        Ok(r) => r,
        Err(crate::platform::selection::SelectionError::Empty(_)) => {
            eprintln!("No selection found. Select text and press Meta+R again.");
            crate::platform::notifications::try_notify(
                "Select text first",
                Some("Lepramim: no selection found."),
                1.0,
            );
            return 2;
        }
        Err(crate::platform::selection::SelectionError::DisplayUnavailable(msg)) => {
            eprintln!("{msg}");
            crate::platform::notifications::try_notify(
                "Lepramim: cannot reach display server",
                Some("Is DISPLAY set? Are you running from a session that can talk to X/Wayland?"),
                1.0,
            );
            return 5;
        }
        Err(crate::platform::selection::SelectionError::ToolMissing(msg)) => {
            eprintln!("{msg}");
            crate::platform::notifications::try_notify(
                "Lepramim: capture tool missing",
                Some(&msg),
                1.0,
            );
            return 5;
        }
        Err(crate::platform::selection::SelectionError::Timeout(t)) => {
            eprintln!("capture timed out after {t}s");
            crate::platform::notifications::try_notify(
                "Lepramim: capture timed out",
                Some(&format!("capture timed out after {t}s")),
                1.0,
            );
            return 5;
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    if capture.truncated {
        crate::platform::notifications::try_notify(
            "Selection truncated",
            Some(&format!(
                "Lepramim captured the first {} bytes of a larger selection.",
                max_bytes
            )),
            1.0,
        );
    }

    let body = serde_json::json!({"text": capture.text, "mode":"replace"});
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
                    Some(&format!("Lepramim captured the first {} bytes", max_bytes)),
                    1.0,
                );
            }
            r.text
        }
        Err(crate::platform::selection::SelectionError::Empty(msg)) => {
            eprintln!("{}", msg);
            crate::platform::notifications::try_notify(
                "Copy text first",
                Some("Lepramim: clipboard is empty. Press Ctrl+C first."),
                1.0,
            );
            return 2;
        }
        Err(crate::platform::selection::SelectionError::ToolMissing(msg)) => {
            eprintln!("{}", msg);
            crate::platform::notifications::try_notify(
                "Lepramim: capture tool missing",
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
        .header(hyper::header::HOST, "lepramim")
        .header(hyper::header::CONNECTION, "close")
        .body(())
        .expect("hyper request builds");

    let mut stream = match tokio::net::UnixStream::connect(&sock).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not reach Lepramim ({}): {e}", sock.display());
            return 3;
        }
    };

    let raw_req = b"GET /state HTTP/1.1\r\nHost: lepramim\r\nConnection: close\r\n\r\n";
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
        eprintln!("Lepramim daemon returned {code}: {body}");
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

async fn cmd_download_models() -> i32 {
    let cache = crate::models::default_cache_dir();
    println!("Downloading models to {}", cache.display());
    match crate::models::ensure_artifacts(None, true) {
        Ok(map) => {
            for (name, path) in &map {
                println!("{} verified at {}", name, path.display());
            }
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    }
    0
}

async fn cmd_setup(force: bool) -> i32 {
    let binary = crate::platform::service::resolve_binary_path();
    if let Err(e) = ensure_user_files() {
        eprintln!("{e}");
        return 1;
    }
    if force {
        let cfg_path = crate::config::config_path();
        let default = crate::config::Config::default();
        match toml::to_string(&default) {
            Ok(toml_str) => {
                if let Err(e) = std::fs::write(&cfg_path, toml_str) {
                    eprintln!("Failed to write config: {e}");
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("Failed to serialize config: {e}");
                return 1;
            }
        }
    }
    match crate::platform::service::write_autostart(&binary) {
        Ok(path) => println!("Will start with the desktop session ({})", path.display()),
        Err(e) => eprintln!("Could not write autostart entry: {e}"),
    }
    println!("Lepramim is ready. Double-click the AppImage to use it.");
    0
}

async fn cmd_daemon() -> i32 {
    println!("Starting Lepramim daemon...");
    match crate::daemon::run().await {
        Ok(_) => {
            // Lands in daemon.log: distinguishes a clean stop (quit menu,
            // toggle off) from a crash, which leaves no such line.
            println!("Lepramim daemon stopped.");
            0
        }
        Err(e) => {
            eprintln!("daemon failed: {e}");
            1
        }
    }
}

async fn cmd_uninstall() -> i32 {
    request_daemon_shutdown(None).await;

    match crate::platform::service::remove_autostart() {
        Ok(Some(path)) => println!("Removed {}", path.display()),
        Ok(None) => {}
        Err(e) => eprintln!("Could not remove autostart: {e}"),
    }

    let desktop = crate::platform::service::desktop_file_path();
    if desktop.is_file() {
        if let Err(e) = std::fs::remove_file(&desktop) {
            eprintln!("Failed to remove {}: {e}", desktop.display());
        } else {
            println!("Removed {}", desktop.display());
        }
    }

    let sock = crate::config::socket_path();
    if sock.exists() {
        let _ = std::fs::remove_file(&sock);
    }

    println!("Configuration and downloaded models were kept.");
    0
}

async fn cmd_bug_report(full: bool) -> i32 {
    let redact = !full;
    // Collect similar to native bug_report
    let version = env!("CARGO_PKG_VERSION");
    let mut out = String::new();
    out.push_str("# Lepramim bug report\n\n");
    out.push_str(&format!("- **Lepramim**: {}\n", version));
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
        let cli = Cli::try_parse_from(["lepramim", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Status)));
    }

    #[test]
    fn parse_app_and_tray() {
        let cli = Cli::try_parse_from(["lepramim", "app"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::App)));
        let cli = Cli::try_parse_from(["lepramim", "tray"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Tray)));
    }

    #[test]
    fn parse_download_models() {
        let cli = Cli::try_parse_from(["lepramim", "download-models"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::DownloadModels)));
    }

    #[test]
    fn parse_setup_force() {
        let cli = Cli::try_parse_from(["lepramim", "setup", "--force"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Setup { force: true })));
    }

    #[test]
    fn parse_bug_report_full() {
        let cli = Cli::try_parse_from(["lepramim", "bug-report", "--full"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::BugReport { full: true })
        ));
        let cli = Cli::try_parse_from(["lepramim", "bug-report"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::BugReport { full: false })
        ));
    }

    #[test]
    fn parse_speak_selection_max_bytes() {
        let cli =
            Cli::try_parse_from(["lepramim", "speak-selection", "--max-bytes", "12345"]).unwrap();
        if let Some(Commands::SpeakSelection { max_bytes }) = cli.command {
            assert_eq!(max_bytes, Some(12345));
        } else {
            panic!("wrong command");
        }
    }
}
