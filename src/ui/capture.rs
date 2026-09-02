//! Selection capture for tray, hotkeys, and CLI (Qt algorithm port).

use std::process::Command;
use std::time::Duration;

pub use crate::platform::selection::{CaptureResult, SelectionError, utf8_safe_truncate};

const MAX_BYTES: usize = 200 * 1024;
const TOOL_TIMEOUT_MS: u64 = 400;
const INJECTOR_TIMEOUT_MS: u64 = 250;
const CLIPBOARD_SETTLE_MS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionCapture {
    pub text: String,
    pub source: String,
    pub truncated: bool,
}

/// Pick between selection and clipboard snapshots (Wayland path).
pub fn resolve_capture(
    selection: &str,
    clipboard_before: &str,
    clipboard_after: &str,
) -> SelectionCapture {
    let sel = selection.trim();
    let before = clipboard_before.trim();
    let after = clipboard_after.trim();
    if !sel.is_empty() {
        return SelectionCapture {
            text: sel.to_string(),
            source: "primary/qt".into(),
            truncated: false,
        };
    }
    if !after.is_empty() && after != before {
        return SelectionCapture {
            text: after.to_string(),
            source: "clipboard/updated".into(),
            truncated: false,
        };
    }
    if !after.is_empty() {
        return SelectionCapture {
            text: after.to_string(),
            source: "clipboard".into(),
            truncated: false,
        };
    }
    if !before.is_empty() {
        return SelectionCapture {
            text: before.to_string(),
            source: "clipboard".into(),
            truncated: false,
        };
    }
    SelectionCapture {
        text: String::new(),
        source: String::new(),
        truncated: false,
    }
}

pub fn is_wayland() -> bool {
    let session = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    session == "wayland" || std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn find_tool(name: &str) -> Option<String> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        let p = std::path::Path::new(dir).join(name);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(name);
            if sibling.is_file() {
                return Some(sibling.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn command_with_wayland(program: &str) -> Command {
    let mut command = Command::new(program);
    if let Ok(display) = std::env::var("LEXALOUD_WAYLAND_DISPLAY") {
        command.env("WAYLAND_DISPLAY", display);
    }
    command
}

fn run_capture(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    if program.is_empty() {
        return None;
    }
    let mut proc = command_with_wayland(program);
    proc.args(args);
    proc.stdout(std::process::Stdio::piped());
    proc.stderr(std::process::Stdio::null());
    let mut child = proc.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return child.wait_with_output().ok().map(|o| o.stdout);
            }
            Ok(Some(_)) => return None,
            Ok(None) if start.elapsed().as_millis() > TOOL_TIMEOUT_MS as u128 => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return None,
        }
    }
}

fn decode_text(raw: &[u8]) -> (String, bool) {
    let mut truncated = false;
    let data = if raw.len() > MAX_BYTES {
        truncated = true;
        utf8_safe_truncate(raw, MAX_BYTES)
    } else {
        raw.to_vec()
    };
    (String::from_utf8_lossy(&data).trim().to_string(), truncated)
}

fn read_primary_x11() -> Option<SelectionCapture> {
    let xclip = find_tool("xclip")?;
    let raw = run_capture(&xclip, &["-o", "-selection", "primary"])?;
    let (text, truncated) = decode_text(&raw);
    if text.is_empty() {
        return None;
    }
    Some(SelectionCapture {
        text,
        source: "primary/xclip".into(),
        truncated,
    })
}

fn read_clipboard_tool() -> Option<SelectionCapture> {
    let raw = if is_wayland() {
        let wl = find_tool("wl-paste")?;
        run_capture(&wl, &["--no-newline"])?
    } else {
        let xclip = find_tool("xclip")?;
        run_capture(&xclip, &["-o", "-selection", "clipboard"])?
    };
    let source = if is_wayland() {
        "clipboard/wl-paste"
    } else {
        "clipboard/xclip"
    };
    let (text, truncated) = decode_text(&raw);
    if text.is_empty() {
        return None;
    }
    Some(SelectionCapture {
        text,
        source: source.into(),
        truncated,
    })
}

fn read_klipper() -> Option<SelectionCapture> {
    let output = command_with_wayland("qdbus6")
        .args([
            "org.kde.klipper",
            "/klipper",
            "org.kde.klipper.klipper.getClipboardContents",
        ])
        .output()
        .ok()
        .or_else(|| {
            command_with_wayland("qdbus")
                .args([
                    "org.kde.klipper",
                    "/klipper",
                    "org.kde.klipper.klipper.getClipboardContents",
                ])
                .output()
                .ok()
        })?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.starts_with('\u{25a8}') || text.is_empty() {
        return None;
    }
    Some(SelectionCapture {
        text,
        source: "clipboard/klipper".into(),
        truncated: false,
    })
}

fn current_clipboard_text() -> String {
    if let Some(cap) = read_clipboard_tool() {
        return cap.text;
    }
    read_klipper().map(|c| c.text).unwrap_or_default()
}

