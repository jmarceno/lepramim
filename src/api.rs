use axum::{
    Router,
    extract::{Json, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json as JsonResponse},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::audio::CpalSink;
use crate::config::Config;
use crate::player::{Player, PlayerState};
use crate::preprocessor::{PreprocessorConfig, preprocess_with_llm};
use crate::tts::kokoro::KokoroProvider;

pub const MAX_SENTENCE_CHARS: usize = 4096;

// Shared state for Axum handlers
pub struct AppState {
    pub player: Arc<Player<KokoroProvider, CpalSink>>,
    pub config: Arc<Mutex<Config>>,
    pub preproc_config: PreprocessorConfig,
    pub normalizer: Option<Arc<crate::preprocessor::llm::LlmNormalizer>>,
    pub shutdown: tokio::sync::Notify,
}

#[derive(Debug, Deserialize)]
pub struct SpeakRequest {
    pub text: String,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StateResponse {
    pub state: String,
    pub current_sentence: Option<String>,
    pub pending_count: usize,
    pub ready_count: usize,
    pub provider_name: String,
    pub session_providers: Vec<String>,
    pub last_error: Option<String>,
}

fn player_state_to_response(ps: PlayerState) -> StateResponse {
    StateResponse {
        state: ps.state.as_str().to_string(),
        current_sentence: ps.current_sentence,
        pending_count: ps.pending_count,
        ready_count: ps.ready_count,
        provider_name: ps.provider_name,
        session_providers: ps.session_providers,
        last_error: ps.last_error,
    }
}

// Middleware to enforce body size cap
async fn payload_guard(req: Request<axum::body::Body>, next: Next) -> impl IntoResponse {
    if req.uri().path() == "/speak" {
        // Check content-length header
        if let Some(len) = req.headers().get("content-length") {
            if let Ok(s) = len.to_str() {
                if let Ok(n) = s.parse::<usize>() {
                    // Hard cap = capture.max_bytes + 4096 - need config; use 200KB default for now
                    // We'll get config via extension later? For now use 200*1024+4096
                    let cap = 200 * 1024 + 4096;
                    if n > cap {
                        return (
                            StatusCode::PAYLOAD_TOO_LARGE,
                            JsonResponse(serde_json::json!({"detail":"payload too large"})),
                        )
                            .into_response();
                    }
                }
            }
        }
    }
    next.run(req).await
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/state", get(get_state))
        .route("/speak", post(speak))
        .route("/pause", post(pause))
        .route("/resume", post(resume))
        .route("/toggle", post(toggle))
        .route("/stop", post(stop))
        .route("/shutdown", post(shutdown))
        .route("/skip", post(skip))
        .route("/back", post(back))
        .route("/config", get(get_config).post(post_config))
        .route("/models/status", get(models_status))
        .route("/diagnostics", get(diagnostics))
        .layer(middleware::from_fn(payload_guard))
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    JsonResponse(serde_json::json!({"status":"ok"}))
}

async fn shutdown(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.shutdown.notify_waiters();
    JsonResponse(serde_json::json!({"status":"ok"}))
}

async fn get_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}

async fn speak(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SpeakRequest>,
) -> impl IntoResponse {
    // Validate null bytes
    if req.text.contains('\0') {
        return (
            StatusCode::BAD_REQUEST,
            JsonResponse(serde_json::json!({"detail":"text contains null bytes"})),
        )
            .into_response();
    }
    let cfg = state.config.lock().await.clone();
    let max_bytes = cfg.capture.max_bytes;
    if req.text.len() > max_bytes {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            JsonResponse(serde_json::json!({"detail":"text exceeds capture.max_bytes"})),
        )
            .into_response();
    }
    // Validate JSON text not empty after trim? native checks min_length 1 via serde, but also checks no synthesizable sentences after preprocess
    if req.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            JsonResponse(serde_json::json!({"detail":"text is empty"})),
        )
            .into_response();
    }
    let mode = req.mode.unwrap_or_else(|| "replace".to_string());
    if mode != "replace" && mode != "append" {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            JsonResponse(serde_json::json!({"detail":"mode must be replace or append"})),
        )
            .into_response();
    }

    // Preprocess
    let normalizer_ref = state.normalizer.as_deref();
    let sentences = preprocess_with_llm(
        req.text.clone(),
        Some(state.preproc_config.clone()),
        normalizer_ref,
    )
    .await;
    if sentences.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            JsonResponse(serde_json::json!({"detail":"no synthesizable sentences"})),
        )
            .into_response();
    }
    for (i, s) in sentences.iter().enumerate() {
        if s.len() > MAX_SENTENCE_CHARS {
            return (StatusCode::BAD_REQUEST, JsonResponse(serde_json::json!({"detail": format!("sentence {} exceeds MAX_SENTENCE_CHARS ({} > {}); preprocessing failed to segment this input", i, s.len(), MAX_SENTENCE_CHARS)}))).into_response();
        }
    }

    state.player.speak(sentences, &mode).await;
    let ps = state.player.state_snapshot().await;
    (
        StatusCode::OK,
        JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap()),
    )
        .into_response()
}

