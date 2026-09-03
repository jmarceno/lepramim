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
    fn pipeline_abbreviations_exact() {
        let sentences = preprocess(
            "Hello world. This is a test with Fig. 3 and e.g., stuff.",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "Hello world.".to_string(),
                "This is a test with Figure 3 and for example stuff.".to_string(),
            ]
        );
    }
    #[test]
    fn pipeline_unicode_greek_exact() {
        let sentences = preprocess("Hello α → β world. Test ∞.", None);
        assert_eq!(
            sentences,
            vec![
                "Hello alpha implies beta world.".to_string(),
                "Test infinity.".to_string(),
            ]
        );
    }
    #[test]
    fn pipeline_citation_strip_idempotent() {
        // Numeric brackets are stripped, parentheticals kept (disabled by
        // default); a second pass over the output must be a fixed point.
        let s1 = preprocess("Test with [12] citation (Smith, 2023).", None);
        assert_eq!(s1, vec!["Test with citation (Smith, 2023).".to_string()]);
        let s2 = preprocess(&s1.join(" "), None);
        assert_eq!(s2, s1);
    }

    #[test]
    fn markdown_pipeline_exact() {
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
    fn pipeline_mathjax_stacked_dedupe() {
        // Stacked one-char-per-line MathJax duplicates are removed; the
        // surviving sentence gets symbol expansion.
        let sentences = preprocess(
            "ρ\nρ\nX\nX\nu∈U\nu∈U\nThe policy is defined as ρ(x) = 1.",
            None,
        );
        assert_eq!(
            sentences,
            vec!["The policy is defined as rho of x equals 1.".to_string()]
        );
        let joined = sentences.join(" ");
        for debris in ["X X", "x∈X x∈X", "u∈U u∈U"] {
            assert!(!joined.contains(debris), "debris {debris:?} survived");
        }
    }

    #[test]
    fn pipeline_numeric_brackets() {
        let sentences = preprocess(
            "As shown in [1] and [2-4], the method works [1,3,5]. Keep [a] and [hello].",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "As shown in and the method works.".to_string(),
                "Keep [a] and [hello].".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_parenthetical_disabled_by_default() {
        // Parentheticals are kept, but abbreviation expansion still runs, so
        // "et al." becomes "et alia." and the segmenter splits there.
        let sentences = preprocess(
            "This was shown (Smith 2023) and (Doe et al. 2021) but keep (see Fig. 3).",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "This was shown (Smith 2023) and (Doe et alia.".to_string(),
                "2021) but keep (see Figure 3).".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_parenthetical_enabled() {
        let cfg = PreprocessorConfig {
            strip_parenthetical_citations: true,
            ..Default::default()
        };
        let sentences = preprocess(
            "This was shown (Smith 2023) and (Doe et al. 2021). Keep (hello world).",
            Some(&cfg),
        );
        assert_eq!(
            sentences,
            vec![
                "This was shown and.".to_string(),
                "Keep (hello world).".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_latin_abbreviations() {
        let sentences = preprocess("e.g. this is i.e. an example etc. and et al.", None);
        assert_eq!(
            sentences,
            vec!["for example this is that is an example et cetera and et alia.".to_string()]
        );
    }

    #[test]
    fn pipeline_academic_abbreviations() {
        let sentences = preprocess(
            "See Fig. 3 and Eq. 2 in Sec. 4. By Thm. 1, Chap. 2 is relevant. Ref. [1].",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "See Figure 3 and Equation 2 in Section 4.".to_string(),
                "By Theorem 1, Chapter 2 is relevant.".to_string(),
                "Reference [1].".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_numbers_cardinal_percent_currency() {
        let sentences = preprocess("There are 1,234 items and 50% of $100. See Figure 3.", None);
        let joined = sentences.join(" ");
        for needle in [
            "one thousand two hundred thirty-four",
            "fifty percent",
            "one hundred dollars",
            "Figure 3",
        ] {
            assert!(joined.contains(needle), "missing {needle:?} in {joined:?}");
        }
    }

    #[test]
    fn pipeline_numbers_ordinals_decimals_date_preserved() {
        // Hyphenated dates are protected, not spoken digit-by-digit.
        let sentences = preprocess("On 2024-01-15, 3.14 and 1st, 2nd, 3rd places.", None);
        let joined = sentences.join(" ");
        assert!(joined.contains("2024-01-15"), "got {joined:?}");
        for needle in ["three point one four", "first", "second", "third"] {
            assert!(joined.contains(needle), "missing {needle:?} in {joined:?}");
        }
    }

    #[test]
    fn pipeline_math_symbols() {
        let sentences = preprocess("Let α = β + γ where α ≤ β and x → y.", None);
        assert_eq!(
            sentences,
            vec![
                "Let alpha = beta + gamma where alpha less than or equal to beta and x implies y."
                    .to_string()
            ]
        );
    }

    #[test]
    fn pipeline_urls_emails() {
        let sentences = preprocess(
            "Visit https://example.com/path and email test@example.com",
            None,
        );
        assert_eq!(
            sentences,
            vec!["Visit link to example.com and email test at example.com".to_string()]
        );
    }

    #[test]
    fn pipeline_pdf_hyphenation() {
        let sentences = preprocess(
            "This is a hyphen-\nated word and  multiple   spaces.\n\nNew paragraph.",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "This is a hyphenated word and multiple spaces.".to_string(),
                "New paragraph.".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_segmentation_protects_references() {
        let sentences = preprocess(
            "Dr. Smith went to Washington. He met Mr. Jones. See Fig. 3 for details. This is sentence four.",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "Dr. Smith went to Washington.".to_string(),
                "He met Mr. Jones.".to_string(),
                "See Figure 3 for details.".to_string(),
                "This is sentence four.".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_ellipsis_and_quotes() {
        let sentences = preprocess(
            "He said \"Hello... is anyone there?\" She replied: Yes! Indeed.",
            None,
        );
        assert_eq!(
            sentences,
            vec![
                "He said \"Hello... is anyone there?\"".to_string(),
                "She replied: Yes!".to_string(),
                "Indeed.".to_string(),
            ]
        );
    }

    #[test]
    fn pipeline_latex_without_sre_keeps_words() {
        // Without the optional SRE binary the LaTeX span passes through, but
        // the surrounding words must survive intact either way.
        let sentences = preprocess(
            "The equation $\\alpha + \\beta = \\gamma$ is important.",
            None,
        );
        let joined = sentences.join(" ");
        assert!(joined.contains("equation"), "got {joined:?}");
        assert!(joined.contains("important"), "got {joined:?}");
    }

    #[test]
    fn pipeline_mathjax_sample_end_to_end() {
        // Inline MathJax/KaTeX paste: stacked duplicates removed, symbols
        // expanded, no markdown/zalgo debris in the output.
        let input = "Reinforcement Learning Problem\nRL systems are defined by a tuple of elements\n{\nX\n,\nU\n,\nT\n,\nR\n,\nρ\n0\n}\n{X,U,T,R,ρ\u{a0}\n0\n\u{200b}\n\u{a0}}. Here\u{a0}\nX\nX stands for the set of possible agent states i.e.\u{a0}\nx\n∈\nX\nx∈X, for instance the position and velocity of the child and bike in the above example.\u{a0}\nu\n∈\nU\nu∈U is the action taken to steer the evolution of the state, e.g., the forces exerted on the bike pedals. Numerically\u{a0}\nx\nx and\u{a0}\nu\nu are usually vectors whose entries include quantities that the agent can acquire and implement.";
        let sentences = preprocess(input, None);
        assert!(!sentences.is_empty());
        assert!(sentences.len() >= 3, "got {sentences:?}");
        let joined = sentences.join(" ");
        assert!(
            joined.to_lowercase().contains("rho"),
            "missing rho in {joined:?}"
        );
        for debris in [
            "X X",
            "x∈X x∈X",
            "u∈U u∈U",
            "**",
            "~~",
            "```",
            "\u{200b}",
            "\u{a0}",
        ] {
            assert!(!joined.contains(debris), "debris {debris:?} in {joined:?}");
        }
        for s in &sentences {
            assert!(!s.starts_with('#'), "hash sentence: {s:?}");
        }
        // A second pass must not change the sentence count.
        let again = preprocess(&joined, None);
        assert_eq!(again.len(), sentences.len());
    }

    #[test]
    fn pipeline_regression_properties() {
        // No panics, valid Unicode, bounded output, idempotent cleanup.
        let inputs = [
            "Hello world.",
            "Fig. 3 and Eq. 2",
            "https://example.com",
            "\u{200b}\u{a0}test\u{200b}",
            "   ",
        ];
        for input in inputs {
            let out = preprocess(input, None);
            let joined = out.join(" ");
            assert!(
                joined.len() <= 4 * input.len() + 64,
                "output blew up for {input:?}: {joined:?}"
            );
            let again = preprocess(&joined, None);
            if input.trim().is_empty() {
                assert!(out.is_empty(), "whitespace must yield nothing");
            } else {
                assert!(!out.is_empty(), "lost input {input:?}");
                assert_eq!(again, out, "not idempotent for {input:?}");
            }
        }
    }

    #[test]
    fn pipeline_empty_and_whitespace() {
        assert!(preprocess("   \n\n  \n", None).is_empty());
    }
}