pub fn try_force_copy() -> bool {
    let run = |program: &str, args: &[&str], stdin: Option<&[u8]>| -> bool {
        if program.is_empty() {
            return false;
        }
        let mut proc = command_with_wayland(program);
        proc.args(args);
        if stdin.is_some() {
            proc.stdin(std::process::Stdio::piped());
        }
        proc.stdout(std::process::Stdio::null());
        proc.stderr(std::process::Stdio::null());
        let mut child = match proc.spawn() {
            Ok(c) => c,
            Err(_) => return false,
        };
        if let Some(data) = stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(data);
            }
        }
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if start.elapsed().as_millis() > INJECTOR_TIMEOUT_MS as u128 => {
                    let _ = child.kill();
                    return false;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => return false,
            }
        }
    };

    let injected = run(
        &find_tool("ydotool").unwrap_or_default(),
        &["key", "29:1", "46:1", "46:0", "29:0"],
        None,
    ) || run(
        &find_tool("wtype").unwrap_or_default(),
        &["-M", "ctrl", "-P", "c", "-m", "ctrl"],
        None,
    ) || run(
        &find_tool("xdotool").unwrap_or_default(),
        &["key", "ctrl+c"],
        None,
    ) || run(
        &find_tool("dotool").unwrap_or_default(),
        &[],
        Some(b"key Ctrl_L+c\n"),
    );

    if injected {
        std::thread::sleep(Duration::from_millis(80));
    }
    injected
}

/// Capture highlighted text using the Qt UI algorithm.
pub fn capture_highlighted_text() -> SelectionCapture {
    if !is_wayland() {
        if let Some(primary) = read_primary_x11() {
            return primary;
        }
    }

    let before = current_clipboard_text();
    let injected = try_force_copy();
    let mut after = current_clipboard_text();
    if injected {
        let deadline = std::time::Instant::now() + Duration::from_millis(CLIPBOARD_SETTLE_MS);
        while after.is_empty() || after == before {
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
            after = current_clipboard_text();
        }
    }
    resolve_capture("", &before, &after)
}

pub fn speak_captured_selection() {
    let cap = capture_highlighted_text();
    if cap.text.is_empty() {
        crate::platform::notifications::try_notify(
            "Select text first",
            Some("Lexaloud could not capture a selection. Copy the text (Ctrl+C) and try again."),
            3.0,
        );
        return;
    }
    if cap.truncated {
        crate::platform::notifications::try_notify(
            "Selection truncated",
            Some("Lexaloud captured the first part of a larger selection."),
            3.0,
        );
    }
    let r = crate::ui::client::post_speak(&cap.text, "replace");
    if r.is_daemon_down() {
        crate::platform::notifications::try_notify(
            "Lexaloud",
            Some("Speech daemon is not running."),
            3.0,
        );
    } else if !r.is_success() {
        crate::platform::notifications::try_notify(
            "Lexaloud",
            Some("Could not start speech. Is the control window working?"),
            3.0,
        );
    }
}

pub fn toggle_playback() {
    let _ = crate::ui::client::post_toggle();
}

/// CLI-facing capture using the same algorithm as the UI.
pub fn capture_for_cli(max_bytes: usize, timeout_s: f64) -> Result<CaptureResult, SelectionError> {
    let _ = timeout_s;
    let cap = capture_highlighted_text();
    if cap.text.is_empty() {
        return Err(SelectionError::Empty("no selection found".into()));
    }
    let truncated = cap.text.len() > max_bytes;
    let text = if truncated {
        String::from_utf8_lossy(&utf8_safe_truncate(cap.text.as_bytes(), max_bytes)).to_string()
    } else {
        cap.text.clone()
    };
    Ok(CaptureResult {
        text,
        truncated,
        original_byte_length: cap.text.len(),
        source: cap.source.clone(),
        tool: cap.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_selection_over_clipboard() {
        let cap = resolve_capture(" highlighted ", "old", "new");
        assert_eq!(cap.text, "highlighted");
        assert_eq!(cap.source, "primary/qt");
    }

    #[test]
    fn uses_updated_clipboard_when_selection_empty() {
        let cap = resolve_capture("", "old clip", "fresh copy");
        assert_eq!(cap.text, "fresh copy");
        assert_eq!(cap.source, "clipboard/updated");
    }

    #[test]
    fn falls_back_to_unchanged_clipboard() {
        let cap = resolve_capture("", "already copied", "already copied");
        assert_eq!(cap.text, "already copied");
        assert_eq!(cap.source, "clipboard");
    }

    #[test]
    fn empty_when_nothing_present() {
        let cap = resolve_capture("  ", "", "");
        assert!(cap.text.is_empty());
    }

    #[test]
    fn trims_whitespace() {
        let cap = resolve_capture("", " before\n", " after \t");
        assert_eq!(cap.text, "after");
        assert_eq!(cap.source, "clipboard/updated");
    }
}
