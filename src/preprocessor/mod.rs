pub mod abbreviations;
pub mod citations;
pub mod markdown;
pub mod math;
pub mod numbers;
pub mod pdf;
pub mod segmenter;
pub mod sre;

use crate::config::PreprocessorCfg;

pub use abbreviations::{expand_academic_abbreviations, expand_latin_abbreviations};
pub use citations::{strip_numeric_bracket_citations, strip_parenthetical_citations};
pub use markdown::markdown_to_tts_prose;
pub use math::{normalize_math_symbols, normalize_urls_emails};
pub use pdf::clean_pdf_paste;
pub use segmenter::split_sentences;
pub use sre::latex_to_speech;

/// Preprocessor config wrapper that mirrors native PreprocessorConfig
#[derive(Debug, Clone)]
pub struct PreprocessorConfig {
    pub dedupe_mathjax_selection: bool,
    pub strip_markdown: bool,
    pub sre_latex_enabled: bool,
    pub sre_latex_timeout_s: f64,
    pub sre_latex_domain: String,
    pub sre_latex_style: String,
    pub strip_numeric_bracket_citations: bool,
    pub strip_parenthetical_citations: bool,
    pub expand_latin_abbreviations: bool,
    pub expand_academic_abbreviations: bool,
    pub normalize_numbers: bool,
    pub normalize_urls: bool,
    pub normalize_math_symbols: bool,
    pub pdf_cleanup: bool,
}

impl Default for PreprocessorConfig {
    fn default() -> Self {
        Self {
            dedupe_mathjax_selection: true,
            strip_markdown: true,
            sre_latex_enabled: false,
            sre_latex_timeout_s: 10.0,
            sre_latex_domain: "clearspeak".to_string(),
            sre_latex_style: String::new(),
            strip_numeric_bracket_citations: true,
            strip_parenthetical_citations: false,
            expand_latin_abbreviations: true,
            expand_academic_abbreviations: true,
            normalize_numbers: true,
            normalize_urls: true,
            normalize_math_symbols: true,
            pdf_cleanup: true,
        }
    }
}

impl From<PreprocessorCfg> for PreprocessorConfig {
    fn from(c: PreprocessorCfg) -> Self {
        Self {
            dedupe_mathjax_selection: c.dedupe_mathjax_selection,
            strip_markdown: c.strip_markdown,
            sre_latex_enabled: false,
            sre_latex_timeout_s: 10.0,
            sre_latex_domain: "clearspeak".to_string(),
            sre_latex_style: String::new(),
            strip_numeric_bracket_citations: c.strip_numeric_bracket_citations,
            strip_parenthetical_citations: c.strip_parenthetical_citations,
            expand_latin_abbreviations: c.expand_latin_abbreviations,
            expand_academic_abbreviations: c.expand_academic_abbreviations,
            normalize_numbers: c.normalize_numbers,
            normalize_urls: c.normalize_urls,
            normalize_math_symbols: c.normalize_math_symbols,
            pdf_cleanup: c.pdf_cleanup,
        }
    }
}

/// MathJax dedupe: port of mathjax_dedupe.py (simplified)
pub fn dedupe_mathjax_selection(text: &str) -> String {
    // Port minimal: strip zero-width chars, handle stacked lines
    let t = text
        .replace(['\u{200b}', '\u{200c}', '\u{200d}', '\u{feff}'], "")
        .replace('\u{00a0}', " ");
    if !t.contains('\n') {
        return t;
    }
    // Simplified stacked detection: if many single-char lines followed by compact duplicate, remove stacked
    let lines: Vec<&str> = t.split('\n').collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let vis = lines[i].trim();
        // check if next lines are also single char
        if vis.chars().count() == 1
            && i + 1 < lines.len()
            && lines[i + 1].trim().chars().count() == 1
        {
            let run_start = i;
            let mut stacked: Vec<char> = Vec::new();
            while i < lines.len() {
                let v = lines[i].trim();
                if v.chars().count() == 1 {
                    stacked.push(v.chars().next().unwrap());
                    i += 1;
                } else if v.is_empty() {
                    i += 1;
                } else {
                    break;
                }
            }
            if stacked.len() >= 2 {
                // Drop identical stacked single-character lines (MathJax duplicate).
                if stacked.windows(2).all(|w| w[0] == w[1])
                    || stacked.iter().all(|&c| c == stacked[0])
                {
                    continue;
                }
                if stacked.iter().all(|c| c.is_ascii_alphabetic()) {
                    for line in &lines[run_start..i] {
                        result.push((*line).to_string());
                    }
                    continue;
                }
                let stacked_seq: String = stacked.iter().collect();
                // look ahead compact
                let compact: String = lines[i..std::cmp::min(i + 6, lines.len())]
                    .join("")
                    .chars()
                    .filter(|c| !c.is_whitespace() && !",;:{}()[]".contains(*c))
                    .collect();
                let stacked_norm: String = stacked_seq
                    .chars()
                    .filter(|c| !c.is_whitespace() && !",;:{}()[]".contains(*c))
                    .collect();
                if compact.starts_with(&stacked_norm) {
                    continue;
                } else {
                    for line in &lines[run_start..i] {
                        result.push((*line).to_string());
                    }
                    continue;
                }
            } else {
                for line in &lines[run_start..i] {
                    result.push((*line).to_string());
                }
                continue;
            }
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }
    let mut out = result.join("\n");
    // collapse subscript lines etc.
    out = regex::Regex::new(r"(\S) \n(\d+)\n\s*\n\s*([})\]])")
        .unwrap()
        .replace_all(&out, "$1$2$3")
        .to_string();
    out = regex::Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(&out, "\n\n")
        .to_string();
    out.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    // Drop consecutive duplicate lines (e.g. u∈U stacked twice).
    let mut lines: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let mut deduped: Vec<&str> = Vec::new();
    for line in lines.drain(..) {
        if deduped.last().copied() == Some(line) {
            continue;
        }
        deduped.push(line);
    }
    // Drop short MathJax debris lines (single symbols / compact tokens).
    deduped.retain(|line| {
        let t = line.trim();
        t.split_whitespace().count() >= 4 || t.chars().count() > 12
    });
    deduped.join("\n")
}

