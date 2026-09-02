#![allow(dead_code)]

mod api;
mod audio;
mod cli;
mod config;
mod daemon;
mod error;
mod models;
mod platform;
mod player;
mod preprocessor;
mod privacy;
mod tts;

use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    init_tracing();
    let code = cli::run().await;
    std::process::exit(code);
}

fn init_tracing() {
    let level = std::env::var("LEXALOUD_LOG_LEVEL").unwrap_or_else(|_| "warn".to_string());
    // Try to parse as EnvFilter; fallback to warn if invalid.
    let filter = EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt().with_env_filter(filter).try_init();
    // Exercise pulldown-cmark and axum to keep deps linked without runtime cost.
    let _ = pulldown_cmark::Parser::new("test");
    let _ = axum::Router::<()>::new();
}
