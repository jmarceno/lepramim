use thiserror::Error;

/// Top-level application error.
#[derive(Error, Debug)]
pub enum LexaloudError {
    #[error("config error: {0}")]
    Config(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("daemon not running: {0}")]
    DaemonNotRunning(String),
    #[error("request failed: {0}")]
    RequestFailed(String),
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
    #[error("oversized payload")]
    Oversized,
    #[error("capture failed: {0}")]
    Capture(String),
}

/// Errors arising from configuration loading.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config {path}: {msg}")]
    Read {
        path: String,
        msg: String,
        source: std::io::Error,
    },
    #[error("failed to parse config {path}: {msg}")]
    Parse { path: String, msg: String },
}

/// Errors for CLI command dispatch.
#[derive(Error, Debug)]
pub enum CliError {
    #[error("daemon not running")]
    DaemonDown,
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("request failed: {0}")]
    Request(String),
}

/// Errors for daemon / API layer.
#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("socket error: {0}")]
    Socket(String),
    #[error("bind failed: {0}")]
    Bind(String),
    #[error("internal error: {0}")]
    Internal(String),
}
