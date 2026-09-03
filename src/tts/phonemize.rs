use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

static ESPEAK_EXE: OnceLock<Option<String>> = OnceLock::new();
static PHONEME_CACHE: Mutex<Option<HashMap<(String, String), String>>> = Mutex::new(None);
const MAX_CACHE_ENTRIES: usize = 1024;

/// Map Lepramim lang codes to espeak-ng voice names.
fn espeak_voice(lang: &str) -> &str {
    match lang {
        "en-gb" => "en-gb",
        "es" => "es",
        "fr-fr" | "fr" => "fr",
        "hi" => "hi",
        "it" => "it",
        "ja" => "ja",
        "pt-br" => "pt-br",
        "zh" => "cmn",
        _ => "en-us",
    }
}

fn find_espeak() -> Option<String> {
    if let Ok(lib) = std::env::var("PHONEMIZER_ESPEAK_LIBRARY") {
        if std::path::Path::new(&lib).is_file() {
            return Some(lib);
        }
    }
    for dir in std::env::var("PATH").unwrap_or_default().split(':') {
        for name in ["espeak-ng", "espeak"] {
            let p = std::path::Path::new(dir).join(name);
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn which_espeak() -> Option<String> {
    ESPEAK_EXE.get_or_init(find_espeak).clone()
}

fn phonemize_oneshot(exe: &str, voice: &str, text: &str) -> Result<String, String> {
    let output = Command::new(exe)
        .arg("-v")
        .arg(voice)
        .arg("--ipa")
        .arg("-q")
        .arg(text)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("espeak-ng failed to start: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("espeak-ng exited {}: {err}", output.status));
    }
    let ipa = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if ipa.is_empty() {
        return Err("espeak-ng returned empty phoneme string".to_string());
    }
    Ok(ipa)
}

/// Phonemize text to IPA using espeak-ng (matches kokoro-onnx phonemizer path).
pub fn phonemize(text: &str, lang: &str) -> Result<String, String> {
    let t = text.trim().replace(['\n', '\r'], " ");
    let t = t.trim();
    if t.is_empty() {
        return Err("empty text".to_string());
    }

    let cache_key = (t.to_string(), lang.to_string());
    if let Ok(guard) = PHONEME_CACHE.lock() {
        if let Some(ref cache) = *guard {
            if let Some(hit) = cache.get(&cache_key) {
                return Ok(hit.clone());
            }
        }
    }

    let exe = which_espeak().ok_or_else(|| {
        "espeak-ng not found; install espeak-ng or set PHONEMIZER_ESPEAK_LIBRARY".to_string()
    })?;
    let voice = espeak_voice(lang);
    let ipa = phonemize_oneshot(&exe, voice, t)?;

    if let Ok(mut guard) = PHONEME_CACHE.lock() {
        let cache = guard.get_or_insert_with(HashMap::new);
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(cache_key, ipa.clone());
    }

    Ok(ipa)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonemize_empty_fails() {
        assert!(phonemize("", "en-us").is_err());
    }

    #[test]
    fn phonemize_basic_when_espeak_available() {
        if which_espeak().is_none() {
            return;
        }
        let p = phonemize("Hello", "en-us").unwrap();
        assert!(!p.is_empty());
        let p2 = phonemize("Hello", "en-us").unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn phonemize_multi_clause_does_not_desync() {
        if which_espeak().is_none() {
            return;
        }
        let p1 = phonemize("Hello, world! What a wonderful day.", "en-us").unwrap();
        let p2 = phonemize("Second sentence.", "en-us").unwrap();
        assert!(!p1.is_empty());
        assert!(!p2.is_empty());
        assert_ne!(p1, p2);
    }
}
