use regex::Regex;
use std::sync::OnceLock;

static SENTENCE_RE: OnceLock<Regex> = OnceLock::new();

fn sentence_re() -> &'static Regex {
    SENTENCE_RE.get_or_init(|| {
        // Split on sentence boundaries: . ! ? followed by whitespace and capital or end.
        // Keep delimiters with sentence.
        // Use lookahead via splitting; we will manually segment.
        Regex::new(r"[.!?]+").unwrap()
    })
}

/// Split text into sentences.
/// Simple heuristic: split on [.!?]+ followed by whitespace, keep punctuation.
/// Preserves abbreviations already expanded upstream.
pub fn split_sentences(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    // Use a stateful split: iterate over matches of sentence terminators.
    let re = Regex::new(r"[^.!?]+[.!?]+(?:\s+|$)").unwrap();
    let mut out = Vec::new();
    let mut last_end = 0usize;
    for m in re.find_iter(text) {
        let s = m.as_str().trim();
        if !s.is_empty() {
            out.push(s.to_string());
        }
        last_end = m.end();
    }
    // Tail without terminator
    if last_end < text.len() {
        let tail = text[last_end..].trim();
        if !tail.is_empty() {
            out.push(tail.to_string());
        }
    }
    if out.is_empty() {
        // Fallback: return whole trimmed text as one sentence
        let t = text.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_basic() {
        let s = split_sentences("Hello world. Second sentence! Third?");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0], "Hello world.");
    }
    #[test]
    fn empty() {
        assert!(split_sentences("").is_empty());
        assert!(split_sentences("   ").is_empty());
    }
    #[test]
    fn unicode() {
        let s = split_sentences("Hello α. World β!");
        assert_eq!(s.len(), 2);
    }
}
