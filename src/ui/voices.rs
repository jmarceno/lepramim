//! Kokoro voice and language lists.

pub struct VoiceEntry {
    pub id: &'static str,
    pub label: &'static str,
}

pub struct LanguageEntry {
    pub id: &'static str,
    pub label: &'static str,
}

pub const KOKORO_VOICES: &[VoiceEntry] = &[
    VoiceEntry {
        id: "af_heart",
        label: "Heart \u{2014} American female, warm (default)",
    },
    VoiceEntry {
        id: "af_alloy",
        label: "Alloy \u{2014} American female",
    },
    VoiceEntry {
        id: "af_aoede",
        label: "Aoede \u{2014} American female",
    },
    VoiceEntry {
        id: "af_bella",
        label: "Bella \u{2014} American female",
    },
    VoiceEntry {
        id: "af_jessica",
        label: "Jessica \u{2014} American female",
    },
    VoiceEntry {
        id: "af_kore",
        label: "Kore \u{2014} American female",
    },
    VoiceEntry {
        id: "af_nicole",
        label: "Nicole \u{2014} American female",
    },
    VoiceEntry {
        id: "af_nova",
        label: "Nova \u{2014} American female",
    },
    VoiceEntry {
        id: "af_river",
        label: "River \u{2014} American female",
    },
    VoiceEntry {
        id: "af_sarah",
        label: "Sarah \u{2014} American female",
    },
    VoiceEntry {
        id: "af_sky",
        label: "Sky \u{2014} American female",
    },
    VoiceEntry {
        id: "am_adam",
        label: "Adam \u{2014} American male",
    },
    VoiceEntry {
        id: "am_echo",
        label: "Echo \u{2014} American male",
    },
    VoiceEntry {
        id: "am_eric",
        label: "Eric \u{2014} American male",
    },
    VoiceEntry {
        id: "am_fenrir",
        label: "Fenrir \u{2014} American male",
    },
    VoiceEntry {
        id: "am_liam",
        label: "Liam \u{2014} American male",
    },
    VoiceEntry {
        id: "am_michael",
        label: "Michael \u{2014} American male",
    },
    VoiceEntry {
        id: "am_onyx",
        label: "Onyx \u{2014} American male",
    },
    VoiceEntry {
        id: "am_puck",
        label: "Puck \u{2014} American male",
    },
    VoiceEntry {
        id: "am_santa",
        label: "Santa \u{2014} American male",
    },
    VoiceEntry {
        id: "bf_alice",
        label: "Alice \u{2014} British female",
    },
    VoiceEntry {
        id: "bf_emma",
        label: "Emma \u{2014} British female",
    },
    VoiceEntry {
        id: "bf_isabella",
        label: "Isabella \u{2014} British female",
    },
    VoiceEntry {
        id: "bf_lily",
        label: "Lily \u{2014} British female",
    },
    VoiceEntry {
        id: "bm_daniel",
        label: "Daniel \u{2014} British male",
    },
    VoiceEntry {
        id: "bm_fable",
        label: "Fable \u{2014} British male",
    },
    VoiceEntry {
        id: "bm_george",
        label: "George \u{2014} British male",
    },
    VoiceEntry {
        id: "bm_lewis",
        label: "Lewis \u{2014} British male",
    },
    VoiceEntry {
        id: "ef_dora",
        label: "Dora \u{2014} Spanish female",
    },
    VoiceEntry {
        id: "em_alex",
        label: "Alex \u{2014} Spanish male",
    },
    VoiceEntry {
        id: "em_santa",
        label: "Santa \u{2014} Spanish male",
    },
    VoiceEntry {
        id: "ff_siwis",
        label: "Siwis \u{2014} French female",
    },
    VoiceEntry {
        id: "hf_alpha",
        label: "Alpha \u{2014} Hindi female",
    },
    VoiceEntry {
        id: "hf_beta",
        label: "Beta \u{2014} Hindi female",
    },
    VoiceEntry {
        id: "hm_omega",
        label: "Omega \u{2014} Hindi male",
    },
    VoiceEntry {
        id: "hm_psi",
        label: "Psi \u{2014} Hindi male",
    },
    VoiceEntry {
        id: "if_sara",
        label: "Sara \u{2014} Italian female",
    },
    VoiceEntry {
        id: "im_nicola",
        label: "Nicola \u{2014} Italian male",
    },
    VoiceEntry {
        id: "jf_alpha",
        label: "Alpha \u{2014} Japanese female",
    },
    VoiceEntry {
        id: "jf_gongitsune",
        label: "Gongitsune \u{2014} Japanese female",
    },
    VoiceEntry {
        id: "jf_nezumi",
        label: "Nezumi \u{2014} Japanese female",
    },
    VoiceEntry {
        id: "jf_tebukuro",
        label: "Tebukuro \u{2014} Japanese female",
    },
    VoiceEntry {
        id: "jm_kumo",
        label: "Kumo \u{2014} Japanese male",
    },
    VoiceEntry {
        id: "pf_dora",
        label: "Dora \u{2014} Brazilian Portuguese female",
    },
    VoiceEntry {
        id: "pm_alex",
        label: "Alex \u{2014} Brazilian Portuguese male",
    },
    VoiceEntry {
        id: "pm_santa",
        label: "Santa \u{2014} Brazilian Portuguese male",
    },
    VoiceEntry {
        id: "zf_xiaobei",
        label: "Xiaobei \u{2014} Mandarin Chinese female",
    },
    VoiceEntry {
        id: "zf_xiaoni",
        label: "Xiaoni \u{2014} Mandarin Chinese female",
    },
    VoiceEntry {
        id: "zf_xiaoxiao",
        label: "Xiaoxiao \u{2014} Mandarin Chinese female",
    },
    VoiceEntry {
        id: "zf_xiaoyi",
        label: "Xiaoyi \u{2014} Mandarin Chinese female",
    },
    VoiceEntry {
        id: "zm_yunjian",
        label: "Yunjian \u{2014} Mandarin Chinese male",
    },
    VoiceEntry {
        id: "zm_yunxi",
        label: "Yunxi \u{2014} Mandarin Chinese male",
    },
    VoiceEntry {
        id: "zm_yunxia",
        label: "Yunxia \u{2014} Mandarin Chinese male",
    },
    VoiceEntry {
        id: "zm_yunyang",
        label: "Yunyang \u{2014} Mandarin Chinese male",
    },
];

