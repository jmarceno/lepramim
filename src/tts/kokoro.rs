use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider, ExecutionProvider};
use ort::session::Session;
use ort::value::Tensor;
use tokio::sync::Mutex;

use crate::audio::AudioChunk;
use crate::player::SpeechProvider;
use crate::tts::phonemes::{self, SAMPLE_RATE, normalize_ipa, split_at_token_cap, tokenize};
use crate::tts::phonemize::phonemize;
use crate::tts::voices::{VoiceBank, load_all_voices, select_voice_bank, style_row};

/// Kokoro-82M provider via ONNX Runtime (ort 2.x).
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
    session: Option<Arc<StdMutex<Session>>>,
    voices: HashMap<String, VoiceBank>,
    session_providers: Vec<String>,
    warmed: bool,
    tokens_input: String,
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
                session: None,
                voices: HashMap::new(),
                session_providers: Vec::new(),
                warmed: false,
                tokens_input: "input_ids".to_string(),
            })),
        }
    }

    fn build_session(model_path: &Path, try_cuda: bool) -> Result<(Session, Vec<String>), String> {
        if try_cuda {
            let cuda_ok = CUDAExecutionProvider::default()
                .is_available()
                .unwrap_or(false);
            if cuda_ok {
                let cuda_session = (|| {
                    let builder = Session::builder().map_err(|e| e.to_string())?;
                    let builder = builder
                        .with_execution_providers([
                            CUDAExecutionProvider::default().build(),
                            CPUExecutionProvider::default().build(),
                        ])
                        .map_err(|e| e.to_string())?;
                    builder
                        .commit_from_file(model_path)
                        .map_err(|e| format!("ORT session load failed: {e}"))
                })();
                match cuda_session {
                    Ok(session) => {
                        tracing::info!("Kokoro session using CUDA");
                        return Ok((
                            session,
                            vec![
                                "CUDAExecutionProvider".to_string(),
                                "CPUExecutionProvider".to_string(),
                            ],
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("CUDA session failed ({e}); falling back to CPU");
                    }
                }
            } else {
                tracing::info!("CUDA not available; using CPU");
            }
        }

        let cpu_threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(1, 8))
            .unwrap_or(4);
        let builder = Session::builder().map_err(|e| e.to_string())?;
        let builder = builder
            .with_intra_threads(cpu_threads)
            .map_err(|e| e.to_string())?
            .with_execution_providers([CPUExecutionProvider::default().build()])
            .map_err(|e| e.to_string())?;
        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| format!("ORT session load failed: {e}"))?;
        Ok((session, vec!["CPUExecutionProvider".to_string()]))
    }

    async fn ensure_initialized(&self) -> Result<Vec<String>, String> {
        let mut inner = self.inner.lock().await;
        if inner.session.is_some() {
            return Ok(inner.session_providers.clone());
        }
        if !self.model_path.is_file() {
            return Err(format!("model not found: {}", self.model_path.display()));
        }
        if !self.voices_path.is_file() {
            return Err(format!("voices not found: {}", self.voices_path.display()));
        }
        let model_path = self.model_path.clone();
        let voices_path = self.voices_path.clone();
        let try_cuda = self.prefer_cuda;
        let (session, providers) =
            tokio::task::spawn_blocking(move || Self::build_session(&model_path, try_cuda))
                .await
                .map_err(|e| e.to_string())??;

        let voices = tokio::task::spawn_blocking(move || load_all_voices(&voices_path))
            .await
            .map_err(|e| e.to_string())??;

        let tokens_input = if session.inputs.iter().any(|i| i.name == "input_ids") {
            "input_ids".to_string()
        } else {
            "tokens".to_string()
        };

        inner.session = Some(Arc::new(StdMutex::new(session)));
        inner.voices = voices;
        inner.session_providers = providers.clone();
        inner.tokens_input = tokens_input;
        tracing::info!("Kokoro session providers: {:?}", providers);
        Ok(providers)
    }

    fn infer_sync(
        session: &mut Session,
        tokens_input: &str,
        phonemes: &str,
        bank: &VoiceBank,
        speed: f64,
    ) -> Result<Vec<f32>, String> {
        let vocab = phonemes::kokoro_vocab();
        let tokens = tokenize(phonemes, vocab);
        let n_phoneme_tokens = tokens.len().saturating_sub(2);
        if n_phoneme_tokens == 0 {
            return Err("no phonemes in vocabulary".to_string());
        }
        let style = style_row(bank, n_phoneme_tokens)?;
        let n = tokens.len();
        let input_ids = Tensor::from_array((vec![1_i64, n as i64], tokens))
            .map_err(|e| format!("input_ids tensor: {e}"))?;
        let style_t = Tensor::from_array((vec![1_i64, phonemes::STYLE_DIM as i64], style))
            .map_err(|e| format!("style tensor: {e}"))?;
        let speed_t = Tensor::from_array((vec![1_i64], vec![speed as f32]))
            .map_err(|e| format!("speed tensor: {e}"))?;

        let outputs = match tokens_input {
            "tokens" => session
                .run(ort::inputs![
                    "tokens" => input_ids,
                    "style" => style_t,
                    "speed" => speed_t,
                ])
                .map_err(|e| e.to_string())?,
            _ => session
                .run(ort::inputs![
                    "input_ids" => input_ids,
                    "style" => style_t,
                    "speed" => speed_t,
                ])
                .map_err(|e| e.to_string())?,
        };
        let (_shape, pcm) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extract audio: {e}"))?;
        Ok(pcm.to_vec())
    }
}

