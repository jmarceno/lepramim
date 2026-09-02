use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

static MATH_SYMBOLS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static SYMBOL_PATTERN: OnceLock<Regex> = OnceLock::new();
static MARKDOWN_LINK: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();
static TRAILING_PUNCT: OnceLock<Regex> = OnceLock::new();
static EMAIL_RE: OnceLock<Regex> = OnceLock::new();

fn math_symbols() -> &'static HashMap<&'static str, &'static str> {
    MATH_SYMBOLS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("α", "alpha");
        m.insert("β", "beta");
        m.insert("γ", "gamma");
        m.insert("δ", "delta");
        m.insert("ε", "epsilon");
        m.insert("ζ", "zeta");
        m.insert("η", "eta");
        m.insert("θ", "theta");
        m.insert("ι", "iota");
        m.insert("κ", "kappa");
        m.insert("λ", "lambda");
        m.insert("μ", "mu");
        m.insert("ν", "nu");
        m.insert("ξ", "xi");
        m.insert("ο", "omicron");
        m.insert("π", "pi");
        m.insert("ρ", "rho");
        m.insert("σ", "sigma");
        m.insert("τ", "tau");
        m.insert("υ", "upsilon");
        m.insert("φ", "phi");
        m.insert("χ", "chi");
        m.insert("ψ", "psi");
        m.insert("ω", "omega");
        m.insert("Γ", "Gamma");
        m.insert("Δ", "Delta");
        m.insert("Θ", "Theta");
        m.insert("Λ", "Lambda");
        m.insert("Ξ", "Xi");
        m.insert("Π", "Pi");
        m.insert("Σ", "Sigma");
        m.insert("Φ", "Phi");
        m.insert("Ψ", "Psi");
        m.insert("Ω", "Omega");
        m.insert("≤", "less than or equal to");
        m.insert("≥", "greater than or equal to");
        m.insert("≠", "not equal to");
        m.insert("≈", "approximately equal to");
        m.insert("≡", "equivalent to");
        m.insert("≪", "much less than");
        m.insert("≫", "much greater than");
        m.insert("±", "plus or minus");
        m.insert("∓", "minus or plus");
        m.insert("×", "times");
        m.insert("÷", "divided by");
        m.insert("∙", "dot");
        m.insert("⋅", "dot");
        m.insert("∈", "in");
        m.insert("∉", "not in");
        m.insert("⊂", "subset of");
        m.insert("⊆", "subset of or equal to");
        m.insert("∪", "union");
        m.insert("∩", "intersection");
        m.insert("∀", "for all");
        m.insert("∃", "there exists");
        m.insert("¬", "not");
        m.insert("∧", "and");
        m.insert("∨", "or");
        m.insert("→", "implies");
        m.insert("←", "from");
        m.insert("↔", "if and only if");
        m.insert("⇒", "implies");
        m.insert("⇔", "if and only if");
        m.insert("∫", "integral of");
        m.insert("∑", "sum of");
        m.insert("∏", "product of");
        m.insert("∞", "infinity");
        m.insert("∂", "partial");
        m.insert("∇", "nabla");
        m.insert("²", " squared");
        m.insert("³", " cubed");
        m.insert("¹", " to the first");
        m.insert("⁰", " to the zero");
        m.insert("⁴", " to the fourth");
        m.insert("⁵", " to the fifth");
        m.insert("⁶", " to the sixth");
        m.insert("⁷", " to the seventh");
        m.insert("⁸", " to the eighth");
        m.insert("⁹", " to the ninth");
        m.insert("√", "square root of");
        m.insert("∥", "parallel to");
        m.insert("⊥", "perpendicular to");
        m.insert("∠", "angle");
        m.insert("°", " degrees");
        m
    })
}

fn symbol_pattern() -> &'static Regex {
    SYMBOL_PATTERN.get_or_init(|| {
        let mut keys: Vec<&str> = math_symbols().keys().copied().collect();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        let pat = keys
            .into_iter()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join("|");
        Regex::new(&pat).unwrap()
    })
}

pub fn normalize_math_symbols(text: &str) -> String {
    let symbols = math_symbols();
    let pat = symbol_pattern();
    let mut result = String::new();
    let mut last = 0usize;
    for m in pat.find_iter(text) {
        let start = m.start();
        let end = m.end();
        result.push_str(&text[last..start]);
        let repl = symbols.get(m.as_str()).unwrap();
        let prev_is_word = start > 0
            && text[..start]
                .chars()
                .last()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let next_is_word = end < text.len()
            && text[end..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let prefix = if prev_is_word && !repl.starts_with([' ', '\t', '\n']) {
            " "
        } else {
            ""
        };
        let suffix = if next_is_word && !repl.ends_with([' ', '\t', '\n']) {
            " "
        } else {
            ""
        };
        result.push_str(prefix);
        result.push_str(repl);
        result.push_str(suffix);
        last = end;
    }
    result.push_str(&text[last..]);
    result
}

fn markdown_link() -> &'static Regex {
    MARKDOWN_LINK.get_or_init(|| Regex::new(r"\[([^\]]+)\]\(https?://[^)]+\)").unwrap())
}
fn url_re() -> &'static Regex {
    URL_RE.get_or_init(|| Regex::new(r#"https?://[^\s<>"]+"#).unwrap())
}
fn trailing_punct() -> &'static Regex {
    TRAILING_PUNCT.get_or_init(|| Regex::new(r"[.,;:!?)\]]+$").unwrap())
}
fn email_re() -> &'static Regex {
    EMAIL_RE.get_or_init(|| {
        Regex::new(r"\b([a-zA-Z0-9._%+\-]+)@([a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})\b").unwrap()
    })
}

fn url_to_spoken(url: &str) -> String {
    let mut domain = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };
    if let Some(idx) = domain.find('/') {
        domain = &domain[..idx];
    }
    if let Some(idx) = domain.find('?') {
        domain = &domain[..idx];
    }
    if let Some(idx) = domain.find('#') {
        domain = &domain[..idx];
    }
    if domain.starts_with("www.") {
        domain = &domain[4..];
    }
    format!("link to {}", domain)
}

pub fn normalize_urls_emails(text: &str) -> String {
    let mut t = markdown_link().replace_all(text, "$1").to_string();
    t = url_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let url = caps.get(0).unwrap().as_str();
            // strip trailing punctuation
            if let Some(m) = trailing_punct().find(url) {
                let trailing = m.as_str();
                let stripped = &url[..m.start()];
                format!("{}{}", url_to_spoken(stripped), trailing)
            } else {
                url_to_spoken(url)
            }
        })
        .to_string();
    t = email_re()
        .replace_all(&t, |caps: &regex::Captures| {
            format!("{} at {}", &caps[1], &caps[2])
        })
        .to_string();
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn math_alpha() {
        let t = normalize_math_symbols("x∈X");
        assert!(t.contains("in"), "got {}", t);
    }
    #[test]
    fn math_word_padding() {
        let t = normalize_math_symbols("a≤b");
        assert!(t.contains("less than or equal to"), "got {}", t);
    }
    #[test]
    fn url_basic() {
        let t = normalize_urls_emails("See https://example.com/path.");
        assert!(t.contains("link to example.com"), "got {}", t);
        assert!(t.contains("."), "should preserve trailing period");
    }
    #[test]
    fn email_basic() {
        let t = normalize_urls_emails("Contact foo@bar.com");
        assert!(t.contains("foo at bar.com"), "got {}", t);
    }
    #[test]
    fn no_panic() {
        let t = normalize_math_symbols("Hello π world ∞");
        assert!(!t.is_empty());
    }
}
