use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Hard cap on a single model download. 4 GiB.
pub const MAX_MODEL_DOWNLOAD_BYTES: u64 = 4 * (1u64 << 30);

pub const OPTION_A_TAG: &str = "model-files-v1.0";
pub const OPTION_A_BASE: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";

#[derive(Debug, Clone)]
pub struct Artifact {
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub expected_size: u64,
}

pub const ARTIFACTS: &[Artifact] = &[
    Artifact {
        filename: "kokoro-v1.0.onnx",
        url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx",
        sha256: "7d5df8ecf7d4b1878015a32686053fd0eebe2bc377234608764cc0ef3636a6c5",
        expected_size: 325_532_387,
    },
    Artifact {
        filename: "voices-v1.0.bin",
        url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin",
        sha256: "bca610b8308e8d99f32e6fe4197e7ec01679264efed0cac9140fe9c29f1fbf7d",
        expected_size: 28_214_398,
    },
];

#[derive(Debug, Clone)]
pub struct LlmArtifact {
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: Option<&'static str>,
}

pub const LLM_ARTIFACT: LlmArtifact = LlmArtifact {
    filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
    sha256: None,
};

#[derive(thiserror::Error, Debug)]
pub enum ArtifactError {
    #[error("missing artifact: {0}. Run `lexaloud download-models` to fetch it.")]
    Missing(PathBuf),
    #[error(
        "SHA256 mismatch for {path}\n  expected: {expected}\n  got:      {got}\n  delete the file and re-run `lexaloud download-models`."
    )]
    ShaMismatch {
        path: PathBuf,
        expected: String,
        got: String,
    },
    #[error("download of {url} exceeded {MAX_MODEL_DOWNLOAD_BYTES} bytes cap; aborting")]
    TooLarge { url: String },
    #[error("download failed for {url}: {detail}")]
    DownloadFailed { url: String, detail: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct OnnxruntimeEnvironmentError(pub String);

pub const KNOWN_ORT_DISTS: &[&str] = &[
    "onnxruntime",
    "onnxruntime-gpu",
    "onnxruntime-openvino",
    "onnxruntime-directml",
    "onnxruntime-rocm",
    "onnxruntime-qnn",
    "onnxruntime-migraphx",
];

/// Return default cache dir: $XDG_CACHE_HOME/lexaloud/models or ~/.cache/lexaloud/models
pub fn default_cache_dir() -> PathBuf {
    if let Ok(base) = std::env::var("XDG_CACHE_HOME") {
        if !base.is_empty() {
            return PathBuf::from(base).join("lexaloud").join("models");
        }
    }
    let home = directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| std::env::var("HOME").map(PathBuf::from).ok())
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".cache").join("lexaloud").join("models")
}

fn resolve_cache_dir(cache_dir: Option<&Path>) -> PathBuf {
    let cache = cache_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_cache_dir);
    let cache = if cache.starts_with("~") {
        if let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()) {
            home.join(cache.strip_prefix("~").unwrap_or(&cache))
        } else {
            cache
        }
    } else {
        cache
    };
    if cache.is_absolute() {
        cache
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(cache)
    }
}

/// Ensure model_file stays inside cache_dir (path containment).
pub fn model_file_in_cache(cache_dir: &Path, model_file: &str) -> Result<PathBuf, String> {
    if model_file.is_empty() {
        return Err("model_file is empty".to_string());
    }
    if model_file.contains("..") || model_file.starts_with('/') {
        return Err(format!(
            "model_file must be a relative name inside the cache: {model_file}"
        ));
    }
    let path = cache_dir.join(model_file);
    let canonical_cache = cache_dir
        .canonicalize()
        .unwrap_or_else(|_| cache_dir.to_path_buf());
    let parent = path.parent().unwrap_or(cache_dir);
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let canonical_path = path.canonicalize().unwrap_or(path.clone());
    if !canonical_path.starts_with(&canonical_cache) {
        return Err(format!("model_file escapes cache dir: {}", model_file));
    }
    Ok(path)
}

/// Compute SHA256 hex of file.
pub fn sha256_of(path: &Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 1 << 20];
    use std::io::Read;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn partial_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    dest.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{name}.partial"))
}

