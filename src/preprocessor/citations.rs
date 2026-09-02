use regex::Regex;
use std::sync::OnceLock;

static NUMERIC_BRACKET: OnceLock<Regex> = OnceLock::new();
static PAREN_AUTHOR_YEAR: OnceLock<Regex> = OnceLock::new();

fn numeric_bracket() -> &'static Regex {
    // Simplified without lookbehind: match bracket citation anywhere, but caller checks preceding char
    NUMERIC_BRACKET.get_or_init(|| Regex::new(r"\[\s*\d+(?:\s*[–\-,]\s*\d+)*\s*\]").unwrap())
}

fn paren_author_year() -> &'static Regex {
    PAREN_AUTHOR_YEAR.get_or_init(|| {
        let surname = r"[A-ZÀ-Ý][A-Za-zÀ-ÿ\-']+";
        let pat = format!(
            r"\(\s*(?:{surname}(?:\s+et\s+al\.?)?(?:\s*(?:&|and)\s*{surname})*(?:\s*,)?\s*\d{{4}}[a-z]?(?:\s*[;,]\s*{surname}(?:\s+et\s+al\.?)?(?:\s*(?:&|and)\s*{surname})*(?:\s*,)?\s*\d{{4}}[a-z]?)*\s*)\)",
            surname = surname
        );
        Regex::new(&pat).unwrap()
    })
}

fn tidy(text: String) -> String {
    let t = Regex::new(r"\s+([,.;:!?])")
        .unwrap()
        .replace_all(&text, "$1")
        .to_string();
    let t = Regex::new(r" +\n")
        .unwrap()
        .replace_all(&t, "\n")
        .to_string();
    Regex::new(r"[ \t]{2,}")
        .unwrap()
        .replace_all(&t, " ")
        .to_string()
}

pub fn strip_numeric_bracket_citations(text: &str) -> String {
    // Manual scan to preserve native's guard: only strip if '[' is at start or preceded by whitespace/punct
    let re = numeric_bracket();
    let mut result = String::new();
    let mut last = 0usize;
    for m in re.find_iter(text) {
        let start = m.start();
        let preceding_ok = if start == 0 {
            true
        } else {
            let prev = text[..start].chars().last().unwrap_or(' ');
            " \t\n,;:.!?".contains(prev)
        };
        // Append segment before match
        result.push_str(&text[last..start]);
        if preceding_ok {
            // strip the bracket (skip)
        } else {
            result.push_str(m.as_str());
        }
        last = m.end();
    }
    result.push_str(&text[last..]);
    tidy(result)
}

pub fn strip_parenthetical_citations(text: &str) -> String {
    let t = paren_author_year().replace_all(text, "").to_string();
    tidy(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_bracket_basic() {
        let t = strip_numeric_bracket_citations("Smith [12] said");
        assert!(!t.contains("[12]"));
        assert!(t.contains("Smith"));
    }
    #[test]
    fn numeric_bracket_no_false_positive_array() {
        let t = strip_numeric_bracket_citations("arr[3] is value");
        assert!(t.contains("arr[3]"), "got {}", t);
    }
    #[test]
    fn parenthetical_basic() {
        let t = strip_parenthetical_citations("As shown (Smith, 2023) the result");
        assert!(!t.contains("Smith, 2023"));
    }
    #[test]
    fn no_panic_unicode() {
        let t = strip_numeric_bracket_citations("Hello [1, 2–3] world");
        assert!(!t.is_empty());
    }
}
