use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

unsafe extern "C" {
    unsafe fn getuid() -> u32;
}

fn get_uid() -> u32 {
    unsafe { getuid() }
}

/// Return the path to the user config file.
///
/// Returns the path to the user config file (XDG-aware).
/// - if `$XDG_CONFIG_HOME` is set, use `$XDG_CONFIG_HOME/lexaloud/config.toml` (resolved)
/// - else `~/.config/lexaloud/config.toml`
pub fn config_path() -> PathBuf {
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        if !base.is_empty() {
            let p = PathBuf::from(&base);
            // Mimic Path(base).resolve(): make absolute and canonicalize if exists.
            let abs = if p.is_absolute() {
                p
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(&p)
            };
            let resolved = std::fs::canonicalize(&abs).unwrap_or(abs);
            return resolved.join("lexaloud").join("config.toml");
        }
    }
    let home = directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| std::env::var("HOME").map(PathBuf::from).ok())
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".config").join("lexaloud").join("config.toml")
}

/// Return the XDG runtime directory for the current user.
///
/// Returns the XDG runtime directory for the current user.
pub fn runtime_dir() -> PathBuf {
    if let Ok(val) = std::env::var("XDG_RUNTIME_DIR") {
        if !val.is_empty() {
            return PathBuf::from(val);
        }
    }
    PathBuf::from(format!("/run/user/{}", get_uid()))
}

/// Return the absolute path to the daemon's Unix domain socket.
///
/// Returns the absolute path to the daemon Unix domain socket.
/// `$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock` inside a mode-0700 directory.
/// Intentionally NOT resolved.
pub fn socket_path() -> PathBuf {
    runtime_dir().join("lexaloud").join("lexaloud.sock")
}

// ---------------------------------------------------------------------------
// Config structs — defaults mirror documented configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
    #[serde(default = "default_subprocess_timeout")]
    pub subprocess_timeout_s: f64,
}