pub const LANGUAGES: &[LanguageEntry] = &[
    LanguageEntry {
        id: "en-us",
        label: "English (US)",
    },
    LanguageEntry {
        id: "en-gb",
        label: "English (UK)",
    },
    LanguageEntry {
        id: "es",
        label: "Spanish",
    },
    LanguageEntry {
        id: "fr-fr",
        label: "French",
    },
    LanguageEntry {
        id: "hi",
        label: "Hindi",
    },
    LanguageEntry {
        id: "it",
        label: "Italian",
    },
    LanguageEntry {
        id: "ja",
        label: "Japanese",
    },
    LanguageEntry {
        id: "pt-br",
        label: "Portuguese (Brazil)",
    },
    LanguageEntry {
        id: "zh",
        label: "Chinese (Mandarin)",
    },
];

pub fn speed_hint_for_value(speed: f64) -> String {
    if (0.85..=1.3).contains(&speed) {
        return format!("{speed:.2}\u{00d7} \u{2014} safe range for dense reading.");
    }
    if speed < 0.85 {
        return format!("{speed:.2}\u{00d7} \u{2014} slower than natural; may feel dragged.");
    }
    if speed <= 1.5 {
        return format!(
            "{speed:.2}\u{00d7} \u{2014} fine for familiar material, may strain comprehension on new dense text."
        );
    }
    format!(
        "{speed:.2}\u{00d7} \u{2014} risky for unfamiliar academic material; comprehension drops."
    )
}

pub fn speed_from_slider(value: i32) -> f64 {
    let snapped = (value / 5) * 5;
    snapped as f64 / 100.0
}

pub fn speed_to_slider(speed: f64) -> i32 {
    let clamped = speed.clamp(0.5, 2.0);
    (clamped * 100.0).round() as i32
}

/// Map a Kokoro voice id to its native language id.
///
/// Kokoro voice prefixes encode the language:
/// `a` = American English, `b` = British English, `e` = Spanish,
/// `f` = French, `h` = Hindi, `i` = Italian, `j` = Japanese,
/// `p` = Brazilian Portuguese, `z` = Mandarin Chinese.
pub fn voice_language(voice_id: &str) -> &'static str {
    let prefix = voice_id.split('_').next().unwrap_or("");
    match prefix {
        "af" | "am" => "en-us",
        "bf" | "bm" => "en-gb",
        "ef" | "em" => "es",
        "ff" => "fr-fr",
        "hf" | "hm" => "hi",
        "if" | "im" => "it",
        "jf" | "jm" => "ja",
        "pf" | "pm" => "pt-br",
        "zf" | "zm" => "zh",
        _ => "en-us",
    }
}

