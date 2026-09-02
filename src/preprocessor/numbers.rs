use regex::Regex;
use std::sync::OnceLock;

static PROTECT_PREFIX: &str = "\x00NUMPROTECT";
static CURRENCY_RE: OnceLock<Regex> = OnceLock::new();
static PERCENT_RE: OnceLock<Regex> = OnceLock::new();
static FRACTION_RE: OnceLock<Regex> = OnceLock::new();
static CARDINAL_COMMA_RE: OnceLock<Regex> = OnceLock::new();
static DECIMAL_RE: OnceLock<Regex> = OnceLock::new();
static ORDINAL_RE: OnceLock<Regex> = OnceLock::new();
static YEAR_CONTEXT_RE: OnceLock<Regex> = OnceLock::new();
static IP_RE: OnceLock<Regex> = OnceLock::new();
static VERSION_RE: OnceLock<Regex> = OnceLock::new();
static PHONE_RE: OnceLock<Regex> = OnceLock::new();
static HYPH_RE: OnceLock<Regex> = OnceLock::new();

fn currency_re() -> &'static Regex {
    CURRENCY_RE.get_or_init(|| Regex::new(r"\$(\d{1,3}(?:,\d{3})*(?:\.\d{1,2})?)\b").unwrap())
}
fn percent_re() -> &'static Regex {
    PERCENT_RE.get_or_init(|| Regex::new(r"\b(\d+(?:\.\d+)?)%").unwrap())
}
fn fraction_re() -> &'static Regex {
    FRACTION_RE.get_or_init(|| Regex::new(r"\b(\d)/(\d)\b").unwrap())
}
fn cardinal_comma_re() -> &'static Regex {
    CARDINAL_COMMA_RE.get_or_init(|| Regex::new(r"\b(\d{1,3}(?:,\d{3})+)\b").unwrap())
}
fn decimal_re() -> &'static Regex {
    DECIMAL_RE.get_or_init(|| Regex::new(r"\b(\d+)\.(\d+)\b").unwrap())
}
fn ordinal_re() -> &'static Regex {
    ORDINAL_RE.get_or_init(|| Regex::new(r"\b(\d{1,6})(st|nd|rd|th)\b").unwrap())
}
fn year_context_re() -> &'static Regex {
    YEAR_CONTEXT_RE.get_or_init(|| {
        Regex::new(r"\b(?:in|by|since|until|circa|around|from|after|before|during)\s+(\d{4})\b")
            .unwrap()
    })
}