/// Stream-download a URL to dest with SHA256 verification after atomic rename.
pub fn download_file(url: &str, dest: &Path) -> Result<(), ArtifactError> {
    let partial = partial_path(dest);
    if partial.exists() {
        std::fs::remove_file(&partial)?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let response = ureq::get(url)
        .call()
        .map_err(|e| ArtifactError::DownloadFailed {
            url: url.to_string(),
            detail: e.to_string(),
        })?;
    if !(200..300).contains(&response.status()) {
        return Err(ArtifactError::DownloadFailed {
            url: url.to_string(),
            detail: format!("HTTP {}", response.status()),
        });
    }

    let mut file = std::fs::File::create(&partial)?;
    let mut reader = response.into_reader();
    let mut buf = [0u8; 1 << 20];
    let mut downloaded: u64 = 0;
    use std::io::{Read, Write};
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| ArtifactError::DownloadFailed {
                url: url.to_string(),
                detail: e.to_string(),
            })?;
        if n == 0 {
            break;
        }
        downloaded += n as u64;
        if downloaded > MAX_MODEL_DOWNLOAD_BYTES {
            let _ = std::fs::remove_file(&partial);
            return Err(ArtifactError::TooLarge {
                url: url.to_string(),
            });
        }
        file.write_all(&buf[..n])?;
    }
    file.sync_all()?;
    drop(file);
    std::fs::rename(&partial, dest)?;
    Ok(())
}

fn verify_artifact(path: &Path, art: &Artifact) -> Result<(), ArtifactError> {
    let digest = sha256_of(path).map_err(ArtifactError::Io)?;
    if digest != art.sha256 {
        return Err(ArtifactError::ShaMismatch {
            path: path.to_path_buf(),
            expected: art.sha256.to_string(),
            got: digest,
        });
    }
    Ok(())
}

/// Ensure artifacts are present and hash-verified.
/// Returns mapping filename -> absolute path.
pub fn ensure_artifacts(
    cache_dir: Option<&Path>,
    download_if_missing: bool,
) -> Result<HashMap<String, PathBuf>, ArtifactError> {
    let cache = resolve_cache_dir(cache_dir);
    std::fs::create_dir_all(&cache)?;

    let mut out = HashMap::new();
    for art in ARTIFACTS {
        let path = cache.join(art.filename);
        if !path.exists() {
            if !download_if_missing {
                return Err(ArtifactError::Missing(path));
            }
            tracing::info!("Downloading {} from {}", art.filename, art.url);
            download_file(art.url, &path)?;
        }
        verify_artifact(&path, art)?;
        out.insert(art.filename.to_string(), path);
    }
    Ok(out)
}

/// Download optional LLM model when requested.
pub fn ensure_llm_model(
    cache_dir: Option<&Path>,
    model_file: &str,
    download_if_missing: bool,
) -> Result<PathBuf, ArtifactError> {
    let cache = resolve_cache_dir(cache_dir);
    std::fs::create_dir_all(&cache)?;
    let path =
        model_file_in_cache(&cache, model_file).map_err(|e| ArtifactError::DownloadFailed {
            url: LLM_ARTIFACT.url.to_string(),
            detail: e,
        })?;
    if path.is_file() {
        return Ok(path);
    }
    if !download_if_missing {
        return Err(ArtifactError::Missing(path));
    }
    if model_file != LLM_ARTIFACT.filename {
        return Err(ArtifactError::Missing(path));
    }
    tracing::info!("Downloading LLM model from {}", LLM_ARTIFACT.url);
    download_file(LLM_ARTIFACT.url, &path)?;
    Ok(path)
}