/// Return the voices that belong to `lang`. Unknown lang ids return everything.
pub fn voices_for_lang(lang: &str) -> Vec<&'static VoiceEntry> {
    let filtered: Vec<&'static VoiceEntry> = KOKORO_VOICES
        .iter()
        .filter(|v| voice_language(v.id) == lang)
        .collect();
    if filtered.is_empty() {
        KOKORO_VOICES.iter().collect()
    } else {
        filtered
    }
}

pub fn language_label(lang_id: &str) -> &str {
    LANGUAGES
        .iter()
        .find(|l| l.id == lang_id)
        .map(|l| l.label)
        .unwrap_or(lang_id)
}

#[derive(Debug, Clone)]
pub struct ControlForm {
    pub voice: String,
    pub lang: String,
    pub speed_slider: i32,
    pub overlay: bool,
    pub dedupe_mathjax: bool,
    pub strip_markdown: bool,
    pub strip_numeric_citations: bool,
    pub expand_latin: bool,
    pub normalize_numbers: bool,
    pub status: String,
    pub unknown_voice_note: bool,
    /// UI-only: when true, the voice pick list only shows voices for `lang`.
    pub filter_voices_by_lang: bool,
}

impl Default for ControlForm {
    fn default() -> Self {
        Self::load_from_config(&crate::config::Config::default())
    }
}

impl ControlForm {
    pub fn load_from_config(cfg: &crate::config::Config) -> Self {
        let mut form = Self {
            voice: cfg.provider.voice.clone(),
            lang: cfg.provider.lang.clone(),
            speed_slider: speed_to_slider(cfg.provider.speed),
            overlay: cfg.advanced.overlay,
            dedupe_mathjax: cfg.preprocessor.dedupe_mathjax_selection,
            strip_markdown: cfg.preprocessor.strip_markdown,
            strip_numeric_citations: cfg.preprocessor.strip_numeric_bracket_citations,
            expand_latin: cfg.preprocessor.expand_latin_abbreviations,
            normalize_numbers: cfg.preprocessor.normalize_numbers,
            status: String::new(),
            unknown_voice_note: false,
            filter_voices_by_lang: false,
        };
        form.unknown_voice_note = !KOKORO_VOICES.iter().any(|v| v.id == form.voice);
        if form.unknown_voice_note {
            form.status = format!(
                "Note: current voice '{}' is outside the curated list; edit ~/.config/lepramim/config.toml directly to keep it.",
                form.voice
            );
        }
        form
    }

    pub fn merge_into_config(&self, base: &crate::config::Config) -> crate::config::Config {
        let mut cfg = base.clone();
        cfg.provider.voice = self.voice.clone();
        cfg.provider.lang = self.lang.clone();
        cfg.provider.speed = speed_from_slider(self.speed_slider);
        cfg.advanced.overlay = self.overlay;
        cfg.preprocessor.dedupe_mathjax_selection = self.dedupe_mathjax;
        cfg.preprocessor.strip_markdown = self.strip_markdown;
        cfg.preprocessor.strip_numeric_bracket_citations = self.strip_numeric_citations;
        cfg.preprocessor.expand_latin_abbreviations = self.expand_latin;
        cfg.preprocessor.normalize_numbers = self.normalize_numbers;
        cfg
    }

