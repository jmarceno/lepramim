use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::audio::AudioChunk;
use crate::player::SpeechProvider;

/// Kokoro-82M provider stub.
/// In real build, would use `ort` crate for ONNX Runtime and `kokoro-onnx` bindings.
/// Here we verify artifacts, check provider, and return fake audio for tests.
pub struct KokoroProvider {
    pub model_path: PathBuf,
    pub voices_path: PathBuf,
    pub voice: String,
    pub lang: String,
    pub speed: f64,
    pub prefer_cuda: bool,
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    warmed: bool,
    session_providers: Vec<String>,
}

impl KokoroProvider {
    pub fn new(
        model_path: impl AsRef<Path>,
        voices_path: impl AsRef<Path>,
        voice: String,
        lang: String,
        speed: f64,
        prefer_cuda: bool,
    ) -> Self {
        Self {
            model_path: model_path.as_ref().to_path_buf(),
            voices_path: voices_path.as_ref().to_path_buf(),
            voice,
            lang,
            speed,
            prefer_cuda,
            inner: Arc::new(Mutex::new(Inner {
                warmed: false,
                session_providers: Vec::new(),
            })),
        }
    }

    async fn ensure_initialized(&self) -> Result<Vec<String>, String> {
        let mut inner = self.inner.lock().await;
        if !inner.warmed && inner.session_providers.is_empty() {
            // Simulate session construction
            // Check files exist?
            if !self.model_path.is_file() {
                return Err(format!("model not found: {}", self.model_path.display()));
            }
            if !self.voices_path.is_file() {
                return Err(format!("voices not found: {}", self.voices_path.display()));
            }
            // Decide providers: if prefer_cuda, try CUDA, else CPU
            let providers = if self.prefer_cuda {
                // Check if CUDA is actually available via env var simulation
                if std::env::var("LEXALOUD_SIMULATE_CUDA_AVAILABLE").as_deref() == Ok("1") {
                    vec![
                        "CUDAExecutionProvider".to_string(),
                        "CPUExecutionProvider".to_string(),
                    ]
                } else {
                    // Simulate silent fallback detection: if prefer_cuda but CUDA not available, still fallback to CPU and log error
                    tracing::error!(
                        "Requested CUDAExecutionProvider but session reports [\"CPUExecutionProvider\"]; continuing on CPU"
                    );
                    vec!["CPUExecutionProvider".to_string()]
                }
            } else {
                vec!["CPUExecutionProvider".to_string()]
            };
            inner.session_providers = providers.clone();
            tracing::info!("Kokoro session providers: {:?}", providers);
            return Ok(providers);
        }
        Ok(inner.session_providers.clone())
    }

    /// Verify that CUDA was actually used when prefer_cuda is true.
    pub fn verify_providers(&self, providers: &[String]) -> Result<(), String> {
        if self.prefer_cuda && !providers.contains(&"CUDAExecutionProvider".to_string()) {
            return Err(format!(
                "Requested CUDAExecutionProvider but session reports {:?}. Likely cause: CUDA not available. Continuing on CPU.",
                providers
            ));
        }
        Ok(())
    }
}

impl SpeechProvider for KokoroProvider {
    fn name(&self) -> &str {
        "kokoro"
    }
    fn session_providers(&self) -> Vec<String> {
        // Try to get from inner without blocking? For snapshot we return empty if not init
        // We can't block in sync method, so return empty; daemon will query via async if needed.
        // For tests, we return prefer_cuda derived.
        if self.prefer_cuda {
            tracing::info!("prefer_cuda enabled, requesting CUDAExecutionProvider");
            vec![
                "CUDAExecutionProvider".to_string(),
                "CPUExecutionProvider".to_string(),
            ]
        } else {
            vec!["CPUExecutionProvider".to_string()]
        }
    }

    async fn synthesize(
        &self,
        sentence: String,
        job_id: u64,
        is_current: Arc<dyn Fn(u64) -> bool + Send + Sync>,
    ) -> Option<AudioChunk> {
        if !is_current(job_id) {
            return None;
        }
        let providers = match self.ensure_initialized().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Kokoro init failed: {}", e);
                return None;
            }
        };
        if let Err(e) = self.verify_providers(&providers) {
            tracing::warn!("{}", e);
            // Still continue on CPU
        }
        if !is_current(job_id) {
            return None;
        }
        // Simulate synthesis: generate sine wave
        let sample_rate = 24000u32;
        let duration = 0.3; // seconds per sentence stub, scaled by speed
        let speed = self.speed.clamp(0.5, 2.0);
        let n = (duration * sample_rate as f64 / speed) as usize;
        // Filter very short outputs like native does: <50ms dropped
        let min_samples = (0.05 * sample_rate as f64) as usize;
        if n < min_samples {
            tracing::debug!(
                "Kokoro returned very short output ({} samples); dropping",
                n
            );
            return None;
        }
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / sample_rate as f64;
            let v = 0.1 * (2.0 * std::f64::consts::PI * 440.0 * t).sin();
            samples.push(v as f32);
        }
        if !is_current(job_id) {
            tracing::debug!(
                "Kokoro synthesis discarded: job {} no longer current",
                job_id
            );
            return None;
        }
        let mut chunk = AudioChunk::new(samples, sample_rate);
        chunk.metadata.insert("sentence".to_string(), sentence);
        chunk
            .metadata
            .insert("voice".to_string(), self.voice.clone());
        Some(chunk)
    }

    async fn warmup(&self) {
        let mut inner = self.inner.lock().await;
        if inner.warmed {
            return;
        }
        // Simulate warmup synthesis
        if let Err(e) = self.ensure_initialized().await {
            tracing::error!("Kokoro warmup failed: {}", e);
            return;
        }
        // Fake create call
        tracing::info!(
            "Kokoro warmup complete (providers={:?})",
            inner.session_providers
        );
        inner.warmed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::SpeechProvider;
    use std::io::Write;

    fn temp_model() -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kokoro_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("kokoro-v1.0.onnx");
        let voices = dir.join("voices-v1.0.bin");
        std::fs::File::create(&model)
            .unwrap()
            .write_all(b"dummy")
            .unwrap();
        std::fs::File::create(&voices)
            .unwrap()
            .write_all(b"dummy")
            .unwrap();
        (dir, model, voices)
    }

    #[tokio::test]
    async fn synthesize_fake_audio() {
        let (dir, model, voices) = temp_model();
        let p = KokoroProvider::new(
            model,
            voices,
            "af_heart".to_string(),
            "en-us".to_string(),
            1.0,
            false,
        );
        let is_current = Arc::new(|_: u64| true);
        let chunk = p.synthesize("Hello world".to_string(), 1, is_current).await;
        assert!(chunk.is_some());
        let c = chunk.unwrap();
        assert_eq!(c.sample_rate, 24000);
        assert!(!c.samples.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn verify_cuda_fails_loudly() {
        let (dir, model, voices) = temp_model();
        let p = KokoroProvider::new(
            model,
            voices,
            "af_heart".to_string(),
            "en-us".to_string(),
            1.0,
            true,
        );
        let providers = vec!["CPUExecutionProvider".to_string()];
        let res = p.verify_providers(&providers);
        assert!(res.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