async fn pause(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.pause().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}
async fn resume(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.resume().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}
async fn toggle(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.toggle().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}
async fn stop(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.stop().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}
async fn skip(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.skip().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}
async fn back(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.player.back().await;
    let ps = state.player.state_snapshot().await;
    JsonResponse(serde_json::to_value(player_state_to_response(ps)).unwrap())
}

async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.config.lock().await.clone();
    JsonResponse(serde_json::to_value(&cfg).unwrap_or(serde_json::Value::Null))
}

async fn post_config(
    State(state): State<Arc<AppState>>,
    Json(new_cfg): Json<Config>,
) -> impl IntoResponse {
    *state.config.lock().await = new_cfg.clone();
    // Optionally persist to file? For now just in-memory.
    // Try to write to config file
    let path = crate::config::config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Serialize to toml
    if let Ok(toml_str) = toml::to_string(&new_cfg) {
        let _ = std::fs::write(&path, toml_str);
    }
    JsonResponse(serde_json::json!({"status":"ok"}))
}

async fn models_status() -> impl IntoResponse {
    let cache = crate::models::default_cache_dir();
    let mut out = serde_json::Map::new();
    for art in crate::models::ARTIFACTS {
        let p = cache.join(art.filename);
        let status = if p.is_file() {
            match std::fs::metadata(&p) {
                Ok(meta) => {
                    serde_json::json!({"present": true, "size": meta.len(), "expected_size": art.expected_size})
                }
                Err(_) => serde_json::json!({"present": true}),
            }
        } else {
            serde_json::json!({"present": false, "expected_size": art.expected_size})
        };
        out.insert(art.filename.to_string(), status);
    }
    JsonResponse(serde_json::Value::Object(out))
}

async fn diagnostics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ps = state.player.state_snapshot().await;
    let cfg = state.config.lock().await.clone();
    let diag = serde_json::json!({
        "state": player_state_to_response(ps),
        "config": cfg,
        "version": env!("CARGO_PKG_VERSION"),
    });
    JsonResponse(diag)
}

// UDS HTTP client helpers for CLI
pub async fn uds_get(path: &str) -> Result<serde_json::Value, String> {
    let sock = crate::config::socket_path();
    let mut stream = tokio::net::UnixStream::connect(&sock)
        .await
        .map_err(|e| format!("daemon not running: {}", e))?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: lexaloud\r\nConnection: close\r\n\r\n",
        path
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let read_fut = stream.read_to_end(&mut buf);
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), read_fut)
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("empty response".to_string());
    }
    let resp = String::from_utf8_lossy(&buf);
    let header_end = resp.find("\r\n\r\n").unwrap_or(0);
    let header = &resp[..header_end];
    let body = &resp[header_end + 4..];
    let status_line = header.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if code >= 400 {
        return Err(format!("daemon returned {}: {}", code, body));
    }
    let v: serde_json::Value = serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

pub async fn uds_post(path: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let sock = crate::config::socket_path();
    let mut stream = tokio::net::UnixStream::connect(&sock)
        .await
        .map_err(|e| format!("daemon not running: {}", e))?;
    let body_str = serde_json::to_string(body).unwrap();
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: lexaloud\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        body_str.len(),
        body_str
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let read_fut = stream.read_to_end(&mut buf);
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), read_fut)
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("empty response".to_string());
    }
    let resp = String::from_utf8_lossy(&buf);
    let header_end = resp.find("\r\n\r\n").unwrap_or(0);
    let header = &resp[..header_end];
    let body = &resp[header_end + 4..];
    let status_line = header.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if code >= 400 {
        if code == 413 {
            return Err("payload too large".to_string());
        }
        return Err(format!("daemon returned {}: {}", code, body));
    }
    let v: serde_json::Value = serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::CpalSink;
    use crate::tts::kokoro::KokoroProvider;

    fn test_state() -> Arc<AppState> {
        let dir = std::env::temp_dir().join(format!("api_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("kokoro.onnx");
        let voices = dir.join("voices.bin");
        std::fs::write(&model, b"dummy").unwrap();
        std::fs::write(&voices, b"dummy").unwrap();
        let provider = KokoroProvider::new(
            model,
            voices,
            "af_heart".to_string(),
            "en-us".to_string(),
            1.0,
            false,
        );
        let sink = CpalSink::new();
        let player = Player::new(provider, sink, 3);
        let cfg = Config::default();
        Arc::new(AppState {
            player,
            config: Arc::new(Mutex::new(cfg)),
            preproc_config: PreprocessorConfig::default(),
            normalizer: None,
            shutdown: tokio::sync::Notify::new(),
        })
    }

    #[tokio::test]
    async fn healthz_ok() {
        let state = test_state();
        let _router = create_router(state);
        // Use axum test via hyper? We'll just test handler directly
        let resp = healthz().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
