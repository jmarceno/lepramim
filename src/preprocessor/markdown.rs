use regex::Regex;
use std::sync::OnceLock;

static MD_HINT: OnceLock<Regex> = OnceLock::new();

fn md_hint() -> &'static Regex {
    MD_HINT.get_or_init(|| Regex::new(
        r"(?m)(^\s{0,3}#{1,6}\s|^\s{0,3}[-*+]\s|^\s{0,3}\d+\.\s|^\s{0,3}>\s?|^\s{0,3}```|^\s{0,3}-{3,}\s*$|^\s{0,3}\*{3,}\s*$|\*\*[^\s*][^\n*]*\*\*|~~[^\s~][^\n~]*~~|\[[^\]]+\]\([^)]+\)|!\[[^\]]*\]\([^)]+\)|^\|[^\n]*\||</?[a-zA-Z][\w-]*[^>]*>)"
    ).unwrap())
}

fn canonicalize(text: String) -> String {
    let t = Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(&text, "\n\n")
        .to_string();
    let t = Regex::new(r"[ \t]+")
        .unwrap()
        .replace_all(&t, " ")
        .to_string();
    let t = t
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    t.trim().to_string()
}

/// Very simplified markdown stripping using pulldown-cmark.
/// Heuristic: if no markdown hint, return unchanged.
pub fn markdown_to_tts_prose(text: &str) -> String {
    if !md_hint().is_match(text) {
        return text.to_string();
    }

    // Use pulldown-cmark to extract plain text with structure
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, options);

    let mut out = String::new();
    let mut list_counter: Option<usize> = None;
    let mut in_table_cell = false;
    let mut cell_buf = String::new();
    let mut in_code_block = false;
    let mut in_heading = false;

    // Simpler approach: iterate and build out directly, handling tags.
    // We'll do single pass with state machine similar to native but simplified.
    let events: Vec<Event> = parser.collect();

    let mut table_state = 0; // 0 none, 1 head, 2 body
    let mut headers: Vec<String> = Vec::new();
    let mut col_idx = 0;

    for event in events {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading { .. } => {
                        in_heading = true;
                        out.push_str("\n\n");
                    }
                    Tag::Paragraph => {}
                    Tag::List(ordered) => {
                        out.push_str("\n\n");
                        list_counter = ordered.map(|n| n as usize);
                    }
                    Tag::Item => {
                        if let Some(ref mut cnt) = list_counter {
                            out.push_str(&format!("{}. ", *cnt));
                            *cnt += 1;
                        }
                    }
                    Tag::BlockQuote(_) => {
                        out.push_str("Quote. ");
                    }
                    Tag::CodeBlock(_) => {
                        out.push_str("Code block omitted.\n\n");
                        in_code_block = true;
                    }
                    Tag::Table(_) => {
                        table_state = 0;
                        headers.clear();
                        col_idx = 0;
                    }
                    Tag::TableHead => {
                        table_state = 1;
                        col_idx = 0;
                    }
                    Tag::TableRow => {
                        col_idx = 0;
                    }
                    Tag::TableCell => {
                        in_table_cell = true;
                        cell_buf.clear();
                    }
                    Tag::Emphasis | Tag::Strong | Tag::Strikethrough => {}
                    Tag::Link { .. } | Tag::Image { .. } => {}
                    Tag::HtmlBlock | Tag::MetadataBlock(_) => {}
                    _ => {}
                }
                // push to stack if needed
                // Not pushing all
            }
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    in_heading = false;
                    out.push_str("\n\n");
                }
                TagEnd::Paragraph => {
                    out.push_str("\n\n");
                }
                TagEnd::List(_) => {
                    list_counter = None;
                    out.push_str("\n\n");
                }
                TagEnd::Item => {
                    let trimmed = out.trim_end();
                    if let Some(last) = trimmed.chars().last() {
                        if !".!?:;".contains(last) {
                            out.push('.');
                        }
                    }
                    out.push(' ');
                }
                TagEnd::BlockQuote(_) => {
                    let trimmed = out.trim_end();
                    if let Some(last) = trimmed.chars().last() {
                        if !".!?:;".contains(last) {
                            out.push('.');
                        }
                    }
                    out.push_str("\n\n");
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                }
                TagEnd::Table => {
                    out.push_str("\n\n");
                    headers.clear();
                    table_state = 0;
                }
                TagEnd::TableHead => {
                    table_state = 2;
                }
                TagEnd::TableRow => {
                    let trimmed = out.trim_end();
                    if let Some(last) = trimmed.chars().last() {
                        if !".!?:;".contains(last) {
                            out.push('.');
                        }
                    }
                    out.push('\n');
                }
                TagEnd::TableCell => {
                    let body = cell_buf.trim().to_string();
                    cell_buf.clear();
                    in_table_cell = false;
                    if table_state == 1 {
                        headers.push(body);
                    } else if table_state == 2 {
                        if col_idx < headers.len() && !headers[col_idx].is_empty() {
                            out.push_str(&format!("{}: {}, ", headers[col_idx], body));
                        } else {
                            out.push_str(&format!("{}, ", body));
                        }
                        col_idx += 1;
                    } else {
                        out.push_str(&body);
                        out.push(' ');
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {}
                TagEnd::Link => {}
                TagEnd::Image => {}
                TagEnd::HtmlBlock => {}
                _ => {}
            },
            Event::Text(t) => {
                if in_code_block || in_heading {
                    continue;
                }
                if in_table_cell {
                    cell_buf.push_str(&t);
                } else {
                    // Handle table header collection: if in head, push to headers directly
                    if table_state == 1 {
                        // header cell text comes via TableCell already; but Text inside TableCell will be captured there
                        // For non-table, just push
                        // If we are inside TableCell, we already handle via cell_buf, so ignore
                        // But Text outside cell in head should still be header
                        // Simpler: if in head and not in cell, accumulate header
                        // Not needed: pulldown-cmark always puts text inside cell
                        out.push_str(&t);
                    } else {
                        out.push_str(&t);
                    }
                }
            }
            Event::Code(t) => {
                if in_code_block {
                    continue;
                }
                if in_table_cell {
                    cell_buf.push_str(&t);
                } else {
                    out.push_str(&t);
                }
            }
            Event::Html(_) => {
                // strip
            }
            Event::InlineHtml(_) => {
                // strip
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_table_cell {
                    cell_buf.push(' ');
                } else {
                    out.push(' ');
                }
            }
            Event::FootnoteReference(_) => {}
            Event::TaskListMarker(_) => {}
            Event::Rule => {
                out.push_str("\n\n");
            }
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                // keep math as text? For TTS, preserve contents
                // pulldown-cmark's math events carry the raw math string
                // We'll just push a space to avoid concatenation
                out.push(' ');
            }
        }
    }

    // Protect LaTeX delimiters? The native protects \(...\) etc. For Rust stub, just return canonicalized.
    canonicalize(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_hint_passthrough() {
        let t = "Hello world plain prose.";
        assert_eq!(markdown_to_tts_prose(t), t);
    }
    #[test]
    fn heading_stripped() {
        let t = markdown_to_tts_prose("# Title\n\nParagraph.");
        assert!(!t.contains("Title"), "headings should not be spoken: {}", t);
        assert!(t.contains("Paragraph"), "got {}", t);
        assert!(!t.contains("#"), "got {}", t);
    }
    #[test]
    fn link_text_kept() {
        let t = markdown_to_tts_prose("See [example](https://example.com) now.");
        assert!(t.contains("example"), "got {}", t);
        assert!(!t.contains("https://"), "got {}", t);
    }
    #[test]
    fn fixture_markdown_sample() {
        let t = markdown_to_tts_prose(
            "# Introduction\n\nThis is **bold** and *italic* with `code` and [link](https://example.com).",
        );
        assert!(t.contains("bold"), "got {t}");
        assert!(t.contains("link"), "got {t}");
        assert!(!t.contains("Introduction"), "got {t}");
    }

    #[test]
    fn inline_emphasis_stripped() {
        let t = markdown_to_tts_prose("Hello **bold** world.");
        assert_eq!(t, "Hello bold world.");
    }
}
