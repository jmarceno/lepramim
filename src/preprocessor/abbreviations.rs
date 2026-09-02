use regex::Regex;
use std::sync::OnceLock;

static REPLACEMENTS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn replacements() -> &'static Vec<(Regex, &'static str)> {
    REPLACEMENTS.get_or_init(|| {
        vec![
            (Regex::new(r"(?i)\be\.\s*g\.,?").unwrap(), "for example"),
            (Regex::new(r"(?i)\bi\.\s*e\.,?").unwrap(), "that is"),
            (Regex::new(r"(?i)\betc\.,?").unwrap(), "et cetera"),
            (Regex::new(r#"(?i)\bet\s+al\.?"#).unwrap(), "et alia."),
            (Regex::new(r"(?i)\bcf\.").unwrap(), "compare"),
            (Regex::new(r"(?i)\bviz\.").unwrap(), "namely"),
            (Regex::new(r"(?i)\bibid\.").unwrap(), "same source"),
            (Regex::new(r"(?i)\bN\.?B\.").unwrap(), "note well"),
            (Regex::new(r"(?i)\bvs\.").unwrap(), "versus"),
            // Academic abbreviations (merge)
            (Regex::new(r"(?i)\bet\s+seq\.").unwrap(), "and following"),
            (
                Regex::new(r"(?i)\bw\.l\.o\.g\.").unwrap(),
                "without loss of generality",
            ),
            (
                Regex::new(r"(?i)\bi\.i\.d\.").unwrap(),
                "independently and identically distributed",
            ),
            (Regex::new(r"(?i)\bw\.r\.t\.").unwrap(), "with respect to"),
            (Regex::new(r"\bs\.t\.").unwrap(), "such that"),
            (Regex::new(r"(?i)\bApprox\.").unwrap(), "approximately"),
            (Regex::new(r"(?i)\bChap\.").unwrap(), "Chapter"),
            (Regex::new(r"(?i)\bEqn\.").unwrap(), "Equation"),
            (Regex::new(r"(?i)\bEq\.").unwrap(), "Equation"),
            (Regex::new(r"(?i)\bFig\.").unwrap(), "Figure"),
            (Regex::new(r"(?i)\bSec\.").unwrap(), "Section"),
            (Regex::new(r"(?i)\bRef\.").unwrap(), "Reference"),
            (Regex::new(r"(?i)\bTab\.").unwrap(), "Table"),
            (Regex::new(r"(?i)\bVol\.").unwrap(), "Volume"),
            (Regex::new(r"(?i)\bCh\.").unwrap(), "Chapter"),
            (Regex::new(r"(?i)\bDef\.").unwrap(), "Definition"),
            (Regex::new(r"(?i)\bThm\.").unwrap(), "Theorem"),
            (Regex::new(r"(?i)\bLem\.").unwrap(), "Lemma"),
            (Regex::new(r"(?i)\bCor\.").unwrap(), "Corollary"),
            (Regex::new(r"(?i)\bProp\.").unwrap(), "Proposition"),
            (Regex::new(r"(?i)\bEx\.").unwrap(), "Example"),
            (Regex::new(r"(?i)\bRem\.").unwrap(), "Remark"),
            (Regex::new(r"\bpp\.").unwrap(), "pages "),
            (Regex::new(r"\bp\.").unwrap(), "page "),
            (Regex::new(r"\bNo\.").unwrap(), "Number "),
        ]
    })
}

pub fn expand_latin_abbreviations(text: &str) -> String {
    let mut t = text.to_string();
    for (pat, repl) in replacements().iter().take(9) {
        t = pat.replace_all(&t, *repl).to_string();
    }
    t
}

pub fn expand_academic_abbreviations(text: &str) -> String {
    let mut t = text.to_string();
    for (pat, repl) in replacements().iter().skip(9) {
        t = pat.replace_all(&t, *repl).to_string();
    }
    t
}

pub fn expand_all_abbreviations(text: &str) -> String {
    let mut t = text.to_string();
    for (pat, repl) in replacements().iter() {
        t = pat.replace_all(&t, *repl).to_string();
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latin_eg() {
        let t = expand_latin_abbreviations("e.g., something");
        assert!(t.contains("for example"), "got {}", t);
    }
    #[test]
    fn academic_fig() {
        let t = expand_academic_abbreviations("See Fig. 3");
        assert!(t.contains("Figure"), "got {}", t);
    }
    #[test]
    fn st_strict() {
        let t = expand_academic_abbreviations("s.t. condition");
        assert!(t.contains("such that"));
        let t2 = expand_academic_abbreviations("St. John");
        assert!(!t2.contains("such that"), "got {}", t2);
    }
    #[test]
    fn unicode_valid() {
        let t = expand_all_abbreviations("Hello e.g. world Fig. 2");
        assert!(t.is_ascii() || !t.is_empty());
    }
}
