use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, OnceLock};

struct PersistentEspeak {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    voice: String,
    exe: String,
}

impl Drop for PersistentEspeak {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static ESPEAK: Mutex<Option<PersistentEspeak>> = Mutex::new(None);
static ESPEAK_EXE: OnceLock<Option<String>> = OnceLock::new();

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

fn spawn_persistent(exe: &str, voice: &str) -> Result<PersistentEspeak, String> {
    let mut child = Command::new(exe)
        .arg("-v")
        .arg(voice)
        .arg("--ipa")
        .arg("-q")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("espeak-ng failed to start: {e}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "espeak-ng stdin missing".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "espeak-ng stdout missing".to_string())?;
    Ok(PersistentEspeak {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        voice: voice.to_string(),
        exe: exe.to_string(),
    })
}

fn phonemize_oneshot(exe: &str, voice: &str, text: &str) -> Result<String, String> {
    let output = Command::new(exe)
        .arg("-v")
        .arg(voice)
        .arg("--ipa")
        .arg("-q")
        .arg("--stdout")
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

fn phonemize_line(proc: &mut PersistentEspeak, text: &str) -> Result<String, String> {
    writeln!(proc.stdin, "{text}").map_err(|e| format!("espeak-ng stdin write: {e}"))?;
    proc.stdin
        .flush()
        .map_err(|e| format!("espeak-ng stdin flush: {e}"))?;
    let mut line = String::new();
    let n = proc
        .stdout
        .read_line(&mut line)
        .map_err(|e| format!("espeak-ng stdout read: {e}"))?;
    if n == 0 {
        return Err("espeak-ng stdout closed".to_string());
    }
    let ipa = line.trim().to_string();
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
    let exe = which_espeak().ok_or_else(|| {
        "espeak-ng not found; install espeak-ng or set PHONEMIZER_ESPEAK_LIBRARY".to_string()
    })?;
    let voice = espeak_voice(lang);
    let mut slot = ESPEAK.lock().map_err(|e| e.to_string())?;
    let reuse = slot
        .as_ref()
        .is_some_and(|p| p.voice == voice && p.exe == exe);
    if !reuse {
        *slot = None;
        *slot = Some(spawn_persistent(&exe, voice)?);
    }
    match phonemize_line(slot.as_mut().expect("espeak spawned"), t) {
        Ok(ipa) => Ok(ipa),
        Err(e) => {
            tracing::warn!("persistent espeak-ng failed ({e}); falling back to one-shot");
            *slot = None;
            phonemize_oneshot(&exe, voice, t)
        }
    }
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
}