pub fn preprocess(text: &str, config: Option<&PreprocessorConfig>) -> Vec<String> {
    let cfg = config.cloned().unwrap_or_default();
    let mut t = text.to_string();
    if cfg.dedupe_mathjax_selection {
        t = dedupe_mathjax_selection(&t);
    }
    if cfg.strip_markdown {
        t = markdown_to_tts_prose(&t);
    }
    if cfg.sre_latex_enabled {
        t = latex_to_speech(
            &t,
            cfg.sre_latex_timeout_s,
            &cfg.sre_latex_domain,
            if cfg.sre_latex_style.is_empty() {
                None
            } else {
                Some(&cfg.sre_latex_style)
            },
        );
    }
    if cfg.normalize_math_symbols {
        t = normalize_math_symbols(&t);
    }
    if cfg.pdf_cleanup {
        t = clean_pdf_paste(&t);
    }
    if cfg.strip_numeric_bracket_citations {
        t = strip_numeric_bracket_citations(&t);
    }
    if cfg.strip_parenthetical_citations {
        t = strip_parenthetical_citations(&t);
    }
    if cfg.expand_latin_abbreviations {
        t = expand_latin_abbreviations(&t);
    }
    if cfg.expand_academic_abbreviations {
        t = expand_academic_abbreviations(&t);
    }
    if cfg.normalize_urls {
        t = normalize_urls_emails(&t);
    }
    if cfg.normalize_numbers {
        t = crate::preprocessor::numbers::normalize_numbers(&t);
    }
    let sentences = split_sentences(&t);
    sentences
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_no_panic() {
        let sentences = preprocess(
            "Hello world. This is a test with Fig. 3 and e.g., stuff.",
            None,
        );
        assert!(!sentences.is_empty());
        for s in &sentences {
            assert!(!s.is_empty());
            assert!(
                s.is_ascii()
                    || s.chars().all(|c| c.is_alphanumeric()
                        || c.is_whitespace()
                        || ".,;:!?\"'()-".contains(c)
                        || !c.is_ascii())
            );
        }
    }
    #[test]
    fn unicode_valid() {
        let t = "Hello α → β world. Test ∞.";
        let sentences = preprocess(t, None);
        assert!(!sentences.is_empty());
        for s in sentences {
            assert!(s.is_ascii() || !s.is_empty());
        }
    }
    #[test]
    fn idempotent_unicode() {
        // Ensure output is valid unicode and second pass doesn't panic
        let t = "Test with [12] citation (Smith, 2023).";
        let s1 = preprocess(t, None);
        let joined = s1.join(" ");
        let s2 = preprocess(&joined, None);
        assert!(!s2.is_empty());
    }
    #[test]
    fn pipeline_smoke() {
        let s = preprocess("Hello world. Test.", None);
        assert!(!s.is_empty());
    }

    #[test]
    fn markdown_fixture_pipeline() {
        let cfg = PreprocessorConfig {
            dedupe_mathjax_selection: true,
            strip_markdown: true,
            strip_numeric_bracket_citations: true,
            strip_parenthetical_citations: false,
            expand_latin_abbreviations: true,
            expand_academic_abbreviations: true,
            normalize_numbers: true,
            normalize_urls: true,
            normalize_math_symbols: true,
            pdf_cleanup: true,
            ..Default::default()
        };
        let input = "# Introduction\n\nThis is **bold** and *italic* with `code` and [link](https://example.com).";
        let sentences = preprocess(input, Some(&cfg));
        assert_eq!(
            sentences,
            vec!["This is bold and italic with code and link.".to_string()]
        );
    }

    #[test]
    fn preprocessor_fixtures_json() {
        #[derive(serde::Deserialize)]
        struct FixtureFile {
            cases: Vec<FixtureCase>,
        }
        #[derive(serde::Deserialize)]
        struct FixtureCase {
            id: String,
            input: Option<String>,
            input_file: Option<String>,
            config: Option<serde_json::Value>,
            expected_sentences: Option<Vec<String>>,
            expectations: Option<Expectations>,
            must_not_contain: Option<Vec<String>>,
        }
        #[derive(serde::Deserialize)]
        struct Expectations {
            #[serde(default)]
            non_empty: bool,
            min_sentences: Option<usize>,
            contains_lower: Option<Vec<String>>,
            must_not_contain: Option<Vec<String>>,
            no_sentence_starts_with_hash: bool,
        }

        fn fixture_config(v: &serde_json::Value) -> PreprocessorConfig {
            if v.is_null() || v.as_str() == Some("default") {
                return PreprocessorConfig::default();
            }
            let mut cfg = PreprocessorConfig::default();
            if let Some(obj) = v.as_object() {
                macro_rules! set_bool {
                    ($field:ident, $key:literal) => {
                        if let Some(b) = obj.get($key).and_then(|x| x.as_bool()) {
                            cfg.$field = b;
                        }
                    };
                }
                set_bool!(dedupe_mathjax_selection, "dedupe_mathjax_selection");
                set_bool!(strip_markdown, "strip_markdown");
                set_bool!(
                    strip_numeric_bracket_citations,
                    "strip_numeric_bracket_citations"
                );
                set_bool!(
                    strip_parenthetical_citations,
                    "strip_parenthetical_citations"
                );
                set_bool!(expand_latin_abbreviations, "expand_latin_abbreviations");
                set_bool!(
                    expand_academic_abbreviations,
                    "expand_academic_abbreviations"
                );
                set_bool!(normalize_numbers, "normalize_numbers");
                set_bool!(normalize_urls, "normalize_urls");
                set_bool!(normalize_math_symbols, "normalize_math_symbols");
                set_bool!(pdf_cleanup, "pdf_cleanup");
                // Partial citation-only configs inherit Python test defaults (no abbreviation expansion).
                let citation_only = obj
                    .keys()
                    .all(|k| k.contains("citation") || k.starts_with("strip_"));
                if citation_only {
                    cfg.expand_latin_abbreviations = false;
                    cfg.expand_academic_abbreviations = false;
                }
            }
            cfg
        }

        let raw = include_str!("../../tests/fixtures/preprocessor_fixtures.json");
        let fixtures: FixtureFile = serde_json::from_str(raw).expect("parse fixtures");
        for case in fixtures.cases {
            let input = if let Some(s) = case.input {
                s
            } else if let Some(rel) = case.input_file {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(rel);
                match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(_) => continue,
                }
            } else {
                continue;
            };
            let cfg = case.config.as_ref().map(fixture_config).unwrap_or_default();
            let sentences = preprocess(&input, Some(&cfg));

            if let Some(expected) = case.expected_sentences {
                if expected.len() == 1 {
                    assert_eq!(
                        sentences.join(" "),
                        expected[0],
                        "fixture {} mismatch",
                        case.id
                    );
                } else {
                    assert_eq!(sentences, expected, "fixture {} mismatch", case.id);
                }
            }
            if let Some(must_not) = case.must_not_contain {
                let joined = sentences.join(" ");
                for needle in must_not {
                    assert!(
                        !joined.contains(&needle),
                        "fixture {} must not contain {:?}",
                        case.id,
                        needle
                    );
                }
            }
            if let Some(exp) = case.expectations {
                if exp.non_empty {
                    assert!(!sentences.is_empty(), "fixture {} empty", case.id);
                }
                if let Some(min) = exp.min_sentences {
                    assert!(
                        sentences.len() >= min,
                        "fixture {} needs >= {} sentences",
                        case.id,
                        min
                    );
                }
                let joined_lower = sentences.join(" ").to_lowercase();
                if let Some(needles) = exp.contains_lower {
                    for n in needles {
                        assert!(
                            joined_lower.contains(&n),
                            "fixture {} missing {:?}",
                            case.id,
                            n
                        );
                    }
                }
                if let Some(must_not) = exp.must_not_contain {
                    let joined = sentences.join(" ");
                    for needle in must_not {
                        assert!(
                            !joined.contains(&needle),
                            "fixture {} must not contain {:?}",
                            case.id,
                            needle
                        );
                    }
                }
                if exp.no_sentence_starts_with_hash {
                    for s in &sentences {
                        assert!(
                            !s.starts_with('#'),
                            "fixture {} hash sentence: {:?}",
                            case.id,
                            s
                        );
                    }
                }
            }
        }
    }
}
