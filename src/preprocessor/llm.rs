use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static WARNED: OnceLock<Mutex<bool>> = OnceLock::new();

fn warned() -> &'static Mutex<bool> {
    WARNED.get_or_init(|| Mutex::new(false))
}

#[derive(Debug, Clone)]
pub struct LlmNormalizerConfig {
    pub enabled: bool,
    pub model_path: String,
    pub model_repo: String,
    pub model_file: String,
    pub n_gpu_layers: i32,
    pub n_ctx: u32,
    pub temperature: f64,
    pub max_output_ratio: f64,
    pub glossary: HashMap<String, String>,
}

impl Default for LlmNormalizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_path: String::new(),
            model_repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF".to_string(),
            model_file: "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            n_gpu_layers: -1,
            n_ctx: 4096,
            temperature: 0.0,
            max_output_ratio: 1.5,
            glossary: HashMap::new(),
        }
    }
}

pub struct LlmNormalizer {
    config: LlmNormalizerConfig,
    glossary: Vec<(regex::Regex, String)>,
}

impl LlmNormalizer {
    pub fn new(config: LlmNormalizerConfig) -> Result<Self, String> {
        // Validate model_file doesn't escape cache dir
        if !config.model_file.is_empty()
            && (config.model_file.contains("..") || config.model_file.starts_with('/'))
        {
            // Allow but log; actual containment check in models
            tracing::warn!("model_file contains suspicious path: {}", config.model_file);
        }
        let mut glossary = Vec::new();
        for (abbr, expansion) in &config.glossary {
            if expansion.is_empty() {
                continue;
            }
            let pat = regex::Regex::new(&format!(r"\b{}\b", regex::escape(abbr)))
                .map_err(|e| e.to_string())?;
            glossary.push((pat, expansion.clone()));
        }
        Ok(Self { config, glossary })
    }

    fn apply_glossary(&self, text: &str) -> String {
        let mut t = text.to_string();
        for (pat, repl) in &self.glossary {
            t = pat.replace_all(&t, repl.as_str()).to_string();
        }
        t
    }

    fn needs_llm(&self, text: &str) -> bool {
        // Heuristic gates: LaTeX, tables, or >=2 unknown acronyms
        if text.contains("\\frac") || text.contains("$$") || text.contains('$') {
            return true;
        }
        if text.contains('|') && text.matches('|').count() >= 2 {
            return true;
        }
        // Count uppercase acronyms >=3 letters not in common allowlist
        let common = [
            "AI", "API", "CEO", "GPU", "HTML", "HTTP", "NASA", "URL", "PDF", "SQL",
        ];
        let re = regex::Regex::new(r"\b[A-Z]{3,}\b").unwrap();
        let mut unknown = std::collections::HashSet::new();
        for m in re.find_iter(text) {
            let w = m.as_str();
            if !common.contains(&w) {
                unknown.insert(w.to_string());
            }
        }
        unknown.len() >= 2
    }

    pub async fn warmup(&self) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }
        // Stub: check model file existence
        let model_path = if self.config.model_path.is_empty() {
            crate::models::default_cache_dir().join(&self.config.model_file)
        } else {
            std::path::PathBuf::from(&self.config.model_path)
        };
        if !model_path.is_file() {
            let mut w = warned().lock().unwrap();
            if !*w {
                tracing::warn!(
                    "LLM model file not found: {}. Run `lexaloud download-models --llm`",
                    model_path.display()
                );
                *w = true;
            }
            return Err(format!("model not found: {}", model_path.display()));
        }
        tracing::info!("LLM warmup stub for {}", model_path.display());
        Ok(())
    }

    pub async fn normalize(&self, text: String) -> String {
        let t = self.apply_glossary(&text);
        if !self.needs_llm(&t) {
            return t;
        }
        if !self.config.enabled {
            return t;
        }
        // Stub: if no model, return original
        let model_path = if self.config.model_path.is_empty() {
            crate::models::default_cache_dir().join(&self.config.model_file)
        } else {
            std::path::PathBuf::from(&self.config.model_path)
        };
        if !model_path.is_file() {
            return t;
        }
        // Real inference would happen here; stub returns glossary-applied text
        tracing::warn!("LLM normalize stub: returning glossary-applied text (no inference)");
        t
    }

    pub fn shutdown(&self) {
        tracing::info!("LLM normalizer shutdown");
    }

    fn postprocess(&self, original: &str, output: &str) -> String {
        let out = output.trim().to_string();
        if out.is_empty() {
            tracing::warn!("LLM returned empty output; using original");
            return original.to_string();
        }
        let ratio = out.len() as f64 / original.len().max(1) as f64;
        if !(0.1..=3.0).contains(&ratio) {
            tracing::warn!(
                "LLM output ratio {} outside [0.1,3.0]; using original",
                ratio
            );
            return original.to_string();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn glossary_apply() {
        let mut cfg = LlmNormalizerConfig::default();
        cfg.glossary
            .insert("RLHF".to_string(), "Reinforcement Learning".to_string());
        let n = LlmNormalizer::new(cfg).unwrap();
        let t = n.apply_glossary("We use RLHF here");
        assert!(t.contains("Reinforcement Learning"));
    }
    #[tokio::test]
    async fn normalize_no_llm_passthrough() {
        let cfg = LlmNormalizerConfig::default();
        let n = LlmNormalizer::new(cfg).unwrap();
        let t = n.normalize("Hello world plain prose.".to_string()).await;
        assert_eq!(t, "Hello world plain prose.");
    }
}
