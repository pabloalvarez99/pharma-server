//! In-memory session state.
//!
//! The JWT lives ONLY here (in memory). It is never written to disk — losing
//! it on quit is intentional (re-login each launch, LoL-style). The token is
//! wrapped in a [`SecretString`] so it is zeroed on drop and never leaked
//! through `Debug` output.

use std::sync::Mutex;

use secrecy::SecretString;
use tauri::State;

/// In-memory session. Token is intentionally NOT persisted to disk.
///
/// Uses a `std::sync::Mutex` (not `tokio::sync::Mutex`) on purpose: the guard
/// is never held across an `.await` — every command clones the token (or
/// replaces/clears the slot) and drops the guard immediately.
#[derive(Default)]
pub struct SessionState {
    inner: Mutex<Option<Session>>,
}

#[derive(Clone)]
struct Session {
    token: SecretString,
}

/// Pull the in-memory JWT or return the Spanish "no session" error. Shared by
/// every authenticated command (license_status, list_products, reports, POS).
///
/// Returns the cloned [`SecretString`]; callers pass `token.expose_secret()` to
/// `bearer_auth` so the secret only exists as a `&str` for the duration of the
/// request build.
pub fn token_of(state: &State<'_, SessionState>) -> Result<SecretString, String> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())?;
    guard
        .as_ref()
        .map(|s| s.token.clone())
        .ok_or_else(|| "No hay sesión activa. Inicia sesión primero.".to_string())
}

/// Store a freshly-issued token (login / first-run setup).
pub fn store_token(state: &State<'_, SessionState>, token: String) -> Result<(), String> {
    *state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())? = Some(Session {
        token: SecretString::from(token),
    });
    Ok(())
}

/// Forget the in-memory session (logout). The `SecretString` inside is zeroed
/// when the dropped `Session` is freed.
pub fn clear(state: &State<'_, SessionState>) -> Result<(), String> {
    *state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())? = None;
    Ok(())
}
