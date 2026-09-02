use regex::Regex;
use std::sync::OnceLock;

/// Return `<sha1[:8]> (<N>ch)` as a privacy-safe sentence identifier.
///
/// Mirrors legacy sentence_token behavior for log redaction.
pub fn sentence_token(sentence: &str) -> String {
    let digest = sha1_hex(sentence.as_bytes());
    let short = &digest[..8];
    format!("{short} ({}ch)", sentence.chars().count())
}

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

/// Generic redaction for log lines: home dir + secret keys.
pub fn redact_text(text: &str) -> String {
    let t = redact_home(text);
    // If text looks like TOML, also redact secret keys. Heuristic: if it contains '='.
    if t.contains('=') {
        redact_toml_values(&t)
    } else {
        // Still redact any token-like substrings? Keep simple: only home for now.
        t
    }
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

// ---------------------------------------------------------------------------
// Minimal SHA-1 implementation (no external crate) to match reference SHA-1
// ---------------------------------------------------------------------------

fn sha1_hex(data: &[u8]) -> String {
    let digest = sha1(data);
    let mut s = String::with_capacity(40);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[allow(clippy::needless_range_loop)]
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let ml = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&ml.to_be_bytes());

    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_token_known() {
        // reference: sha1("hello") hex prefix == "aaf4c61d"
        assert_eq!(sentence_token("hello"), "aaf4c61d (5ch)");
    }

    #[test]
    fn sentence_token_empty() {
        // sha1("") == da39a3ee5e6b4b0d3255bfef95601890afd80709
        assert_eq!(sentence_token(""), "da39a3ee (0ch)");
    }

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