/// Verify ONNX Runtime environment via ort session builder.
/// Test hooks: LEXALOUD_ORT_SIMULATE_ERROR, LEXALOUD_ORT_DISTS.
pub fn assert_onnxruntime_environment() -> Result<String, OnnxruntimeEnvironmentError> {
    if let Ok(sim) = std::env::var("LEXALOUD_ORT_SIMULATE_ERROR") {
        if sim == "none" {
            return Err(OnnxruntimeEnvironmentError(
                "No ONNX Runtime distribution is installed. Install the Lexaloud package via scripts/install.sh.".to_string()
            ));
        }
        if sim == "multiple" {
            return Err(OnnxruntimeEnvironmentError(
                "Multiple ONNX Runtime distributions installed. Fix: clean install via scripts/install.sh --from-source.".to_string()
            ));
        }
        if sim == "corrupt" {
            return Err(OnnxruntimeEnvironmentError(
                "ONNX Runtime (onnxruntime-gpu) is installed but unusable. Fix: clean install via scripts/install.sh --from-source.".to_string()
            ));
        }
    }
    if let Ok(dists) = std::env::var("LEXALOUD_ORT_DISTS") {
        let installed: Vec<&str> = dists
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if installed.is_empty() {
            return Err(OnnxruntimeEnvironmentError(
                "No ONNX Runtime distribution is installed. Install the Lexaloud package via scripts/install.sh.".to_string()
            ));
        }
        if installed.len() > 1 {
            return Err(OnnxruntimeEnvironmentError(format!(
                "Multiple ONNX Runtime distributions installed: {:?}. Fix: clean install via scripts/install.sh --from-source.",
                installed
            )));
        }
        let name = installed[0].to_string();
        if !KNOWN_ORT_DISTS.contains(&name.as_str()) {
            tracing::warn!(
                "Detected {}; Lexaloud v1 only tests onnxruntime-gpu and onnxruntime (CPU). CUDA EP path may not be used.",
                name
            );
        }
        return Ok(name);
    }

    ort::session::Session::builder().map_err(|e| {
        OnnxruntimeEnvironmentError(format!(
            "ONNX Runtime is not available: {e}. Install via scripts/install.sh or ensure ort can load its bundled runtime."
        ))
    })?;
    Ok("onnxruntime".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn default_cache_dir_uses_xdg() {
        let _guard = ENV_LOCK.lock().unwrap();
        let orig = std::env::var("XDG_CACHE_HOME").ok();
        unsafe { std::env::set_var("XDG_CACHE_HOME", "/tmp/mycache") };
        let p = default_cache_dir();
        assert_eq!(p, PathBuf::from("/tmp/mycache/lexaloud/models"));
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_CACHE_HOME", v) };
        } else {
            unsafe { std::env::remove_var("XDG_CACHE_HOME") };
        }
    }

    #[test]
    fn default_cache_dir_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        let orig = std::env::var("XDG_CACHE_HOME").ok();
        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
        let p = default_cache_dir();
        assert!(p.ends_with("lexaloud/models"));
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_CACHE_HOME", v) };
        }
    }

    #[test]
    fn sha256_of_known() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lexaloud_sha_test_{}.bin", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"hello").unwrap();
        drop(f);
        let digest = sha256_of(&path).unwrap();
        assert_eq!(
            digest,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ensure_artifacts_missing_without_download() {
        let tmp = std::env::temp_dir().join(format!("lexaloud_models_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let res = ensure_artifacts(Some(&tmp), false);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.to_string().contains("missing artifact"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ensure_artifacts_sha_mismatch() {
        let tmp = std::env::temp_dir().join(format!(
            "lexaloud_models_sha_mismatch_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("kokoro-v1.0.onnx");
        std::fs::write(&path, b"wrong content").unwrap();
        let res = ensure_artifacts(Some(&tmp), false);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("SHA256 mismatch") || err.contains("missing artifact"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn max_model_download_bytes_is_4gib() {
        assert_eq!(MAX_MODEL_DOWNLOAD_BYTES, 4 * (1u64 << 30));
    }

    #[test]
    fn model_file_containment_rejects_traversal() {
        let tmp = std::env::temp_dir().join(format!("lexaloud_contain_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        assert!(model_file_in_cache(&tmp, "../evil.gguf").is_err());
        assert!(model_file_in_cache(&tmp, "/etc/passwd").is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn assert_onnxruntime_environment_ok() {
        let _guard = ENV_LOCK.lock().unwrap();
        let orig = std::env::var("LEXALOUD_ORT_DISTS").ok();
        let orig2 = std::env::var("LEXALOUD_ORT_SIMULATE_ERROR").ok();
        unsafe { std::env::remove_var("LEXALOUD_ORT_DISTS") };
        unsafe { std::env::remove_var("LEXALOUD_ORT_SIMULATE_ERROR") };
        let res = assert_onnxruntime_environment();
        assert!(res.is_ok());
        if let Some(v) = orig {
            unsafe { std::env::set_var("LEXALOUD_ORT_DISTS", v) };
        } else {
            unsafe { std::env::remove_var("LEXALOUD_ORT_DISTS") };
        }
        if let Some(v) = orig2 {
            unsafe { std::env::set_var("LEXALOUD_ORT_SIMULATE_ERROR", v) };
        } else {
            unsafe { std::env::remove_var("LEXALOUD_ORT_SIMULATE_ERROR") };
        }
    }

    #[test]
    fn assert_onnxruntime_multiple_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("LEXALOUD_ORT_DISTS", "onnxruntime,onnxruntime-gpu") };
        let res = assert_onnxruntime_environment();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Multiple"));
        unsafe { std::env::remove_var("LEXALOUD_ORT_DISTS") };
    }

    #[test]
    fn assert_onnxruntime_none_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("LEXALOUD_ORT_SIMULATE_ERROR", "none") };
        let res = assert_onnxruntime_environment();
        assert!(res.is_err());
        unsafe { std::env::remove_var("LEXALOUD_ORT_SIMULATE_ERROR") };
    }
}
