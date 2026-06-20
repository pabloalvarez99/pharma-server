//! Business agent endpoint — `POST /api/v1/assist/ask` ("Pregúntale a tu
//! negocio", ADR-0016).
//!
//! READ-ONLY, tenant-scoped (JWT `tenant_id`), role-gated. Parses the owner's
//! Spanish question into a deterministic intent and answers it from the
//! tenant's own data via the `assist` provider seam. 100% offline-first
//! (ADR-0005): no network, no model in this MVP — only the local deterministic
//! provider is wired.
//!
//! License posture: the gross-margin intent honours the same gate as
//! `/reports/margins-daily` (`reports.margins_daily`). On the Free tier it
//! degrades to a friendly upgrade nudge instead of 402-ing — the agent must
//! always answer something, never error in the owner's face.

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::cashier_plus;
use crate::AppState;

use assist::{Answer, AssistProvider, AssistQuery, Deterministic, Intent};

#[derive(Debug, Deserialize)]
struct AskRequest {
    question: String,
}

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/assist/ask", post(ask))
        .route_layer(crate::role::layer(state, cashier_plus()))
}

async fn ask(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<AskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let question = req.question.trim();
    if question.is_empty() {
        return Err(ApiError::invalid("la pregunta no puede estar vacía"));
    }

    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let intent = assist::parse(question);

    // Gross margin is a paid capability. Degrade (not 402) on the Free tier so
    // the agent stays friendly: it still tells the owner *how* to unlock it.
    if matches!(intent, Intent::MargenMes) {
        let lic = state.license.load();
        if !license::entitled(&lic, "reports.margins_daily") {
            let answer = Answer::new(
                &intent,
                "El margen del mes es parte del plan Pro. Actualiza tu plan para \
                 ver ganancias y rentabilidad. Mientras tanto puedo darte tus \
                 ventas: pregúntame «ventas del mes».",
            );
            return Ok(Json(answer));
        }
    }

    let query = AssistQuery {
        question,
        intent,
        db: db.as_ref(),
        tenant: &tenant,
    };
    let answer = Deterministic.answer(&query).await?;
    Ok(Json(answer))
}
