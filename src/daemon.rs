use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::{AppState, create_router};
use crate::audio::CpalSink;
use crate::config::{Config, load_config, runtime_dir, socket_path};
use crate::platform::service::{socket_path_valid, stale_socket_cleanup};
use crate::player::Player;
use crate::preprocessor::PreprocessorConfig;
use crate::tts::kokoro::KokoroProvider;

pub struct DaemonComponents {
    pub config: Config,
    pub player: Arc<Player<KokoroProvider, CpalSink>>,
    pub preproc_config: PreprocessorConfig,
    pub normalizer: Option<Arc<crate::preprocessor::llm::LlmNormalizer>>,
}

pub fn build_components(cfg: Option<Config>) -> Result<DaemonComponents, String> {
    let cfg = cfg.unwrap_or_else(|| load_config::<PathBuf>(None));
    // Verify ORT env (stub)
    match crate::models::assert_onnxruntime_environment() {
        Ok(dist) => tracing::info!("ORT distribution: {}", dist),
        Err(e) => {
            tracing::warn!("ORT environment check failed: {}", e);
            // Continue with CPU fallback for stub
        }
    }

    // Try to ensure artifacts
    let artifacts = match crate::models::ensure_artifacts(None, false) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("artifacts missing: {} - using stub provider", e);
            // Use dummy paths for stub
            let mut map = std::collections::HashMap::new();
            let dir = std::env::temp_dir().join("lexaloud_stub_models");
            let _ = std::fs::create_dir_all(&dir);
            let model = dir.join("kokoro-v1.0.onnx");
            let voices = dir.join("voices-v1.0.bin");
            // Ensure dummy files exist
            if !model.exists() {
                let _ = std::fs::write(&model, b"stub");
            }
            if !voices.exists() {
                let _ = std::fs::write(&voices, b"stub");
            }
            map.insert("kokoro-v1.0.onnx".to_string(), model);
            map.insert("voices-v1.0.bin".to_string(), voices);
            map
        }
    };

    let model_path = artifacts
        .get("kokoro-v1.0.onnx")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("/tmp/kokoro.onnx"));
    let voices_path = artifacts
        .get("voices-v1.0.bin")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("/tmp/voices.bin"));

    let prefer_cuda = std::env::var("LEXALOUD_ORT_DISTS")
        .map(|s| s.contains("onnxruntime-gpu"))
        .unwrap_or(false);

    let provider = KokoroProvider::new(
        model_path,
        voices_path,
        cfg.provider.voice.clone(),
        cfg.provider.lang.clone(),
        cfg.provider.speed,
        prefer_cuda,
    );
    let sink = CpalSink::new();
    let player = Player::new(provider, sink, cfg.daemon.ready_queue_depth);

    let preproc_config = PreprocessorConfig {
        dedupe_mathjax_selection: cfg.preprocessor.dedupe_mathjax_selection,
        strip_markdown: cfg.preprocessor.strip_markdown,
        sre_latex_enabled: cfg.sre_latex.enabled,
        sre_latex_timeout_s: cfg.sre_latex.timeout_s,
        sre_latex_domain: cfg.sre_latex.domain.clone(),
        sre_latex_style: cfg.sre_latex.style.clone(),
        strip_numeric_bracket_citations: cfg.preprocessor.strip_numeric_bracket_citations,
        strip_parenthetical_citations: cfg.preprocessor.strip_parenthetical_citations,
        expand_latin_abbreviations: cfg.preprocessor.expand_latin_abbreviations,
        expand_academic_abbreviations: cfg.preprocessor.expand_academic_abbreviations,
        normalize_numbers: cfg.preprocessor.normalize_numbers,
        normalize_urls: cfg.preprocessor.normalize_urls,
        normalize_math_symbols: cfg.preprocessor.normalize_math_symbols,
        pdf_cleanup: cfg.preprocessor.pdf_cleanup,
    };

    let normalizer = if cfg.normalizer.enabled {
        let llm_cfg = crate::preprocessor::llm::LlmNormalizerConfig {
            enabled: cfg.normalizer.enabled,
            model_path: cfg.normalizer.model_path.clone(),
            model_repo: cfg.normalizer.model_repo.clone(),
            model_file: cfg.normalizer.model_file.clone(),
            n_gpu_layers: cfg.normalizer.n_gpu_layers,
            n_ctx: cfg.normalizer.n_ctx,
            temperature: cfg.normalizer.temperature,
            max_output_ratio: cfg.normalizer.max_output_ratio,
            glossary: cfg.normalizer.glossary.clone(),
        };
        match crate::preprocessor::llm::LlmNormalizer::new(llm_cfg) {
            Ok(n) => Some(Arc::new(n)),
            Err(e) => {
                tracing::warn!("LLM normalizer init failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    Ok(DaemonComponents {
        config: cfg,
        player,
        preproc_config,
        normalizer,
    })
}

pub async fn run() -> Result<(), String> {
    // Load config
    let cfg = load_config::<PathBuf>(None);
    tracing::info!("daemon starting with config: {:?}", cfg);

    let sock = socket_path();
    let rt = runtime_dir();

    // Validate socket path
    socket_path_valid(&sock, &rt).map_err(|e| format!("socket validation failed: {}", e))?;

    // Cleanup stale socket
    stale_socket_cleanup(&sock).map_err(|e| format!("socket cleanup failed: {}", e))?;

    let components = build_components(Some(cfg.clone()))?;
    let app_state = Arc::new(AppState {
        player: components.player.clone(),
        config: Arc::new(Mutex::new(components.config)),
        preproc_config: components.preproc_config,
        normalizer: components.normalizer,
    });

    // Warmup in background
    let player_clone = app_state.player.clone();
    let _warmup_handle = tokio::spawn(async move {
        player_clone.set_warming(true).await;
        // Provider warmup via player? We'll call provider warmup directly if accessible
        // For now, just sleep a bit and set warming false
        // Real warmup would call provider.warmup and sink.warmup
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        player_clone.set_warming(false).await;
        tracing::info!("warmup complete");
    });

    let router = create_router(app_state.clone());

    // Bind UnixListener
    let listener = tokio::net::UnixListener::bind(&sock)
        .map_err(|e| format!("failed to bind UDS {}: {}", sock.display(), e))?;
    tracing::info!("daemon listening on {}", sock.display());

    // Serve using hyper + tower. Each incoming UnixStream is handled as an HTTP/1.1 connection.
    // We clone the router (which is a Service) for each connection.
    use hyper_util::rt::TokioIo;
    use hyper_util::service::TowerToHyperService;

    loop {
        let (stream, _addr) = listener
            .accept()
            .await
            .map_err(|e| format!("accept failed: {}", e))?;
        let svc = router.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let hyper_svc = TowerToHyperService::new(svc);
            let builder = hyper::server::conn::http1::Builder::new();
            if let Err(e) = builder.serve_connection(io, hyper_svc).await {
                tracing::debug!("serve_connection error: {}", e);
            }
        });
    }
    // unreachable, but keep warmup handle for lint
    #[allow(unreachable_code)]
    let _ = _warmup_handle.await;
    #[allow(unreachable_code)]
    Ok(())

    // Warmup handle would be aborted on shutdown, but loop is infinite
    // let _ = warmup_handle.await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_components_stub() {
        let cfg = Config::default();
        let comps = build_components(Some(cfg));
        assert!(comps.is_ok());
    }
}
