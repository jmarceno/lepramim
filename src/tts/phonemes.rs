use std::collections::HashMap;
use std::sync::OnceLock;

pub const STYLE_DIM: usize = 256;
pub const MAX_PHONEME_TOKENS: usize = 510;
pub const SAMPLE_RATE: u32 = 24_000;

static VOCAB: OnceLock<HashMap<char, i64>> = OnceLock::new();

pub fn kokoro_vocab() -> &'static HashMap<char, i64> {
    VOCAB.get_or_init(|| {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("kokoro_vocab.json")).expect("kokoro_vocab.json");
        let obj = raw
            .get("vocab")
            .and_then(|v| v.as_object())
            .expect("vocab object");
        let mut map = HashMap::new();
        for (k, v) in obj {
            if let Some(ch) = k.chars().next() {
                if k.chars().count() == 1 {
                    if let Some(id) = v.as_i64() {
                        map.insert(ch, id);
                    }
                }
            }
        }
        map
    })
}

/// Normalize IPA string for Kokoro (collapse whitespace).
pub fn normalize_ipa(phonemes: &str) -> String {
    phonemes.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Tokenize phonemes with BOS/EOS id 0 padding like kokoro-onnx.
pub fn tokenize(phonemes: &str, vocab: &HashMap<char, i64>) -> Vec<i64> {
    let mut tokens = vec![0_i64];
    for ch in phonemes.chars() {
        if let Some(&id) = vocab.get(&ch) {
            tokens.push(id);
        }
    }
    tokens.push(0);
    tokens
}

/// Split phoneme string so token count (excluding BOS/EOS) <= MAX_PHONEME_TOKENS.
pub fn split_at_token_cap<'a>(phonemes: &'a str, vocab: &HashMap<char, i64>) -> (&'a str, &'a str) {
    let mut count = 0usize;
    let mut cap_byte = None;
    for (byte_idx, c) in phonemes.char_indices() {
        if vocab.contains_key(&c) {
            if count == MAX_PHONEME_TOKENS {
                cap_byte = Some(byte_idx);
                break;
            }
            count += 1;
        }
    }
    let Some(cap_byte) = cap_byte else {
        return (phonemes, "");
    };
    let mut split = None;
    for (byte_idx, c) in phonemes[..cap_byte].char_indices() {
        if c.is_whitespace() || matches!(c, ',' | '.' | '!' | '?' | ';' | ':' | '—' | '…') {
            split = Some(byte_idx + c.len_utf8());
        }
    }
    match split {
        Some(b) if !phonemes[..b].trim().is_empty() => (&phonemes[..b], &phonemes[b..]),
        _ => (&phonemes[..cap_byte], &phonemes[cap_byte..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_includes_pads() {
        let v = kokoro_vocab();
        let t = tokenize("həloʊ", v);
        assert_eq!(t.first(), Some(&0));
        assert_eq!(t.last(), Some(&0));
        assert!(t.len() > 2);
    }
}
