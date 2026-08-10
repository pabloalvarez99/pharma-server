//! Google Sign-In wire shapes (ADR-0022) — domain stubs, **no secrets**.
//!
//! Product: mobile feria enters with a Google account. The server will verify
//! the Google `id_token` with Google's public keys and mint our own session
//! JWT. This module only freezes the **request/response shapes** so API and
//! Android stay aligned before OAuth is wired.
//!
//! Out of scope here:
//! - OAuth client id / client secret (ops / `local.properties`, never committed)
//! - Token verification (API crate, when the endpoint lands)
//! - Session cookie / refresh policy

use serde::{Deserialize, Serialize};

/// Client → server when the Android picker returns an `id_token`.
///
/// The server **must** verify the token signature against Google JWKs and
/// check `aud` matches our Android client id. The client secret is never
/// sent by the mobile app (public client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSignInRequest {
    /// Opaque JWT from Google Sign-In / Credential Manager.
    pub id_token: String,
    /// Optional tenant slug if the user already belongs to one business.
    pub tenant: Option<String>,
}

/// Server → client after a successful Google exchange.
///
/// Same shape family as password login (`token` + `expires_in`) so the
/// Android `SessionRepository` can reuse the activation path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSignInResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: i64,
    /// Email from the verified Google claims (display only).
    pub email: Option<String>,
    /// Whether this was the first login for this Google subject on the tenant.
    pub is_new_user: bool,
}

/// Documented path (not implemented yet). Kept as a const so OpenAPI / docs
/// and clients share one string.
pub const GOOGLE_SIGN_IN_PATH: &str = "/api/v1/auth/google";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_stable() {
        assert_eq!(GOOGLE_SIGN_IN_PATH, "/api/v1/auth/google");
    }

    #[test]
    fn request_roundtrips_json() {
        let req = GoogleSignInRequest {
            id_token: "opaque.not.a.real.token".into(),
            tenant: Some("puesto-rosa".into()),
        };
        let s = serde_json::to_string(&req).expect("ser");
        let back: GoogleSignInRequest = serde_json::from_str(&s).expect("de");
        assert_eq!(back.tenant.as_deref(), Some("puesto-rosa"));
        assert!(!back.id_token.is_empty());
    }
}
