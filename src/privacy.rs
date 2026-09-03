use regex::Regex;
use std::sync::OnceLock;

/// Replace the current user's home directory prefix with `~`.
///
/// If home cannot be determined, returns text unchanged.
pub fn redact_home(text: &str) -> String {
    if let Some(home) = home_dir_string() {
        if home.is_empty() {
            return text.to_string();
        }
        text.replace(&home, "~")
    } else {
        text.to_string()
    }
}

/// Redact TOML values for keys that look like secrets.
///
/// Keys matching `(?i)(key|token|secret|pass)` have their value replaced
/// with `"<REDACTED>"`.
pub fn redact_toml_values(toml_text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"(?i)(key|token|secret|pass)").expect("redact regex compiles"));
    // Matches `key = value` lines.
    let line_re = Regex::new(r"^(\s*)([A-Za-z0-9_.\-]+)(\s*=\s*)(.+)$").unwrap();
    let mut out = Vec::new();
    for raw in toml_text.lines() {
        let mut line = raw.to_string();
        if let Some(caps) = line_re.captures(raw) {
            let key = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if re.is_match(key) {
                let g1 = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let g2 = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let g3 = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                line = format!(r#"{g1}{g2}{g3}"<REDACTED>""#);
            }
        }
        out.push(line);
    }
    out.join("\n")
}

fn home_dir_string() -> Option<String> {
    // Prefer $HOME, fallback to directories crate.
    if let Ok(h) = std::env::var("HOME") {
        if !h.is_empty() {
            return Some(h);
        }
    }
    directories::BaseDirs::new().map(|d| d.home_dir().to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_home_replaces() {
        let home = home_dir_string().unwrap_or_else(|| "/home/test".to_string());
        let text = format!("path {home}/docs/file.txt");
        let redacted = redact_home(&text);
        assert!(redacted.contains("~/docs/file.txt"));
        assert!(!redacted.contains(&home));
    }

    #[test]
    fn redact_toml_secret() {
        let input = "api_key = \"secret123\"\nnormal = \"keep\"\nmy_token = 'abc'";
        let out = redact_toml_values(input);
        assert!(out.contains("api_key = \"<REDACTED>\""));
        assert!(out.contains("normal = \"keep\""));
        assert!(out.contains("my_token = \"<REDACTED>\""));
    }
}
