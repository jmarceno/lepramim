/// Phonemize stub: would call espeak-ng in real implementation.
/// For now, simple mapping that logs and returns a pseudo-phoneme string for tests.
pub fn phonemize(text: &str, lang: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }
    // Check if espeak-ng is available
    let espeak = which_espeak();
    if let Some(path) = espeak {
        tracing::debug!("phonemize using {} for lang {}", path, lang);
        // In real impl, we'd spawn espeak-ng --pho --ipa etc.
        // For stub, just return lowercased text with phoneme markers
        return Ok(text.to_lowercase());
    }
    // Fallback: simple ascii mapping
    tracing::warn!(
        "espeak-ng not found; using stub phonemizer for lang {}",
        lang
    );
    Ok(text
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect())
}

fn which_espeak() -> Option<String> {
    for dir in std::env::var("PATH").unwrap_or_default().split(':') {
        let p = std::path::Path::new(dir).join("espeak-ng");
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
        let p2 = std::path::Path::new(dir).join("espeak");
        if p2.is_file() {
            return Some(p2.to_string_lossy().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phonemize_basic() {
        let p = phonemize("Hello world", "en-us").unwrap();
        assert!(!p.is_empty());
    }
    #[test]
    fn phonemize_empty_fails() {
        assert!(phonemize("", "en-us").is_err());
    }
}