impl SpeechProvider for KokoroProvider {
    fn name(&self) -> &str {
        "kokoro"
    }

    fn session_providers(&self) -> Vec<String> {
        if let Ok(inner) = self.inner.try_lock() {
            return inner.session_providers.clone();
        }
        Vec::new()
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
        tracing::debug!("Kokoro providers={:?}", providers);
        if !is_current(job_id) {
            return None;
        }

        let raw = match phonemize(&sentence, &self.lang) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("phonemize failed: {}", e);
                return None;
            }
        };
        let phonemes = normalize_ipa(&raw);
        let speed = self.speed.clamp(0.5, 2.0);
        let voice = self.voice.clone();
        let inner = self.inner.clone();

        let pcm = tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            let session_mtx = guard.session.as_ref().ok_or("session not init")?;
            let mut session = session_mtx.lock().map_err(|e| e.to_string())?;
            let bank = select_voice_bank(&guard.voices, &voice)
                .ok_or("voice not found")?
                .clone();
            let tokens_input = guard.tokens_input.clone();
            let vocab = phonemes::kokoro_vocab();
            let mut all_samples = Vec::new();
            let mut remaining = phonemes.as_str();
            while !remaining.is_empty() {
                let (head, tail) = split_at_token_cap(remaining, vocab);
                if !head.trim().is_empty() {
                    let chunk = Self::infer_sync(&mut session, &tokens_input, head, &bank, speed)?;
                    all_samples.extend(chunk);
                }
                remaining = tail.trim_start();
            }
            Ok::<_, String>(all_samples)
        })
        .await
        .ok()?
        .ok()?;

        if !is_current(job_id) {
            return None;
        }
        let min_samples = (0.05 * SAMPLE_RATE as f64) as usize;
        if pcm.len() < min_samples {
            tracing::debug!(
                "Kokoro returned very short output ({} samples); dropping",
                pcm.len()
            );
            return None;
        }
        let mut chunk = AudioChunk::new(pcm, SAMPLE_RATE);
        chunk.metadata.insert("sentence".to_string(), sentence);
        chunk
            .metadata
            .insert("voice".to_string(), self.voice.clone());
        Some(chunk)
    }

    async fn warmup(&self) {
        {
            let inner = self.inner.lock().await;
            if inner.warmed {
                return;
            }
        }
        match self.ensure_initialized().await {
            Ok(providers) => {
                tracing::info!("Kokoro session loaded (providers={:?})", providers);
            }
            Err(e) => {
                tracing::error!("Kokoro init failed during warmup: {}", e);
                return;
            }
        }
        let is_current = Arc::new(|_: u64| true);
        let chunk = self.synthesize("Ready.".to_string(), 0, is_current).await;
        tracing::info!(
            "Kokoro warmup {}",
            if chunk.is_some() { "ok" } else { "failed" }
        );
        let mut inner = self.inner.lock().await;
        inner.warmed = true;
        tracing::info!(
            "Kokoro warmup complete (providers={:?})",
            inner.session_providers
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_tts_smoke_opt_in() {
        if std::env::var("LEPRAMIM_REAL_TTS").ok().as_deref() != Some("1") {
            return;
        }
        let cache = crate::models::default_cache_dir();
        let model = cache.join("kokoro-v1.0.onnx");
        let voices = cache.join("voices-v1.0.bin");
        if !model.is_file() || !voices.is_file() {
            eprintln!("skip real TTS smoke: models missing in {}", cache.display());
            return;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let provider = KokoroProvider::new(
                model,
                voices,
                "af_heart".to_string(),
                "en-us".to_string(),
                1.0,
                false,
            );
            let is_current = Arc::new(|_: u64| true);
            let chunk = provider
                .synthesize("Hello world.".to_string(), 1, is_current)
                .await
                .expect("synthesis failed");
            assert_eq!(chunk.sample_rate, SAMPLE_RATE);
            assert!(chunk.samples.len() > SAMPLE_RATE as usize / 10);
            let peak = chunk
                .samples
                .iter()
                .map(|s| s.abs())
                .fold(0.0_f32, f32::max);
            assert!(peak > 0.001, "peak too low: {peak}");
        });
    }
}
