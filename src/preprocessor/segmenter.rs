use regex::Regex;
use std::sync::OnceLock;

static PROTECTED_ABBREV: OnceLock<Regex> = OnceLock::new();

fn protected_abbrev() -> &'static Regex {
    PROTECTED_ABBREV.get_or_init(|| {
        // Common English abbreviations that must not end a sentence at their trailing period.
        Regex::new(
            r"(?i)\b(?:Dr|Mr|Mrs|Ms|Prof|Sr|Jr|St|vs|etc|Fig|Eq|Eqn|Sec|Ref|Tab|Vol|Ch|Chap|Def|Thm|Lem|Cor|Prop|Ex|Rem|No|pp|p)\.(?:\s|$|\d|\[)",
        )
        .unwrap()
    })
}

/// Split text into sentences with abbreviation-aware boundaries.
pub fn split_sentences(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Protect ellipses from period-based splitting.
    let mut working = trimmed.to_string();
    let mut ellipses: Vec<String> = Vec::new();
    let ellipsis_re = Regex::new(r"\.\.\.").unwrap();
    working = ellipsis_re
        .replace_all(&working, |caps: &regex::Captures| {
            let token = format!("\u{E002}{}\u{E003}", ellipses.len());
            ellipses.push(caps[0].to_string());
            token
        })
        .to_string();

    // Protect abbreviation periods from acting as sentence terminators.
    let mut protected = String::with_capacity(working.len());
    let mut placeholders: Vec<(String, char)> = Vec::new();
    let mut last = 0usize;
    for m in protected_abbrev().find_iter(&working) {
        protected.push_str(&working[last..m.start()]);
        let matched = m.as_str();
        let mut out = matched.to_string();
        if let Some(idx) = out.rfind('.') {
            let token = format!("\u{E000}{}\u{E001}", placeholders.len());
            placeholders.push((token.clone(), '.'));
            out.replace_range(idx..idx + 1, &token);
        }
        protected.push_str(&out);
        last = m.end();
    }
    protected.push_str(&working[last..]);

    let re = Regex::new(r#"[^.!?]+[.!?]+(?:"?\s+|$)"#).unwrap();
    let mut sentences = Vec::new();
    let mut last_end = 0usize;
    for m in re.find_iter(&protected) {
        let mut s = m.as_str().trim().to_string();
        for (token, ch) in &placeholders {
            s = s.replace(token, &ch.to_string());
        }
        for (i, ell) in ellipses.iter().enumerate() {
            let token = format!("\u{E002}{}\u{E003}", i);
            s = s.replace(&token, ell);
        }
        if !s.is_empty() {
            sentences.push(s);
        }
        last_end = m.end();
    }
    if last_end < protected.len() {
        let mut tail = protected[last_end..].trim().to_string();
        for (token, ch) in &placeholders {
            tail = tail.replace(token, &ch.to_string());
        }
        for (i, ell) in ellipses.iter().enumerate() {
            let token = format!("\u{E002}{}\u{E003}", i);
            tail = tail.replace(&token, ell);
        }
        if !tail.is_empty() {
            sentences.push(tail);
        }
    }
    if sentences.is_empty() {
        sentences.push(trimmed.to_string());
    }
    sentences
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
    #[test]
    fn protects_dr_mr() {
        let s = split_sentences("Dr. Smith went to Washington. He met Mr. Jones.");
        assert_eq!(s.len(), 2);
        assert!(s[0].starts_with("Dr. Smith"));
        assert!(s[1].starts_with("He met Mr. Jones"));
    }
    #[test]
    fn protects_fig_reference() {
        let s = split_sentences("See Fig. 3 for details. The results are clear.");
        assert_eq!(s.len(), 2);
        assert!(s[0].contains("Fig. 3"));
    }
    #[test]
    fn ellipsis_in_quotes() {
        let s = split_sentences(r#"He said "Hello... is anyone there?" She replied: Yes! Indeed."#);
        assert_eq!(s.len(), 3);
        assert!(s[0].contains("Hello..."));
    }
}
