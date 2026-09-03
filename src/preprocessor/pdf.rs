use regex::Regex;
use std::sync::OnceLock;

static SOFTHYPHEN_LINEBREAK: OnceLock<Regex> = OnceLock::new();
static SOFTHYPHEN_STANDALONE: OnceLock<Regex> = OnceLock::new();
static COMPOUND_LINEBREAK: OnceLock<Regex> = OnceLock::new();
static PLAIN_HYPHEN_LINEBREAK: OnceLock<Regex> = OnceLock::new();

fn soft_hyphen_linebreak() -> &'static Regex {
    SOFTHYPHEN_LINEBREAK.get_or_init(|| Regex::new("\u{00ad}\n[ \t]*").unwrap())
}
fn soft_hyphen_standalone() -> &'static Regex {
    SOFTHYPHEN_STANDALONE.get_or_init(|| Regex::new("\u{00ad}").unwrap())
}
fn compound_linebreak() -> &'static Regex {
    // Simplified without lookbehind: match a-foo-\nbar style; keep hyphen
    // This is a no-op in Rust stub; actual compound handling is approximate
    COMPOUND_LINEBREAK.get_or_init(|| Regex::new(r"([a-z])-([a-z]+)-\n[ \t]*([a-z]+)").unwrap())
}
fn plain_hyphen_linebreak() -> &'static Regex {
    PLAIN_HYPHEN_LINEBREAK.get_or_init(|| Regex::new(r"([a-z]{2,})-\n[ \t]*([a-z]{2,})").unwrap())
}

fn normalize_punctuation(mut text: String) -> String {
    // NFKC-ish: handle curly quotes / spaces manually
    let map = [
        ('\u{2018}', '\''),
        ('\u{2019}', '\''),
        ('\u{201c}', '"'),
        ('\u{201d}', '"'),
        ('\u{2032}', '\''),
        ('\u{2033}', '"'),
        ('\u{00a0}', ' '),
        ('\u{2009}', ' '),
        ('\u{200a}', ' '),
        ('\u{202f}', ' '),
        ('\u{2010}', '-'),
        ('\u{2011}', '-'),
        ('\u{2012}', '-'),
    ];
    for (src, dst) in map {
        text = text.replace(src, &dst.to_string());
    }
    text = soft_hyphen_linebreak().replace_all(&text, "").to_string();
    text = soft_hyphen_standalone().replace_all(&text, "").to_string();
    text
}

fn dehyphenate(text: String) -> String {
    // Use plain hyphen pattern; compound pattern simplified to keep hyphen via capturing groups
    let t = compound_linebreak()
        .replace_all(&text, "$1-$2-$3")
        .to_string();
    plain_hyphen_linebreak().replace_all(&t, "$1$2").to_string()
}

fn unwrap_lines(text: String) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let paragraphs: Vec<&str> = Regex::new(r"\n[ \t]*\n+").unwrap().split(&text).collect();
    let mut out = Vec::new();
    for para in paragraphs {
        let lines: Vec<String> = para
            .split('\n')
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        let mut pieces: Vec<String> = vec![lines[0].clone()];
        for (prev, cur) in lines.iter().zip(lines.iter().skip(1)) {
            let ends_with_punct = prev
                .chars()
                .last()
                .map(|c| ".!?…".contains(c))
                .unwrap_or(false);
            if ends_with_punct {
                pieces.push("\n".to_string());
            } else {
                pieces.push(" ".to_string());
            }
            pieces.push(cur.clone());
        }
        out.push(pieces.concat());
    }
    out.join("\n\n")
}

fn collapse_whitespace(text: String) -> String {
    let t = Regex::new(r"[ \t]+")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();
    let t = Regex::new(r" +\n")
        .unwrap()
        .replace_all(&t, "\n")
        .to_string();
    let t = Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(&t, "\n\n")
        .to_string();
    t.trim().to_string()
}

/// Run the PDF-paste cleanup stages in order.
pub fn clean_pdf_paste(text: &str) -> String {
    let mut t = text.to_string();
    t = normalize_punctuation(t);
    t = dehyphenate(t);
    t = unwrap_lines(t);
    t = collapse_whitespace(t);
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dehyphenate_basic() {
        let t = clean_pdf_paste("com-\npleted");
        assert!(t.contains("completed"), "got {}", t);
    }
    #[test]
    fn strips_soft_hyphen_keeps_dashes() {
        let t = clean_pdf_paste("Hello — world… \u{00ad} test");
        assert_eq!(t, "Hello — world… test");
    }
    #[test]
    fn idempotent_collapse() {
        let a = clean_pdf_paste("Hello   world");
        let b = clean_pdf_paste(&a);
        assert_eq!(a, b);
    }
}
