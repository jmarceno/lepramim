use std::process::Command;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("selection is empty")]
    Empty(String),
    #[error("tool missing: {0}")]
    ToolMissing(String),
    #[error("capture timed out after {0}s")]
    Timeout(f64),
    #[error("cannot reach display server: {0}")]
    DisplayUnavailable(String),
    #[error("capture failed: {0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub text: String,
    pub truncated: bool,
    pub original_byte_length: usize,
    pub source: String, // "primary" | "clipboard"
    pub tool: String,
}

const DISPLAY_FAILURE_MARKERS: &[&str] = &[
    "can't open display",
    "cannot open display",
    "unable to open display",
    "no display",
    "display name is missing",
    "authorization",
    "not authorized",
    "wayland_display",
    "no wayland connection",
    "compositor doesn't support",
    "does not seem to support primary selection",
    "could not connect",
    "failed to connect",
];

pub fn utf8_safe_truncate(data: &[u8], max_bytes: usize) -> Vec<u8> {
    if data.len() <= max_bytes {
        return data.to_vec();
    }
    let mut cut = max_bytes;
    while cut > 0 && (data[cut] & 0xC0) == 0x80 {
        cut -= 1;
    }
    data[..cut].to_vec()
}

fn run_capture(cmd: &[String], timeout_s: f64) -> Result<Vec<u8>, SelectionError> {
    let prog = &cmd[0];
    if which(prog).is_none() {
        return Err(SelectionError::ToolMissing(format!(
            "{} is not installed",
            prog
        )));
    }
    let mut command = Command::new(prog);
    command.args(&cmd[1..]);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    // Spawn and wait with timeout
    let mut child = command
        .spawn()
        .map_err(|e| SelectionError::Other(e.to_string()))?;
    let timeout = Duration::from_secs_f64(timeout_s);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|e| SelectionError::Other(e.to_string()))?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let lowered = stderr.to_lowercase();
                    if DISPLAY_FAILURE_MARKERS
                        .iter()
                        .any(|m| lowered.contains(&m.to_lowercase()))
                    {
                        return Err(SelectionError::DisplayUnavailable(format!(
                            "{} cannot reach the display server: {}",
                            prog,
                            stderr.trim()
                        )));
                    }
                    if !stderr.trim().is_empty() {
                        tracing::debug!(
                            "{} exited {} with stderr: {}",
                            prog,
                            status.code().unwrap_or(-1),
                            stderr.trim()
                        );
                    }
                    return Ok(Vec::new());
                }
                return Ok(output.stdout);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(SelectionError::Timeout(timeout_s));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(SelectionError::Other(e.to_string())),
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

