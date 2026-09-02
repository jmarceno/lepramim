use std::process::Command;
use std::time::Duration;

pub fn try_notify(summary: &str, body: Option<&str>, timeout_s: f64) {
    let notify = match which("notify-send") {
        Some(p) => p,
        None => {
            tracing::debug!("notify-send not available; falling back to stderr");
            return;
        }
    };
    let mut args = vec![
        notify,
        "--app-name".to_string(),
        "Lexaloud".to_string(),
        "--expire-time".to_string(),
        "3000".to_string(),
        "--".to_string(),
        summary.to_string(),
    ];
    if let Some(b) = body {
        args.push(b.to_string());
    }
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let timeout = Duration::from_secs_f64(timeout_s);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("notify-send failed: {}", e);
            return;
        }
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                tracing::debug!("notify-send wait failed: {}", e);
                break;
            }
        }
    }
}

fn which(name: &str) -> Option<String> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        let p = std::path::Path::new(dir).join(name);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}
