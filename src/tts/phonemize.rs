use std::process::{Command, Stdio};
use std::sync::Mutex;

static ESPEAK_LOCK: Mutex<()> = Mutex::new(());

/// Map Lexaloud lang codes to espeak-ng voice names.
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

fn which_espeak() -> Option<String> {
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

/// Phonemize text to IPA using espeak-ng (matches kokoro-onnx phonemizer path).
pub fn phonemize(text: &str, lang: &str) -> Result<String, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("empty text".to_string());
    }
    let exe = which_espeak().ok_or_else(|| {
        "espeak-ng not found; install espeak-ng or set PHONEMIZER_ESPEAK_LIBRARY".to_string()
    })?;
    let voice = espeak_voice(lang);
    let _guard = ESPEAK_LOCK.lock().map_err(|e| e.to_string())?;
    let output = Command::new(&exe)
        .arg("-v")
        .arg(voice)
        .arg("--ipa")
        .arg("-q")
        .arg("--stdout")
        .arg(t)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("espeak-ng failed to start: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("espeak-ng exited {}: {err}", output.status));
    }
    let ipa = String::from_utf8_lossy(&output.stdout).to_string();
    let ipa = ipa.trim().to_string();
    if ipa.is_empty() {
        return Err("espeak-ng returned empty phoneme string".to_string());
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
    }
}