fn finalize(
    raw: Vec<u8>,
    max_bytes: usize,
    source: &str,
    tool: &str,
) -> Result<CaptureResult, SelectionError> {
    let original_len = raw.len();
    let truncated = original_len > max_bytes;
    let data = if truncated {
        utf8_safe_truncate(&raw, max_bytes)
    } else {
        raw
    };
    let text = String::from_utf8_lossy(&data).to_string();
    if text.trim().is_empty() {
        return Err(SelectionError::Empty(format!(
            "{} selection is empty",
            source
        )));
    }
    Ok(CaptureResult {
        text,
        truncated,
        original_byte_length: original_len,
        source: source.to_string(),
        tool: tool.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_type: String,
    pub desktop: String,
    pub wl_paste: Option<String>,
    pub xclip: Option<String>,
}

impl SessionInfo {
    pub fn is_wayland(&self) -> bool {
        self.session_type == "wayland"
    }
    pub fn is_x11(&self) -> bool {
        self.session_type == "x11"
    }
}

pub fn detect_session() -> SessionInfo {
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_else(|_| "unknown".to_string())
        .to_lowercase();
    let st = if ["wayland", "x11", "unknown"].contains(&session_type.as_str()) {
        session_type
    } else {
        "unknown".to_string()
    };
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "unknown".to_string());
    SessionInfo {
        session_type: st,
        desktop,
        wl_paste: which("wl-paste"),
        xclip: which("xclip"),
    }
}

fn pick_primary_tool(info: &SessionInfo) -> Result<Vec<String>, SelectionError> {
    if info.is_wayland() {
        if info.wl_paste.is_none() {
            return Err(SelectionError::ToolMissing(
                "wl-paste is not installed. Install wl-clipboard: `sudo apt install wl-clipboard`"
                    .to_string(),
            ));
        }
        return Ok(vec![
            "wl-paste".to_string(),
            "--primary".to_string(),
            "--no-newline".to_string(),
        ]);
    }
    if info.is_x11() {
        if info.xclip.is_none() {
            return Err(SelectionError::ToolMissing(
                "xclip is not installed. Install it: `sudo apt install xclip`".to_string(),
            ));
        }
        return Ok(vec![
            "xclip".to_string(),
            "-o".to_string(),
            "-selection".to_string(),
            "primary".to_string(),
        ]);
    }
    if info.wl_paste.is_some() {
        return Ok(vec![
            "wl-paste".to_string(),
            "--primary".to_string(),
            "--no-newline".to_string(),
        ]);
    }
    if info.xclip.is_some() {
        return Ok(vec![
            "xclip".to_string(),
            "-o".to_string(),
            "-selection".to_string(),
            "primary".to_string(),
        ]);
    }
    Err(SelectionError::ToolMissing(
        "neither wl-paste nor xclip is installed; run `sudo apt install wl-clipboard xclip`"
            .to_string(),
    ))
}

fn pick_clipboard_tool(info: &SessionInfo) -> Result<Vec<String>, SelectionError> {
    if info.is_wayland() {
        if info.wl_paste.is_none() {
            return Err(SelectionError::ToolMissing(
                "wl-paste is not installed. Install wl-clipboard: `sudo apt install wl-clipboard`"
                    .to_string(),
            ));
        }
        return Ok(vec!["wl-paste".to_string(), "--no-newline".to_string()]);
    }
    if info.is_x11() {
        if info.xclip.is_none() {
            return Err(SelectionError::ToolMissing(
                "xclip is not installed. Install it: `sudo apt install xclip`".to_string(),
            ));
        }
        return Ok(vec![
            "xclip".to_string(),
            "-o".to_string(),
            "-selection".to_string(),
            "clipboard".to_string(),
        ]);
    }
    if info.wl_paste.is_some() {
        return Ok(vec!["wl-paste".to_string(), "--no-newline".to_string()]);
    }
    if info.xclip.is_some() {
        return Ok(vec![
            "xclip".to_string(),
            "-o".to_string(),
            "-selection".to_string(),
            "clipboard".to_string(),
        ]);
    }
    Err(SelectionError::ToolMissing(
        "neither wl-paste nor xclip is installed; run `sudo apt install wl-clipboard xclip`"
            .to_string(),
    ))
}

pub fn read_primary(max_bytes: usize, timeout_s: f64) -> Result<CaptureResult, SelectionError> {
    let info = detect_session();
    let cmd = pick_primary_tool(&info)?;
    let raw = run_capture(&cmd, timeout_s)?;
    finalize(raw, max_bytes, "primary", &cmd.join(" "))
}

pub fn read_clipboard(max_bytes: usize, timeout_s: f64) -> Result<CaptureResult, SelectionError> {
    let info = detect_session();
    let cmd = pick_clipboard_tool(&info)?;
    let raw = run_capture(&cmd, timeout_s)?;
    finalize(raw, max_bytes, "clipboard", &cmd.join(" "))
}

pub fn try_force_copy(timeout_s: f64) -> bool {
    // Try ydotool, dotool, wtype, xdotool
    let timeout = Duration::from_secs_f64(timeout_s);
    if let Some(ydotool) = which("ydotool") {
        let mut cmd = Command::new(&ydotool);
        cmd.args(["key", "29:1", "46:1", "46:0", "29:0"]);
        if run_with_timeout(cmd, timeout).is_ok() {
            std::thread::sleep(Duration::from_millis(350));
            return true;
        }
    }
    if let Some(dotool) = which("dotool") {
        let mut cmd = Command::new(&dotool);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        if let Ok(mut child) = cmd.spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(b"key Ctrl_L+c\n");
            }
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => {
                        std::thread::sleep(Duration::from_millis(350));
                        return true;
                    }
                    Ok(Some(_)) => break,
                    Ok(None) if start.elapsed() > timeout => {
                        let _ = child.kill();
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
        }
    }
    if let Some(wtype) = which("wtype") {
        let mut cmd = Command::new(&wtype);
        cmd.args(["-M", "ctrl", "-P", "c", "-m", "ctrl"]);
        if run_with_timeout(cmd, timeout).is_ok() {
            std::thread::sleep(Duration::from_millis(350));
            return true;
        }
    }
    if let Some(xdotool) = which("xdotool") {
        let mut cmd = Command::new(&xdotool);
        cmd.args(["key", "ctrl+c"]);
        if run_with_timeout(cmd, timeout).is_ok() {
            std::thread::sleep(Duration::from_millis(350));
            return true;
        }
    }
    false
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<(), String> {
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                } else {
                    return Err(format!("exit {}", status.code().unwrap_or(-1)));
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

pub fn read_clipboard_via_klipper(
    max_bytes: usize,
    timeout_s: f64,
) -> Result<CaptureResult, SelectionError> {
    let qdbus = which("qdbus6").or_else(|| which("qdbus")).ok_or_else(|| {
        SelectionError::ToolMissing("qdbus is not installed; cannot query Klipper".to_string())
    })?;
    let cmd = vec![
        qdbus.clone(),
        "org.kde.klipper".to_string(),
        "/klipper".to_string(),
        "org.kde.klipper.klipper.getClipboardContents".to_string(),
    ];
    let raw = run_capture(&cmd, timeout_s)?;
    let trimmed: Vec<u8> = raw
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace())
        .collect();
    // Check for image placeholder ▨
    if trimmed.starts_with(&[0xe2, 0x96, 0xa8]) {
        return Err(SelectionError::Empty(
            "clipboard contains an image, not text".to_string(),
        ));
    }
    finalize(raw, max_bytes, "clipboard", "klipper")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_truncate_basic() {
        let data = "hello world".as_bytes();
        let t = utf8_safe_truncate(data, 5);
        assert_eq!(String::from_utf8_lossy(&t), "hello");
    }
    #[test]
    fn utf8_truncate_multibyte() {
        // "héllo" -> h(1) é(2) l(1) l(1) o(1) = 6 bytes; truncate at 3 should not split é
        let s = "héllo";
        let data = s.as_bytes();
        let t = utf8_safe_truncate(data, 3);
        // Should be "h" + maybe? Let's ensure valid utf8
        assert!(String::from_utf8(t).is_ok());
    }
    #[test]
    fn finalize_truncated() {
        let raw = b"hello world".to_vec();
        let res = finalize(raw, 5, "primary", "test").unwrap();
        assert!(res.truncated);
        assert_eq!(res.original_byte_length, 11);
        assert_eq!(res.text, "hello");
    }
    #[test]
    fn finalize_empty_fails() {
        let raw = b"   \n".to_vec();
        let res = finalize(raw, 100, "primary", "test");
        assert!(res.is_err());
    }
}
