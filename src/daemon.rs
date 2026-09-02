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

    let ort_dist = crate::models::assert_onnxruntime_environment()
        .map_err(|e| format!("ONNX Runtime check failed: {}", e.0))?;
    tracing::info!("ORT distribution: {}", ort_dist);

    let prefer_cuda = (ort_dist.contains("gpu")
        || std::env::var("LEXALOUD_ORT_DISTS")
            .map(|s| s.contains("onnxruntime-gpu"))
            .unwrap_or(false))
        && std::env::var("LEXALOUD_PREFER_CUDA")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    let artifacts = crate::models::ensure_artifacts(None, false).map_err(|e| {
        format!("{e}. Models download automatically when you open the Lexaloud AppImage.")
    })?;

    let model_path = artifacts
        .get("kokoro-v1.0.onnx")
        .cloned()
        .ok_or_else(|| "kokoro-v1.0.onnx missing from artifact map".to_string())?;
    let voices_path = artifacts
        .get("voices-v1.0.bin")
        .cloned()
        .ok_or_else(|| "voices-v1.0.bin missing from artifact map".to_string())?;

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
    let cfg = load_config::<PathBuf>(None);
    tracing::info!("daemon starting with config: {:?}", cfg);

    let sock = socket_path();
    let rt = runtime_dir();

    socket_path_valid(&sock, &rt).map_err(|e| format!("socket validation failed: {}", e))?;
    stale_socket_cleanup(&sock).map_err(|e| format!("socket cleanup failed: {}", e))?;

    let components = build_components(Some(cfg.clone()))?;
    let app_state = Arc::new(AppState {
        player: components.player.clone(),
        config: Arc::new(Mutex::new(components.config)),
        preproc_config: components.preproc_config,
        normalizer: components.normalizer,
        shutdown: tokio::sync::Notify::new(),
    });

    let player_for_mpris = app_state.player.clone();
    tokio::spawn(async move {
        let _mpris = crate::platform::mpris::wire_mpris(player_for_mpris).await;
    });

    let player_clone = app_state.player.clone();
    tokio::spawn(async move {
        player_clone.set_warming(true).await;
        player_clone.run_warmup().await;
        player_clone.set_warming(false).await;
        tracing::info!("warmup complete");
    });

    let router = create_router(app_state.clone());

    let listener = tokio::net::UnixListener::bind(&sock)
        .map_err(|e| format!("failed to bind UDS {}: {}", sock.display(), e))?;
    tracing::info!("daemon listening on {}", sock.display());

    use hyper_util::rt::TokioIo;
    use hyper_util::service::TowerToHyperService;

    loop {
        tokio::select! {
            _ = app_state.shutdown.notified() => {
                tracing::info!("daemon shutdown requested");
                break;
            }
            accepted = listener.accept() => {
                let (stream, _addr) = accepted.map_err(|e| format!("accept failed: {}", e))?;
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
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_components_missing_models_fails() {
        let _guard = ENV_LOCK.lock().unwrap();
        let orig = std::env::var("XDG_CACHE_HOME").ok();
        let tmp = std::env::temp_dir().join(format!("lexaloud_daemon_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        unsafe { std::env::set_var("XDG_CACHE_HOME", tmp.to_string_lossy().as_ref()) };
        let cfg = Config::default();
        let res = build_components(Some(cfg));
        assert!(res.is_err());
        let err = match res {
            Err(e) => e,
            Ok(_) => panic!("expected error without models"),
        };
        assert!(
            err.contains("missing artifact") || err.contains("Models download automatically"),
            "unexpected error: {err}"
        );
        if let Some(v) = orig {
            unsafe { std::env::set_var("XDG_CACHE_HOME", v) };
        } else {
            unsafe { std::env::remove_var("XDG_CACHE_HOME") };
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
