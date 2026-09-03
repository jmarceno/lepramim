pub mod api;
pub mod audio;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod models;
pub mod platform;
pub mod player;
pub mod preprocessor;
pub mod privacy;
pub mod single_instance;
pub mod tts;
pub mod ui;

pub fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let level = std::env::var("LEPRAMIM_LOG_LEVEL").unwrap_or_else(|_| "warn".to_string());
    let filter = EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt().with_env_filter(filter).try_init();
    let _ = pulldown_cmark::Parser::new("test");
    let _ = axum::Router::<()>::new();
}
