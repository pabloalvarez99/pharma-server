//! Shared HTTP machinery for every command.
//!
//! One `reqwest::Client` per process (connection + TLS session reuse across
//! commands) with hard timeouts so a hung server can never park the UI
//! forever. `health_client` uses a shorter timeout for the login-screen
//! reachability probe.

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;

/// Server error envelope (`crates/api/src/error.rs`):
/// `{ "error": { "code", "message", "details"? } }`.
#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

/// Max total time for a regular API call (LAN server; generous for big
/// exports/imports). A timed-out request maps to the friendly connect copy
/// via [`conn_error`].
const API_TIMEOUT: Duration = Duration::from_secs(30);
/// Max time to establish the TCP/TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// The health probe should feel instant on the login screen.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static HEALTH_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// The shared client (30s total / 5s connect timeout). Initialized on first
/// use; panics only if the TLS backend itself fails to build, which is a
/// fatal environment problem, not a runtime condition.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(API_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("no se pudo inicializar el cliente HTTP")
    })
}

/// Short-timeout client for `server_health` — the login screen polls it and a
/// dead server must surface fast, not after 30s.
pub fn health_client() -> &'static reqwest::Client {
    HEALTH_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("no se pudo inicializar el cliente HTTP de salud")
    })
}

/// Trim a trailing slash so `server_url` + "/path" never doubles up.
pub fn base(server_url: &str) -> &str {
    server_url.trim_end_matches('/')
}

/// Map a non-2xx response to a Spanish message, parsing the server envelope when
/// present. Falls back to a status-based message for non-JSON bodies.
pub async fn error_message(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&body) {
        return env.error.message;
    }
    match status.as_u16() {
        401 => "Credenciales inválidas.".to_string(),
        403 => "Permiso denegado para esta operación.".to_string(),
        404 => "Recurso no encontrado en el servidor.".to_string(),
        503 => "Servicio no disponible. Intenta nuevamente.".to_string(),
        other => format!("Error del servidor ({other})."),
    }
}

/// Connection-level failures (server down, wrong URL) → friendly Spanish copy.
pub fn conn_error(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        "No se pudo conectar al servidor. Verifica la URL y que pharma-server esté corriendo."
            .to_string()
    } else {
        format!("Error de red: {e}")
    }
}

/// Like [`error_message`] but preserves the server `code` so the frontend can
/// branch on it (e.g. `INSUFFICIENT_STOCK`). Encodes as `"CODE|message"`; when
/// no envelope is present the code half is empty (`"|message"`). The JS side
/// splits on the first `|`.
pub async fn coded_error(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&body) {
        return format!("{}|{}", env.error.code, env.error.message);
    }
    let msg = match status.as_u16() {
        401 => "Credenciales inválidas.".to_string(),
        403 => "Permiso denegado para esta operación.".to_string(),
        404 => "Recurso no encontrado en el servidor.".to_string(),
        422 => "No se pudo procesar la venta.".to_string(),
        503 => "Servicio no disponible. Intenta nuevamente.".to_string(),
        other => format!("Error del servidor ({other})."),
    };
    format!("|{msg}")
}