fn default_max_bytes() -> usize {
    200 * 1024
}
fn default_subprocess_timeout() -> f64 {
    2.0
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_bytes: default_max_bytes(),
            subprocess_timeout_s: default_subprocess_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_ready_queue_depth")]
    pub ready_queue_depth: usize,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    5487
}
fn default_ready_queue_depth() -> usize {
    3
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            ready_queue_depth: default_ready_queue_depth(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdvancedConfig {
    #[serde(default)]
    pub overlay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_voice")]
    pub voice: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_speed")]
    pub speed: f64,
}

fn default_voice() -> String {
    "af_heart".to_string()
}
fn default_lang() -> String {
    "en-us".to_string()
}
fn default_speed() -> f64 {
    1.0
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            voice: default_voice(),
            lang: default_lang(),
            speed: default_speed(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessorCfg {
    #[serde(default = "default_true")]
    pub dedupe_mathjax_selection: bool,
    #[serde(default = "default_true")]
    pub strip_markdown: bool,
    #[serde(default = "default_true")]
    pub strip_numeric_bracket_citations: bool,
    #[serde(default = "default_false")]
    pub strip_parenthetical_citations: bool,
    #[serde(default = "default_true")]
    pub expand_latin_abbreviations: bool,
    #[serde(default = "default_true")]
    pub expand_academic_abbreviations: bool,
    #[serde(default = "default_true")]
    pub normalize_numbers: bool,
    #[serde(default = "default_true")]
    pub normalize_urls: bool,
    #[serde(default = "default_true")]
    pub normalize_math_symbols: bool,
    #[serde(default = "default_true")]
    pub pdf_cleanup: bool,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

impl Default for PreprocessorCfg {
    fn default() -> Self {
        Self {
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SreLatexConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_sre_timeout")]
    pub timeout_s: f64,
    #[serde(default = "default_sre_domain")]
    pub domain: String,
    #[serde(default)]
    pub style: String,
}

fn default_sre_timeout() -> f64 {
    10.0
}
fn default_sre_domain() -> String {
    "clearspeak".to_string()
}

impl Default for SreLatexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_s: default_sre_timeout(),
            domain: default_sre_domain(),
            style: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub model_path: String,
    #[serde(default = "default_model_repo")]
    pub model_repo: String,
    #[serde(default = "default_model_file")]
    pub model_file: String,
    #[serde(default = "default_n_gpu_layers")]
    pub n_gpu_layers: i32,
    #[serde(default = "default_n_ctx")]
    pub n_ctx: u32,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default = "default_max_output_ratio")]
    pub max_output_ratio: f64,
    #[serde(default)]
    pub glossary: std::collections::HashMap<String, String>,
}

fn default_model_repo() -> String {
    "Qwen/Qwen2.5-1.5B-Instruct-GGUF".to_string()
}
fn default_model_file() -> String {
    "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string()
}
fn default_n_gpu_layers() -> i32 {
    -1
}
fn default_n_ctx() -> u32 {
    4096
}
fn default_max_output_ratio() -> f64 {
    1.5
}

impl Default for NormalizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_path: String::new(),
            model_repo: default_model_repo(),
            model_file: default_model_file(),
            n_gpu_layers: default_n_gpu_layers(),
            n_ctx: default_n_ctx(),
            temperature: 0.0,
            max_output_ratio: default_max_output_ratio(),
            glossary: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub preprocessor: PreprocessorCfg,
    #[serde(default)]
    pub advanced: AdvancedConfig,
    #[serde(default)]
    pub normalizer: NormalizerConfig,
    #[serde(default)]
    pub sre_latex: SreLatexConfig,
}

/// Load config from `path` or the default `config_path()`.
///
/// Unknown keys are ignored (forward-compatible). TOML parse errors are
/// logged and defaults are returned.
pub fn load_config<P: AsRef<Path>>(path: Option<P>) -> Config {
    let p = path
        .as_ref()
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or_else(config_path);
    if !p.exists() {
        return Config::default();
    }
    let content = match std::fs::read_to_string(&p) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "Could not read {}: {}. Using default configuration.",
                p.display(),
                e
            );
            return Config::default();
        }
    };
    match toml::from_str::<Config>(&content) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!(
                "Failed to parse {}: {}. Using default configuration; edit the file to fix the syntax.",
                p.display(),
                e
            );
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    // Serialize env-var-touching tests to avoid cross-test pollution.
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    // We avoid adding tempfile as dep; instead use std::env::temp_dir and manual file.
    // To keep `cargo check --all-targets` without extra deps, we implement simple helper.

    fn write_temp(content: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("lexaloud_test_{}.toml", rand_suffix()));
        // Simple random suffix without extra crate: use pid + nanos
        std::fs::write(&p, content).unwrap();
        p
    }

    fn rand_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}_{}", std::process::id(), nanos)
    }

    #[test]
    fn defaults_match_native() {
        let cfg = Config::default();
        assert_eq!(cfg.capture.max_bytes, 200 * 1024);
        assert!((cfg.capture.subprocess_timeout_s - 2.0).abs() < 1e-9);
        assert_eq!(cfg.daemon.host, "127.0.0.1");
        assert_eq!(cfg.daemon.port, 5487);
        assert_eq!(cfg.daemon.ready_queue_depth, 3);
        assert_eq!(cfg.provider.voice, "af_heart");
        assert_eq!(cfg.provider.lang, "en-us");
        assert!((cfg.provider.speed - 1.0).abs() < 1e-9);
        assert!(cfg.preprocessor.dedupe_mathjax_selection);
        assert!(cfg.preprocessor.strip_markdown);
        assert!(cfg.preprocessor.strip_numeric_bracket_citations);
        assert!(!cfg.preprocessor.strip_parenthetical_citations);
        assert!(cfg.preprocessor.expand_latin_abbreviations);
        assert!(cfg.preprocessor.expand_academic_abbreviations);
        assert!(cfg.preprocessor.normalize_numbers);
        assert!(cfg.preprocessor.normalize_urls);
        assert!(cfg.preprocessor.normalize_math_symbols);
        assert!(cfg.preprocessor.pdf_cleanup);
        assert!(!cfg.advanced.overlay);
        assert!(!cfg.normalizer.enabled);
        assert_eq!(cfg.normalizer.model_repo, "Qwen/Qwen2.5-1.5B-Instruct-GGUF");
        assert_eq!(
            cfg.normalizer.model_file,
            "qwen2.5-1.5b-instruct-q4_k_m.gguf"
        );
        assert_eq!(cfg.normalizer.n_gpu_layers, -1);
        assert_eq!(cfg.normalizer.n_ctx, 4096);
        assert!((cfg.normalizer.temperature - 0.0).abs() < 1e-9);
        assert!((cfg.normalizer.max_output_ratio - 1.5).abs() < 1e-9);
        assert!(cfg.normalizer.glossary.is_empty());
        assert!(!cfg.sre_latex.enabled);
        assert!((cfg.sre_latex.timeout_s - 10.0).abs() < 1e-9);
        assert_eq!(cfg.sre_latex.domain, "clearspeak");
        assert_eq!(cfg.sre_latex.style, "");
    }

    #[test]
    fn load_valid_toml() {
        let content = r#"
            [capture]
            max_bytes = 12345
            [provider]
            voice = "test_voice"
            speed = 1.5
            [advanced]
            overlay = true
        "#;
        let path = write_temp(content);
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.capture.max_bytes, 12345);
        assert_eq!(cfg.provider.voice, "test_voice");
        assert!((cfg.provider.speed - 1.5).abs() < 1e-9);
        assert!(cfg.advanced.overlay);
        // defaults preserved for unspecified
        assert_eq!(cfg.daemon.port, 5487);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_ignores_unknown_keys() {
        let content = r#"
            unknown_top = "ignored"
            [capture]
            max_bytes = 9999
            unknown_inner = 123
            [unknown_section]
            foo = "bar"
            [provider]
            voice = "x"
            unknown = 1
        "#;
        let path = write_temp(content);
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.capture.max_bytes, 9999);
        assert_eq!(cfg.provider.voice, "x");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_malformed_returns_defaults() {
        let content = "this is not toml ::: [[[[";
        let path = write_temp(content);
        let cfg = load_config(Some(&path));
        // Should return defaults, not panic
        assert_eq!(cfg.capture.max_bytes, 200 * 1024);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let p = PathBuf::from("/tmp/lexaloud_nonexistent_12345.toml");
        let _ = std::fs::remove_file(&p);
        let cfg = load_config(Some(&p));
        assert_eq!(cfg.provider.voice, "af_heart");
    }

    #[test]
    fn config_path_respects_xdg() {
        let _g = env_lock();
        let tmp = std::env::temp_dir().join("lexaloud_xdg_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let orig = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };
        let p = config_path();
        assert!(p.starts_with(&tmp));
        assert!(p.ends_with("lexaloud/config.toml"));
        // restore
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_CONFIG_HOME", v) };
        } else {
            unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
        }
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn runtime_dir_fallback() {
        let _g = env_lock();
        let orig = std::env::var("XDG_RUNTIME_DIR").ok();
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        let rd = runtime_dir();
        let expected = format!("/run/user/{}", get_uid());
        assert_eq!(rd, PathBuf::from(expected));
        let sock = socket_path();
        assert_eq!(sock, rd.join("lexaloud").join("lexaloud.sock"));
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", v) };
        }
    }

    #[test]
    fn runtime_dir_respects_xdg() {
        let _g = env_lock();
        let orig = std::env::var("XDG_RUNTIME_DIR").ok();
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/tmp/my_runtime") };
        assert_eq!(runtime_dir(), PathBuf::from("/tmp/my_runtime"));
        assert_eq!(
            socket_path(),
            PathBuf::from("/tmp/my_runtime/lexaloud/lexaloud.sock")
        );
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_RUNTIME_DIR", v) };
        } else {
            unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        }
    }
}
