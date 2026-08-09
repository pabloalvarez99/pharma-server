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
use crate::role::{admin_plus, cashier_plus, RoleSet};
use crate::AppState;

use assist::{Answer, AssistConfig, AssistQuery, BuildOutcome, Intent};

#[derive(Debug, Deserialize)]
struct AskRequest {
    question: String,
}

#[derive(Debug, Deserialize)]
struct ActRequest {
    confirm_token: String,
}

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

/// Whether the caller may run write actions (admin/owner). Read questions stay
/// open to `cashier_plus`; only the agent's *hands* are gated.
fn can_write(claims: &auth::Claims) -> bool {
    RoleSet::from_claim(&claims.roles).contains_any(RoleSet::ADMIN | RoleSet::OWNER)
}

pub fn router(state: AppState) -> Router<AppState> {
    // `/ask` is open to counter staff (read questions + proposing actions the
    // server itself re-gates). `/act` — the actual write — is admin/owner only.
    let ask = Router::new()
        .route("/api/v1/assist/ask", post(ask))
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));
    let act = Router::new()
        .route("/api/v1/assist/act", post(act))
        .route_layer(crate::role::layer(state, admin_plus()));
    ask.merge(act)
}

/// Execute a previously-proposed action. Admin/owner gated (route layer); the
/// `confirm_token` is single-use, tenant-bound, and short-lived (ADR-0016).
async fn act(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<ActRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let token = req.confirm_token.trim();
    if token.is_empty() {
        return Err(ApiError::invalid("confirm_token requerido"));
    }
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let actor = surrealdb::sql::thing(&claims.sub).ok();

    let action = assist::store()
        .consume(token, &tenant)
        .map_err(|e| ApiError::invalid(e.message()))?;
    let outcome = assist::execute(db.as_ref(), &tenant, actor.as_ref(), action).await?;
    Ok(Json(outcome))
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

    // WRITE-ACTION path first: a write request never reaches the read agent.
    // The proposal writes NOTHING — execution waits for `POST /assist/act`.
    let parsed = assist::parse_action(question);
    if !matches!(parsed, assist::ActionParse::NotAnAction) {
        // Friendly (not 403) gate at propose time: don't even mint a token for a
        // user who couldn't execute it. `/act` re-checks via the admin layer.
        if !can_write(&claims) {
            return Ok(Json(Answer::note(
                "Solo un administrador o dueño puede pedirme acciones (ventas, fiados, gastos, \
                 clientes, productos, precios u órdenes de compra). Pídeselo a quien administra \
                 el negocio; la venta por pantalla la puedes hacer igual.",
            )));
        }
        return match assist::build(db.as_ref(), &tenant, parsed).await? {
            BuildOutcome::NotAnAction => unreachable!("guarded by parse_action above"),
            BuildOutcome::Reject(msg) => Ok(Json(Answer::note(msg))),
            BuildOutcome::Ready(action) => {
                // El resumen que se confirma tiene que estar escrito en la
                // moneda del tenant, no en pesos chilenos por defecto.
                let money =
                    assist::Money::from(domain::settings::currency(db.as_ref(), &tenant).await?);
                let proposal = assist::store().propose(action, &tenant, &money);
                Ok(Json(Answer::proposal(proposal)))
            }
        };
    }

    let intent = assist::parse(question);

    // Gross margin (monthly or per-product) is a paid capability. Degrade (not
    // 402) on the Free tier so the agent stays friendly: it still tells the
    // owner *how* to unlock it.
    if matches!(intent, Intent::MargenMes | Intent::MargenProducto(_)) {
        let lic = state.license.load();
        if !license::entitled(&lic, "reports.margins_daily") {
            let answer = Answer::new(
                &intent,
                "El margen es parte del plan Pro. Actualiza tu plan para ver \
                 ganancias y rentabilidad. Mientras tanto puedo darte tus ventas: \
                 pregúntame «ventas del mes».",
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
    // Offline-first: `select_provider` returns the deterministic provider unless
    // the owner opted into an LLM (default OFF, ADR-0016/ADR-0017). The opt-in +
    // BYO key live in per-tenant settings (telemetry-style consent, ADR-0005
    // inv. 3); absent/empty → fully offline, zero network. Even when enabled,
    // the LLM provider degrades to the deterministic answer on any failure.
    let cfg = load_assist_config(db.as_ref(), &tenant).await;
    let answer = assist::select_provider(&cfg).answer(&query).await?;
    Ok(Json(answer))
}

const ASSIST_LLM_ENABLED_KEY: &str = "assist.llm_enabled";
const ASSIST_LLM_API_KEY: &str = "assist.llm_api_key";

/// Build [`AssistConfig`] from per-tenant settings. Both reads are best-effort:
/// a missing/failed read yields the offline default (the agent must never error
/// because a setting is absent). The key is never logged.
async fn load_assist_config(db: &db::Db, tenant: &Thing) -> AssistConfig {
    let enabled = matches!(
        domain::sales::service::get_setting(db, tenant, ASSIST_LLM_ENABLED_KEY).await,
        Ok(Some(s)) if s.value == "true"
    );
    if !enabled {
        return AssistConfig::default();
    }
    let key = match domain::sales::service::get_setting(db, tenant, ASSIST_LLM_API_KEY).await {
        Ok(Some(s)) if !s.value.trim().is_empty() => Some(s.value),
        _ => None,
    };
    AssistConfig {
        llm_enabled: true,
        llm_api_key: key,
    }
}
