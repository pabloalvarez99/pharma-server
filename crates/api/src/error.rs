//! Standard error envelope for `/api/v1/*` responses.
//!
//! Shape:
//! ```json
//! { "error": { "code": "SCREAMING_SNAKE", "message": "mensaje en español", "details": null | object } }
//! ```
//!
//! `code` stays in English SCREAMING_SNAKE (machine-readable, stable contract).
//! `message` is end-user copy in Spanish.
//! `details` is an optional free-form object for field errors, retry hints, etc.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    // --- common helpers (Spanish copy) ----------------------------------

    pub fn unauthorized_missing_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "MISSING_TOKEN",
            "Falta token de autenticación.",
        )
    }

    pub fn unauthorized_invalid_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "INVALID_TOKEN",
            "Token inválido o expirado.",
        )
    }

    pub fn bad_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "BAD_CREDENTIALS",
            "Credenciales inválidas.",
        )
    }

    pub fn service_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Servicio no disponible. Intenta nuevamente.",
        )
    }

    pub fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Permiso denegado para esta operación.",
        )
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Recurso no encontrado.")
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_INPUT", msg)
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", msg)
    }

    pub fn insufficient_stock() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INSUFFICIENT_STOCK",
            "Stock insuficiente para completar la operación.",
        )
    }

    /// 402 — feature requires a paid tier. Body code: `FEATURE_REQUIRES_UPGRADE`.
    /// Caller passes the feature key (e.g. `reports.margins_daily`) and the
    /// minimum tier (`pro` | `business` | `enterprise`). Spec:
    /// `docs/strategy/license-architecture.md` §7.2.
    pub fn payment_required(feature: &str, tier_required: &str) -> Self {
        Self::new(
            StatusCode::PAYMENT_REQUIRED,
            "FEATURE_REQUIRES_UPGRADE",
            format!(
                "Esta funcionalidad requiere el plan '{tier_required}'. \
                 Active en https://pharma-server.cl/upgrade (feature: {feature})."
            ),
        )
        .with_details(serde_json::json!({
            "feature": feature,
            "tier_required": tier_required,
        }))
    }

    /// 429 — request throttled by the rate-limit middleware.
    /// `scope` is `"tenant"` or `"ip"`. `retry_after_secs` is the recommended
    /// back-off (also surfaced as `Retry-After` response header by the
    /// middleware). Body code: `RATE_LIMITED`.
    pub fn rate_limited(scope: &'static str, retry_after_secs: u64) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Demasiadas solicitudes. Intenta nuevamente en unos segundos.",
        )
        .with_details(serde_json::json!({
            "scope": scope,
            "retry_after_secs": retry_after_secs,
        }))
    }

    pub fn not_implemented() -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "NOT_IMPLEMENTED",
            "Funcionalidad no implementada.",
        )
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg)
    }
}

impl From<license::GateError> for ApiError {
    fn from(e: license::GateError) -> Self {
        Self::payment_required(&e.feature, e.tier_required)
    }
}

impl From<domain::DomainError> for ApiError {
    fn from(e: domain::DomainError) -> Self {
        use domain::DomainError as D;
        let status = match &e {
            D::NotFound => StatusCode::NOT_FOUND,
            D::Conflict(_) => StatusCode::CONFLICT,
            D::Invalid(_) => StatusCode::BAD_REQUEST,
            D::InsufficientStock => StatusCode::UNPROCESSABLE_ENTITY,
            D::Forbidden => StatusCode::FORBIDDEN,
            D::IdempotencyReplay => StatusCode::CONFLICT,
            D::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            D::Db(_) | D::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        // DB/internal: log detail, return opaque Spanish copy.
        let message = match &e {
            D::Db(_) | D::Other(_) => {
                tracing::error!(error = %e, "domain internal error");
                "Error interno del servidor.".to_string()
            }
            other => other.to_string(),
        };
        Self::new(status, e.code(), message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let env = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(env)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn envelope_shape_with_details() {
        let e = ApiError::invalid("RUT inválido").with_details(serde_json::json!({"field":"rut"}));
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["code"], "INVALID_INPUT");
        assert_eq!(v["error"]["message"], "RUT inválido");
        assert_eq!(v["error"]["details"]["field"], "rut");
    }

    #[tokio::test]
    async fn payment_required_envelope() {
        let resp = ApiError::payment_required("reports.margins_daily", "pro").into_response();
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body = to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["code"], "FEATURE_REQUIRES_UPGRADE");
        assert!(v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("plan 'pro'"));
        assert_eq!(v["error"]["details"]["feature"], "reports.margins_daily");
        assert_eq!(v["error"]["details"]["tier_required"], "pro");
    }

    #[tokio::test]
    async fn gate_error_converts_to_payment_required() {
        let gate_err = license::GateError {
            feature: "branding.white_label".to_string(),
            tier_required: "enterprise",
        };
        let api_err: ApiError = gate_err.into();
        assert_eq!(api_err.status, StatusCode::PAYMENT_REQUIRED);
        assert_eq!(api_err.code, "FEATURE_REQUIRES_UPGRADE");
    }

    #[tokio::test]
    async fn envelope_omits_details_when_none() {
        let resp = ApiError::not_found().into_response();
        let body = to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["code"], "NOT_FOUND");
        assert!(v["error"].get("details").is_none());
    }
}
