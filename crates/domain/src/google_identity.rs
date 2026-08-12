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
/// check `aud` matches our **Web** client id — not the Android one. Android
/// asks Credential Manager for a token *addressed to* the Web client
/// (`setServerClientId`), because that is the audience the server can name
/// without shipping a secret. Passing the Android client id there compiles,
/// opens the picker, and then fails every verification server-side. The client
/// secret is never sent by the mobile app (public client).
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

/// Client → server to **create a new business** with a Google account.
///
/// Deliberately a different route from [`GoogleSignInRequest`], not a flag on
/// it. Entering an existing business and creating a new one are different
/// actions with different blast radii: entering touches somebody else's data,
/// creating starts an empty tenant that holds nobody's. Folding them into one
/// endpoint would mean a mistyped business name silently fabricates a tenant
/// instead of returning "no existe" — and the person would never learn they
/// are now in the wrong place.
///
/// Everything here is typed by the person on purpose. The server never infers
/// a business name from the email domain, and never reuses a slug it found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSignUpRequest {
    /// Opaque JWT from Google Sign-In / Credential Manager. Same verification
    /// as sign-in: signature, `iss`, `aud`, `exp`, and `email_verified`.
    pub id_token: String,
    /// Display name of the business. Written by the person, never derived.
    pub business_name: String,
    /// Optional short name used to log in later. Defaults to a slug of
    /// `business_name`.
    pub tenant_slug: Option<String>,
    /// Rubro, stored verbatim as `business.vertical` (`feria`, `almacen`, …).
    pub vertical: Option<String>,
}

/// Server → client after a new business was created with a Google account.
///
/// Mirrors [`GoogleSignInResponse`] plus the slug, which the client cannot
/// guess: the server may have adjusted it. Android needs it to pre-fill the
/// business field on the next login instead of asking the person to remember
/// what the server decided.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSignUpResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: i64,
    /// Email from the verified Google claims (display only).
    pub email: Option<String>,
    /// Short name of the business just created. Echoed so the client never
    /// has to re-derive it.
    pub tenant_slug: String,
}

/// Documented paths. Kept as consts so OpenAPI / docs and clients share one
/// string.
pub const GOOGLE_SIGN_IN_PATH: &str = "/api/v1/auth/google";

/// Creating a business is a **sibling** of sign-in, not a mode of it. See
/// [`GoogleSignUpRequest`] for why they are separate routes.
pub const GOOGLE_SIGN_UP_PATH: &str = "/api/v1/auth/google/negocio";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_stable() {
        assert_eq!(GOOGLE_SIGN_IN_PATH, "/api/v1/auth/google");
    }

    #[test]
    fn sign_up_path_is_stable_and_distinct() {
        assert_eq!(GOOGLE_SIGN_UP_PATH, "/api/v1/auth/google/negocio");
        // Si algún día colapsan en la misma ruta, el que entra y el que crea
        // dejan de distinguirse, y un nombre mal tipeado fabrica un negocio.
        assert_ne!(GOOGLE_SIGN_IN_PATH, GOOGLE_SIGN_UP_PATH);
    }

    #[test]
    fn sign_up_request_roundtrips_json() {
        let req = GoogleSignUpRequest {
            id_token: "opaque.not.a.real.token".into(),
            business_name: "Verdulería Rosa".into(),
            tenant_slug: None,
            vertical: Some("feria".into()),
        };
        let s = serde_json::to_string(&req).expect("ser");
        let back: GoogleSignUpRequest = serde_json::from_str(&s).expect("de");
        assert_eq!(back.business_name, "Verdulería Rosa");
        assert_eq!(back.vertical.as_deref(), Some("feria"));
        assert!(back.tenant_slug.is_none());
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