    /// Visible voices for the pick list, honoring the language filter.
    pub fn visible_voices(&self) -> Vec<&'static VoiceEntry> {
        if self.filter_voices_by_lang {
            voices_for_lang(&self.lang)
        } else {
            KOKORO_VOICES.iter().collect()
        }
    }

    /// If filtering is on and the current voice is not in the selected
    /// language, snap it to the first voice of that language.
    pub fn ensure_voice_matches_filter(&mut self) {
        if !self.filter_voices_by_lang {
            return;
        }
        if voice_language(&self.voice) != self.lang.as_str() {
            if let Some(first) = voices_for_lang(&self.lang).first() {
                self.voice = first.id.to_string();
                self.unknown_voice_note = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_count() {
        assert_eq!(KOKORO_VOICES.len(), 54);
        assert_eq!(KOKORO_VOICES[0].id, "af_heart");
    }

    #[test]
    fn language_count() {
        assert_eq!(LANGUAGES.len(), 9);
        assert_eq!(LANGUAGES[0].id, "en-us");
    }

    #[test]
    fn speed_mapping() {
        assert!((speed_from_slider(50) - 0.5).abs() < f64::EPSILON);
        assert!((speed_from_slider(100) - 1.0).abs() < f64::EPSILON);
        assert!((speed_from_slider(200) - 2.0).abs() < f64::EPSILON);
        assert_eq!(speed_to_slider(0.1), 50);
        assert_eq!(speed_to_slider(5.0), 200);
    }

    #[test]
    fn speed_hints() {
        assert!(
            speed_hint_for_value(1.0)
                .to_ascii_lowercase()
                .contains("safe")
        );
        assert!(
            speed_hint_for_value(0.6)
                .to_ascii_lowercase()
                .contains("slower")
        );
        assert!(
            speed_hint_for_value(1.4)
                .to_ascii_lowercase()
                .contains("familiar")
        );
        assert!(
            speed_hint_for_value(1.9)
                .to_ascii_lowercase()
                .contains("risky")
        );
    }

    #[test]
    fn config_roundtrip() {
        let mut cfg = crate::config::Config::default();
        cfg.provider.voice = "am_adam".into();
        cfg.provider.lang = "ja".into();
        cfg.provider.speed = 1.75;
        cfg.advanced.overlay = true;
        cfg.preprocessor.dedupe_mathjax_selection = false;
        let form = ControlForm::load_from_config(&cfg);
        assert_eq!(form.voice, "am_adam");
        assert_eq!(form.lang, "ja");
        assert!(form.overlay);
        assert!(!form.dedupe_mathjax);
        let merged = form.merge_into_config(&crate::config::Config::default());
        assert_eq!(merged.provider.voice, "am_adam");
        assert_eq!(merged.provider.lang, "ja");
        assert!(merged.advanced.overlay);
    }

    #[test]
    fn voice_language_mapping() {
        assert_eq!(voice_language("af_heart"), "en-us");
        assert_eq!(voice_language("am_adam"), "en-us");
        assert_eq!(voice_language("bf_alice"), "en-gb");
        assert_eq!(voice_language("bm_george"), "en-gb");
        assert_eq!(voice_language("ef_dora"), "es");
        assert_eq!(voice_language("em_alex"), "es");
        assert_eq!(voice_language("ff_siwis"), "fr-fr");
        assert_eq!(voice_language("hf_alpha"), "hi");
        assert_eq!(voice_language("hm_omega"), "hi");
        assert_eq!(voice_language("if_sara"), "it");
        assert_eq!(voice_language("im_nicola"), "it");
        assert_eq!(voice_language("jf_alpha"), "ja");
        assert_eq!(voice_language("jm_kumo"), "ja");
        assert_eq!(voice_language("pf_dora"), "pt-br");
        assert_eq!(voice_language("pm_alex"), "pt-br");
        assert_eq!(voice_language("zf_xiaobei"), "zh");
        assert_eq!(voice_language("zm_yunjian"), "zh");
    }

    #[test]
    fn voices_for_lang_counts() {
        assert_eq!(voices_for_lang("en-us").len(), 20);
        assert_eq!(voices_for_lang("en-gb").len(), 8);
        assert_eq!(voices_for_lang("es").len(), 3);
        assert_eq!(voices_for_lang("ja").len(), 5);
        assert_eq!(voices_for_lang("zh").len(), 8);
        // Every voice belongs to exactly one language bucket.
        let total: usize = LANGUAGES.iter().map(|l| voices_for_lang(l.id).len()).sum();
        assert_eq!(total, KOKORO_VOICES.len());
    }

    #[test]
    fn filter_snaps_voice_to_language() {
        let mut form = ControlForm {
            voice: "af_heart".into(),
            lang: "ja".into(),
            filter_voices_by_lang: true,
            ..ControlForm::default()
        };
        form.ensure_voice_matches_filter();
        assert_eq!(voice_language(&form.voice), "ja");
        assert_eq!(form.voice, "jf_alpha");

        // Matching voice is left alone.
        form.voice = "jm_kumo".into();
        form.ensure_voice_matches_filter();
        assert_eq!(form.voice, "jm_kumo");

        // Filter off never snaps.
        form.filter_voices_by_lang = false;
        form.voice = "af_heart".into();
        form.lang = "ja".into();
        form.ensure_voice_matches_filter();
        assert_eq!(form.voice, "af_heart");
    }

    #[test]
    fn visible_voices_respects_filter() {
        let mut form = ControlForm::default();
        assert_eq!(form.visible_voices().len(), KOKORO_VOICES.len());
        form.lang = "fr-fr".into();
        form.filter_voices_by_lang = true;
        let visible = form.visible_voices();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "ff_siwis");
    }
}