static ONES: &[&str] = &[
    "",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
static TENS: &[&str] = &[
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn int_to_words(n: i64) -> String {
    if n == 0 {
        return "zero".to_string();
    }
    if !(0..=999_999).contains(&n) {
        return n.to_string();
    }
    let mut parts = Vec::new();
    let mut num = n;
    let scales = ["", "thousand", "million"];
    let mut idx = 0;
    while num > 0 {
        let chunk = (num % 1000) as i32;
        num /= 1000;
        if chunk != 0 {
            let mut w = chunk_to_words(chunk);
            if !scales[idx].is_empty() {
                w = format!("{} {}", w, scales[idx]);
            }
            parts.push(w);
        }
        idx += 1;
    }
    parts.reverse();
    parts.join(" ")
}
fn chunk_to_words(n: i32) -> String {
    if n < 20 {
        return ONES[n as usize].to_string();
    }
    if n < 100 {
        let tens = n / 10;
        let ones = n % 10;
        if ones == 0 {
            return TENS[tens as usize].to_string();
        }
        return format!("{}-{}", TENS[tens as usize], ONES[ones as usize]);
    }
    let hundreds = n / 100;
    let rem = n % 100;
    let mut r = format!("{} hundred", ONES[hundreds as usize]);
    if rem != 0 {
        r = format!("{} {}", r, chunk_to_words(rem));
    }
    r
}

fn year_to_words(n: i32) -> String {
    if (2000..=2009).contains(&n) {
        if n == 2000 {
            return "two thousand".to_string();
        }
        return format!("two thousand {}", ONES[(n - 2000) as usize]);
    }
    if (2010..=2099).contains(&n) {
        return format!("twenty {}", chunk_to_words(n - 2000));
    }
    let hi = n / 100;
    let lo = n % 100;
    let hi_w = chunk_to_words(hi);
    if lo == 0 {
        return format!("{} hundred", hi_w);
    }
    format!("{} {}", hi_w, chunk_to_words(lo))
}

fn protect(text: String) -> (String, Vec<(String, String)>) {
    let mut t = text;
    let mut restores = Vec::new();
    let patterns: Vec<(Regex, &str)> = vec![
        (
            Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            "IP",
        ),
        (Regex::new(r"\b[vV]\d+\.\d+(?:\.\d+)*\b").unwrap(), "VER"),
        (
            Regex::new(r"\(?\d{3}\)?[-.\s]\d{3}[-.\s]\d{4}\b").unwrap(),
            "PHONE",
        ),
        (Regex::new(r"\b\d+(?:-\d+){2,}\b").unwrap(), "HYPH"),
    ];
    for (pat, tag) in patterns {
        let mut offset = 0;
        // find all matches in original t (clone)
        let matches: Vec<String> = pat
            .find_iter(&t.clone())
            .map(|m| m.as_str().to_string())
            .collect();
        for m in matches {
            let placeholder = format!("{}{}{}\x00", PROTECT_PREFIX, tag, restores.len());
            let ph_len = placeholder.len();
            // replace first occurrence
            if let Some(pos) = t[offset..].find(&m) {
                let abs = offset + pos;
                t.replace_range(abs..abs + m.len(), &placeholder);
                restores.push((placeholder, m.clone()));
                offset = abs + ph_len;
            }
        }
    }
    (t, restores)
}
fn restore(mut text: String, restores: Vec<(String, String)>) -> String {
    for (ph, orig) in restores.into_iter().rev() {
        text = text.replace(&ph, &orig);
    }
    text
}

const REFERENCE_WORDS: &[&str] = &[
    "Section",
    "Figure",
    "Table",
    "Equation",
    "Algorithm",
    "Theorem",
    "Lemma",
    "Corollary",
    "Proposition",
    "Example",
    "Definition",
    "Remark",
    "Chapter",
    "Step",
    "Appendix",
    "Listing",
    "Reference",
    "Volume",
    "Number",
    "section",
    "figure",
    "table",
    "equation",
    "page",
    "pages",
];

fn find_reference_positions(text: &str) -> std::collections::HashSet<usize> {
    let pat = format!(r"\b(?:{})\s+(\d[\d\.]*)", REFERENCE_WORDS.join("|"));
    let re = Regex::new(&pat).unwrap();
    let mut set = std::collections::HashSet::new();
    for m in re.find_iter(text) {
        // find group 1 start
        if let Some(caps) = re.captures(m.as_str()) {
            if let Some(g1) = caps.get(1) {
                // approximate position: m.start() + prefix len
                // simpler: find the number substring position in text
                let num = g1.as_str();
                if let Some(pos) = text[m.start()..m.end()].find(num) {
                    set.insert(m.start() + pos);
                }
            }
        }
    }
    set
}

pub fn normalize_numbers(text: &str) -> String {
    let (mut t, restores) = protect(text.to_string());
    let ref_positions = find_reference_positions(&t);

    // ordinals
    t = ordinal_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let start = caps.get(0).unwrap().start();
            if ref_positions.contains(&start) {
                return caps[0].to_string();
            }
            let n: i32 = caps[1].parse().unwrap_or(0);
            if !(1..=999_999).contains(&n) {
                return caps[0].to_string();
            }
            // simple ordinal words map for small numbers
            let words = [
                (1, "first"),
                (2, "second"),
                (3, "third"),
                (4, "fourth"),
                (5, "fifth"),
                (6, "sixth"),
                (7, "seventh"),
                (8, "eighth"),
                (9, "ninth"),
                (10, "tenth"),
                (11, "eleventh"),
                (12, "twelfth"),
                (13, "thirteenth"),
                (14, "fourteenth"),
                (15, "fifteenth"),
                (16, "sixteenth"),
                (17, "seventeenth"),
                (18, "eighteenth"),
                (19, "nineteenth"),
                (20, "twentieth"),
                (30, "thirtieth"),
                (40, "fortieth"),
                (50, "fiftieth"),
                (60, "sixtieth"),
                (70, "seventieth"),
                (80, "eightieth"),
                (90, "ninetieth"),
            ];
            for (k, v) in words {
                if k == n {
                    return v.to_string();
                }
            }
            // fallback
            format!("{}th", int_to_words(n as i64))
        })
        .to_string();

    // currency
    t = currency_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let raw = caps[1].replace(',', "");
            if raw.contains('.') {
                let parts: Vec<&str> = raw.split('.').collect();
                let dollars: i64 = parts[0].parse().unwrap_or(0);
                let cents: i64 = parts[1].parse().unwrap_or(0);
                if dollars > 999_999 {
                    return caps[0].to_string();
                }
                let mut res = format!(
                    "{} dollar{}",
                    int_to_words(dollars),
                    if dollars != 1 { "s" } else { "" }
                );
                if cents != 0 {
                    res = format!(
                        "{} and {} cent{}",
                        res,
                        int_to_words(cents),
                        if cents != 1 { "s" } else { "" }
                    );
                }
                res
            } else {
                let dollars: i64 = raw.parse().unwrap_or(0);
                if dollars > 999_999 {
                    return caps[0].to_string();
                }
                format!(
                    "{} dollar{}",
                    int_to_words(dollars),
                    if dollars != 1 { "s" } else { "" }
                )
            }
        })
        .to_string();

    // percentage
    t = percent_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let raw = caps[1].to_string();
            if raw.contains('.') {
                let parts: Vec<&str> = raw.split('.').collect();
                let n: i64 = parts[0].parse().unwrap_or(0);
                if n > 999_999 {
                    return caps[0].to_string();
                }
                let frac_spoken: String = parts[1]
                    .chars()
                    .map(|d| {
                        if d == '0' {
                            "zero".to_string()
                        } else {
                            ONES[d.to_digit(10).unwrap() as usize].to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} point {} percent", int_to_words(n), frac_spoken)
            } else {
                let n: i64 = raw.parse().unwrap_or(0);
                if n > 999_999 {
                    return caps[0].to_string();
                }
                format!("{} percent", int_to_words(n))
            }
        })
        .to_string();

    // fractions
    t = fraction_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let num: i32 = caps[1].parse().unwrap_or(0);
            let den: i32 = caps[2].parse().unwrap_or(0);
            let key = (num, den);
            let map = [
                (1, 2, "one half"),
                (1, 3, "one third"),
                (1, 4, "one quarter"),
                (1, 5, "one fifth"),
                (1, 8, "one eighth"),
                (2, 3, "two thirds"),
                (3, 4, "three quarters"),
                (3, 8, "three eighths"),
                (5, 8, "five eighths"),
                (7, 8, "seven eighths"),
            ];
            for (a, b, w) in map {
                if a == key.0 && b == key.1 {
                    return w.to_string();
                }
            }
            caps[0].to_string()
        })
        .to_string();

    // years
    t = year_context_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let n: i32 = caps[1].parse().unwrap_or(0);
            if (1800..=2099).contains(&n) {
                let full = caps[0].to_string();
                let prefix_len = full.len() - caps[1].len();
                let prefix = &full[..prefix_len];
                format!("{}{}", prefix, year_to_words(n))
            } else {
                caps[0].to_string()
            }
        })
        .to_string();

    // cardinals with commas
    t = cardinal_comma_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let start = caps.get(0).unwrap().start();
            if ref_positions.contains(&start) {
                return caps[0].to_string();
            }
            let raw = caps[1].replace(',', "");
            let n: i64 = raw.parse().unwrap_or(0);
            if n > 999_999 {
                return caps[0].to_string();
            }
            int_to_words(n)
        })
        .to_string();

    // decimals
    t = decimal_re()
        .replace_all(&t, |caps: &regex::Captures| {
            let start = caps.get(0).unwrap().start();
            if ref_positions.contains(&start) {
                return caps[0].to_string();
            }
            let integer = &caps[1];
            let frac = &caps[2];
            let n: i64 = integer.parse().unwrap_or(0);
            if n > 999_999 {
                return caps[0].to_string();
            }
            let spoken_frac: String = frac
                .chars()
                .map(|d| {
                    if d == '0' {
                        "zero".to_string()
                    } else {
                        ONES[d.to_digit(10).unwrap() as usize].to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} point {}", int_to_words(n), spoken_frac)
        })
        .to_string();

    restore(t, restores)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn currency_basic() {
        let t = normalize_numbers("It costs $100.");
        assert!(t.contains("one hundred dollars"), "got {}", t);
    }
    #[test]
    fn protect_ip() {
        let t = normalize_numbers("IP 192.168.1.1 here");
        assert!(t.contains("192.168.1.1"), "got {}", t);
    }
    #[test]
    fn percent() {
        let t = normalize_numbers("50% done");
        assert!(t.contains("fifty percent"), "got {}", t);
    }
    #[test]
    fn no_panic_unicode() {
        let t = normalize_numbers("Hello 123 world");
        assert!(!t.is_empty());
    }
}
