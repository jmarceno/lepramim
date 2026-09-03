use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

static MISSING_LOGGED: OnceLock<Mutex<bool>> = OnceLock::new();

fn missing_logged() -> &'static Mutex<bool> {
    MISSING_LOGGED.get_or_init(|| Mutex::new(false))
}

fn candidate_ok(p: &Path) -> bool {
    p.is_file() && is_executable(p)
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(p) {
        return meta.permissions().mode() & 0o111 != 0;
    }
    false
}
#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

pub fn sre_executable_path() -> Option<String> {
    // Check venv bin first: sibling of current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let cand = parent.join("sre");
            if candidate_ok(&cand) {
                return Some(cand.to_string_lossy().to_string());
            }
        }
    }
    // Check PATH
    if let Some(path) = find_in_path("sre") {
        if candidate_ok(Path::new(&path)) {
            return Some(path);
        }
    }
    None
}

fn find_in_path(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let cand = Path::new(dir).join(name);
        if cand.is_file() {
            return Some(cand.to_string_lossy().to_string());
        }
    }
    None
}

pub fn is_sre_available() -> bool {
    sre_executable_path().is_some()
}

fn log_missing_once() {
    let mut flag = missing_logged().lock().unwrap();
    if !*flag {
        tracing::info!(
            "SRE (speech-rule-engine) not found on PATH or in the venv bin. LaTeX spans will be passed through unchanged."
        );
        *flag = true;
    }
}

/// Very simple LaTeX span detection: looks for $...$, $$...$$, \(...\), \[...\], \begin{...}...\end{...}
pub fn latex_to_speech(text: &str, timeout_s: f64, domain: &str, style: Option<&str>) -> String {
    // cheap hint
    let hint = text.contains("\\frac")
        || text.contains("\\sum")
        || text.contains("\\int")
        || text.contains("\\sqrt")
        || text.contains("\\alpha")
        || text.contains("$$")
        || (text.contains('$') && text.matches('$').count() >= 2)
        || text.contains("\\(")
        || text.contains("\\[");
    if !hint {
        return text.to_string();
    }
    let sre_path = match sre_executable_path() {
        Some(p) => p,
        None => {
            log_missing_once();
            return text.to_string();
        }
    };

    // Find spans using simplified regex without lookaround (regex crate doesn't support it)
    let re = regex::Regex::new(r"(?s)(\$\$(.+?)\$\$|\\\[(.+?)\\\]|\\\((.+?)\\\)|\$(.+?)\$|\\begin\{(equation\*?|align\*?|gather\*?|multline\*?|eqnarray\*?)\}(.+?)\\end\{(?:equation\*?|align\*?|gather\*?|multline\*?|eqnarray\*?)\})").unwrap();
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    for m in re.find_iter(text) {
        let matched = m.as_str();
        // Extract inner: strip delimiters
        let inner = if matched.starts_with("$$")
            || matched.starts_with("\\[")
            || matched.starts_with("\\(")
        {
            matched[2..matched.len() - 2].to_string()
        } else if matched.starts_with('$') {
            matched[1..matched.len() - 1].to_string()
        } else if matched.starts_with("\\begin") {
            // extract between \begin{...} and \end{...}
            if let Some(start) = matched.find('}') {
                if let Some(end) = matched.rfind("\\end") {
                    matched[start + 1..end].to_string()
                } else {
                    matched.to_string()
                }
            } else {
                matched.to_string()
            }
        } else {
            matched.to_string()
        };
        spans.push((m.start(), m.end(), inner));
    }
    if spans.is_empty() {
        return text.to_string();
    }

    let mut cmd: Vec<String> = vec![
        sre_path,
        "--latex".to_string(),
        "--speech".to_string(),
        "-d".to_string(),
        domain.to_string(),
    ];
    if let Some(s) = style {
        if !s.is_empty() {
            cmd.push("-s".to_string());
            cmd.push(s.to_string());
        }
    }

    let mut replacements: Vec<String> = Vec::new();
    for (_, _, inner) in &spans {
        let mut c = Command::new(&cmd[0]);
        c.args(&cmd[1..]);
        // timeout handling via wait_timeout? Simple: use Command with timeout via wait with thread.
        // We'll spawn and use std::process with timeout using Rust std + thread.
        // Simplified: run with timeout_s using `wait_timeout` pattern via polling.
        // For now, use `Command` with stdin piped.
        use std::io::Write;
        use std::process::Stdio;
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match c.spawn() {
            Ok(ch) => ch,
            Err(e) => {
                tracing::warn!("SRE spawn failed: {}", e);
                return text.to_string();
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(inner.as_bytes());
        }
        // Wait with timeout
        let timeout = std::time::Duration::from_secs_f64(timeout_s);
        let start = std::time::Instant::now();
        let result = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        break None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break None,
            }
        };
        match result {
            Some(status) if status.success() => {
                let output = child
                    .wait_with_output()
                    .unwrap_or_else(|_| std::process::Output {
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    });
                let spoken = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if spoken.is_empty() {
                    tracing::warn!("SRE returned empty speech for a LaTeX span; falling back");
                    return text.to_string();
                }
                replacements.push(spoken);
            }
            Some(status) => {
                tracing::warn!(
                    "SRE returned non-zero (rc={:?}); falling back",
                    status.code()
                );
                return text.to_string();
            }
            None => {
                tracing::warn!("SRE timed out; falling back");
                return text.to_string();
            }
        }
    }

    let mut out = text.to_string();
    for ((start, end, _), spoken) in spans.iter().zip(replacements.iter()).rev() {
        out.replace_range(*start..*end, spoken);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_latex_passthrough() {
        let t = latex_to_speech("Hello world", 1.0, "clearspeak", None);
        assert_eq!(t, "Hello world");
    }
}
