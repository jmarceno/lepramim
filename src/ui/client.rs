//! Sync UDS HTTP client for the Iced UI (mirrors the former Qt ApiClient).

use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
pub const MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ApiResult {
    pub status_code: u16,
    pub json: Value,
    pub error: String,
    pub raw_body: Vec<u8>,
}

impl ApiResult {
    pub fn is_success(&self) -> bool {
        self.error.is_empty() && (200..300).contains(&self.status_code)
    }

    pub fn is_daemon_down(&self) -> bool {
        !self.error.is_empty() && self.status_code == 0
    }
}

pub fn default_socket_path() -> std::path::PathBuf {
    crate::config::socket_path()
}

pub fn build_request(method: &str, path: &str, body: &[u8]) -> Vec<u8> {
    let mut req = Vec::with_capacity(256 + body.len());
    req.extend_from_slice(format!("{method} {path} HTTP/1.1\r\n").as_bytes());
    req.extend_from_slice(b"Host: lexaloud\r\nConnection: close\r\n");
    if !body.is_empty() {
        req.extend_from_slice(b"Content-Type: application/json\r\n");
        req.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    } else if method == "POST" {
        req.extend_from_slice(b"Content-Length: 0\r\n");
    }
    req.extend_from_slice(b"\r\n");
    req.extend_from_slice(body);
    req
}

pub fn parse_response(raw: &[u8]) -> ApiResult {
    let mut result = ApiResult {
        status_code: 0,
        json: Value::Null,
        error: String::new(),
        raw_body: Vec::new(),
    };
    if raw.is_empty() {
        result.error = "empty response".into();
        return result;
    }
    if raw.len() > MAX_BODY_BYTES + 8192 {
        result.error = "response body exceeds 256KB limit".into();
        return result;
    }
    let Some(header_end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        result.error = "malformed HTTP response: missing header terminator".into();
        return result;
    };
    let header = &raw[..header_end];
    let body = &raw[header_end + 4..];
    let header_str = String::from_utf8_lossy(header);
    let mut lines = header_str.lines();
    let status_line = lines.next().unwrap_or("");
    let mut parts = status_line.split_whitespace();
    let _http = parts.next();
    let code: u16 = parts.next().and_then(|c| c.parse().ok()).unwrap_or(0);
    if code == 0 {
        result.error = "invalid status code".into();
        return result;
    }
    result.status_code = code;

    let mut content_length: Option<usize> = None;
    for line in lines {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = rest.trim().parse().ok();
        }
    }
    if let Some(cl) = content_length {
        if body.len() < cl {
            result.error = "truncated response body".into();
            result.raw_body = body.to_vec();
            return result;
        }
    }
    if body.len() > MAX_BODY_BYTES {
        result.error = "response body exceeds 256KB limit".into();
        result.raw_body = body[..MAX_BODY_BYTES].to_vec();
        return result;
    }
    result.raw_body = body.to_vec();

    if body.is_empty() {
        return result;
    }
    match serde_json::from_slice(body) {
        Ok(v) => {
            result.json = v;
        }
        Err(e) => {
            if (200..300).contains(&code) {
                result.error = format!("malformed JSON: {e}");
            }
        }
    }
    result
}

pub fn request(method: &str, path: &str, body: &[u8], timeout_ms: u64) -> ApiResult {
    let sock_path = default_socket_path();
    let mut stream = match UnixStream::connect(&sock_path) {
        Ok(s) => s,
        Err(e) => {
            return ApiResult {
                status_code: 0,
                json: Value::Null,
                error: format!("daemon not running: {e}"),
                raw_body: Vec::new(),
            };
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(timeout_ms)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(timeout_ms)));
    let req = build_request(method, path, body);
    if stream.write_all(&req).is_err() {
        return ApiResult {
            status_code: 0,
            json: Value::Null,
            error: "write failed".into(),
            raw_body: Vec::new(),
        };
    }
    let _ = stream.flush();
    let mut raw = Vec::new();
    if stream.read_to_end(&mut raw).is_err() && raw.is_empty() {
        return ApiResult {
            status_code: 0,
            json: Value::Null,
            error: "timeout waiting for daemon response".into(),
            raw_body: Vec::new(),
        };
    }
    parse_response(&raw)
}

pub fn get_state() -> ApiResult {
    request("GET", "/state", &[], DEFAULT_TIMEOUT_MS)
}

pub fn get_healthz() -> ApiResult {
    request("GET", "/healthz", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_toggle() -> ApiResult {
    request("POST", "/toggle", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_stop() -> ApiResult {
    request("POST", "/stop", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_skip() -> ApiResult {
    request("POST", "/skip", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_shutdown() -> ApiResult {
    request("POST", "/shutdown", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_speak(text: &str, mode: &str) -> ApiResult {
    let body = serde_json::json!({ "text": text, "mode": mode });
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    request("POST", "/speak", &bytes, DEFAULT_TIMEOUT_MS)
}

pub fn get_config() -> ApiResult {
    request("GET", "/config", &[], DEFAULT_TIMEOUT_MS)
}

pub fn post_config(cfg: &crate::config::Config) -> ApiResult {
    let body = serde_json::to_value(cfg).unwrap_or(Value::Null);
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    request("POST", "/config", &bytes, DEFAULT_TIMEOUT_MS)
}

pub fn get_models_status() -> ApiResult {
    request("GET", "/models/status", &[], DEFAULT_TIMEOUT_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 62\r\n\r\n{\"state\":\"speaking\",\"current_sentence\":\"hello\",\"pending_count\":1}";
        let r = parse_response(raw);
        assert_eq!(r.status_code, 200);
        assert!(r.error.is_empty());
        assert_eq!(r.json["state"], "speaking");
    }

    #[test]
    fn valid_healthz() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"status\":\"ok\"}";
        let r = parse_response(raw);
        assert_eq!(r.status_code, 200);
        assert!(r.error.is_empty());
    }

    #[test]
    fn truncated_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 62\r\n\r\n{\"state\":";
        let r = parse_response(raw);
        assert!(!r.error.is_empty());
        assert!(r.error.to_ascii_lowercase().contains("truncated"));
    }

    #[test]
    fn malformed() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n{\"bad\"}";
        let r = parse_response(raw);
        assert!(!r.error.is_empty());
    }

    #[test]
    fn build_get_request() {
        let req = build_request("GET", "/state", &[]);
        let s = String::from_utf8_lossy(&req);
        assert!(s.contains("GET /state"));
        assert!(s.contains("Host: lexaloud"));
    }

    #[test]
    fn build_post_request() {
        let body = br#"{"text":"hi"}"#;
        let req = build_request("POST", "/speak", body);
        let s = String::from_utf8_lossy(&req);
        assert!(s.contains("POST /speak"));
        assert!(s.contains("Content-Length:"));
    }

    #[test]
    fn body_too_large() {
        let big = vec![b'x'; 300 * 1024];
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Length: ".to_vec();
        raw.extend_from_slice(big.len().to_string().as_bytes());
        raw.extend_from_slice(b"\r\n\r\n");
        raw.extend_from_slice(&big);
        let r = parse_response(&raw);
        assert!(!r.error.is_empty());
        assert!(r.error.to_ascii_lowercase().contains("256kb"));
    }
}
