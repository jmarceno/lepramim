//! COSMIC global shortcuts.
//!
//! cosmic-comp only honours shortcuts listed in its own config; KGlobalAccel
//! (our KDE path) is D-Bus activatable there but never receives key presses.
//! On COSMIC we therefore add Super+R / Super+P to the user's custom shortcut
//! file, spawning `busctl` calls into the `org.lepramim.App` service that
//! `hotkeys.rs` exports. cosmic-settings-daemon watches the file, so the
//! bindings apply without a re-login.

use std::path::PathBuf;

const MARKER: &str = "org.lepramim.App";

struct Binding {
    key: &'static str,
    method: &'static str,
    description: &'static str,
}

const BINDINGS: [Binding; 2] = [
    Binding {
        key: "r",
        method: "SpeakSelection",
        description: "Lepramim: Speak highlighted selection",
    },
    Binding {
        key: "p",
        method: "Toggle",
        description: "Lepramim: Pause / resume",
    },
];

impl Binding {
    fn entry(&self) -> String {
        format!(
            "(\n        modifiers: [\n            Super,\n        ],\n        key: \"{}\",\n        \
             description: Some(\"{}\"),\n    ): Spawn(\"busctl --user call {MARKER} /org/lepramim/App {MARKER} {}\")",
            self.key, self.description, self.method
        )
    }
}

pub fn is_cosmic() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|d| d.eq_ignore_ascii_case("cosmic"))
}

fn custom_path() -> Option<PathBuf> {
    let base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => directories::BaseDirs::new()?.home_dir().join(".config"),
    };
    Some(base.join("cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom"))
}

/// Add or refresh the Lepramim bindings in COSMIC's custom shortcut file.
pub fn ensure() -> Result<bool, String> {
    let path = custom_path().ok_or("no home directory")?;
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{\n}".to_string(),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let Some(updated) = merge(&existing)? else {
        return Ok(false);
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if path.is_file() {
        let backup = path.with_file_name("custom.lepramim.bak");
        if !backup.exists() {
            let _ = std::fs::copy(&path, &backup);
        }
    }
    let tmp = path.with_file_name(".custom.lepramim.tmp");
    std::fs::write(&tmp, updated).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Returns the new file contents, or `None` when nothing needs changing.
/// Existing Super+R/P bindings are replaced only when they are `Disable` or
/// already ours; a user's own binding on those keys is left alone.
fn merge(existing: &str) -> Result<Option<String>, String> {
    let mut entries = split_entries(existing)?;
    let mut changed = false;
    for binding in &BINDINGS {
        let wanted = binding.entry();
        let found = entries
            .iter()
            .position(|e| key_of(e).is_some_and(|k| k == ("super".to_string(), binding.key)));
        match found {
            Some(i) if entries[i] == wanted => {}
            Some(i) => {
                let action = action_of(&entries[i]);
                if action == "Disable" || action.contains(MARKER) {
                    entries[i] = wanted;
                    changed = true;
                } else {
                    tracing::warn!(
                        "cosmic shortcuts: Super+{} is already bound to {action}; leaving it",
                        binding.key.to_uppercase()
                    );
                }
            }
            None => {
                entries.push(wanted);
                changed = true;
            }
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut out = String::from("{\n");
    for e in &entries {
        out.push_str("    ");
        out.push_str(e);
        out.push_str(",\n");
    }
    out.push_str("}");
    Ok(Some(out))
}

/// Split a RON map body into its top-level `key: value` entries (trimmed).
fn split_entries(src: &str) -> Result<Vec<String>, String> {
    let src = src.trim();
    let inner = src
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or("custom shortcuts file is not a RON map")?;
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let e = inner[start..i].trim();
                if !e.is_empty() {
                    entries.push(e.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 || in_str {
        return Err("unbalanced custom shortcuts file".into());
    }
    let e = inner[start..].trim();
    if !e.is_empty() {
        entries.push(e.to_string());
    }
    Ok(entries)
}

/// Index of the `:` separating an entry's key tuple from its action.
fn key_end(entry: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in entry.char_indices() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// (sorted lowercase modifiers joined by '+', key lowercase)
fn key_of(entry: &str) -> Option<(String, &'static str)> {
    let key_part: String = entry[..key_end(entry)?]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mods_start = key_part.find("modifiers:[")? + "modifiers:[".len();
    let mods_end = mods_start + key_part[mods_start..].find(']')?;
    let mut mods: Vec<String> = key_part[mods_start..mods_end]
        .split(',')
        .filter(|m| !m.is_empty())
        .map(|m| m.to_ascii_lowercase())
        .collect();
    mods.sort();
    let key_start = key_part.find("key:\"")? + "key:\"".len();
    let key_end = key_start + key_part[key_start..].find('"')?;
    let key = key_part[key_start..key_end].to_ascii_lowercase();
    let key = BINDINGS.iter().find(|b| b.key == key)?.key;
    Some((mods.join("+"), key))
}

fn action_of(entry: &str) -> String {
    key_end(entry)
        .map(|i| entry[i..].trim_start().trim_start_matches(':').trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_FILE: &str = r#"{
    (
        modifiers: [
            Super,
        ],
        key: "Return",
    ): System(Terminal),
    (
        modifiers: [
            Ctrl,
        ],
        key: "Home",
    ): Spawn("pkill -USR2 -x handy"),
    (
        modifiers: [],
        key: "Print",
    ): Spawn("cosmicshot region"),
    (
        modifiers: [
            Super,
        ],
        key: "r",
    ): Disable,
}"#;

    #[test]
    fn replaces_disabled_super_r_and_adds_super_p() {
        let out = merge(USER_FILE).unwrap().unwrap();
        let entries = split_entries(&out).unwrap();
        assert_eq!(entries.len(), 5);
        assert!(!out.contains("Disable"));
        assert!(out.contains("pkill -USR2 -x handy"));
        assert!(out.contains("cosmicshot region"));
        assert!(out.contains("org.lepramim.App SpeakSelection"));
        assert!(out.contains("org.lepramim.App Toggle"));
    }

    #[test]
    fn idempotent() {
        let once = merge(USER_FILE).unwrap().unwrap();
        assert_eq!(merge(&once).unwrap(), None);
    }

    #[test]
    fn keeps_user_binding_on_super_r() {
        let src = r#"{
    (modifiers: [Super], key: "r"): Spawn("my-thing"),
}"#;
        let out = merge(src).unwrap().unwrap();
        assert!(out.contains("my-thing"));
        assert!(!out.contains("SpeakSelection"));
        assert!(out.contains("Toggle"));
    }

    #[test]
    fn ignores_shifted_super_r_and_handles_empty_file() {
        let src = r#"{(modifiers: [Super, Shift], key: "r"): Resizing(Inwards)}"#;
        let out = merge(src).unwrap().unwrap();
        assert!(out.contains("Resizing(Inwards)"));
        assert!(out.contains("SpeakSelection"));
        assert!(merge("{\n}").unwrap().unwrap().contains("SpeakSelection"));
    }

    #[test]
    fn strings_with_commas_and_parens_survive() {
        let src = r#"{(modifiers: [Ctrl], key: "a", description: Some("x, (y)")): Spawn("sh -c 'a, b)'")}"#;
        let entries = split_entries(src).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(action_of(&entries[0]), r#"Spawn("sh -c 'a, b)'")"#);
    }

    #[test]
    fn rejects_non_map() {
        assert!(merge("garbage").is_err());
    }
}
