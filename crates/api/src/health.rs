use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
struct LiveResponse {
    status: &'static str,
    uptime_secs: i64,
}

#[derive(Serialize)]
struct ReadyResponse {
    status: &'static str,
    checks: ReadyChecks,
}

#[derive(Serialize)]
struct ReadyChecks {
    db: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
}

async fn live(State(s): State<AppState>) -> Json<LiveResponse> {
    let uptime = (chrono::Utc::now() - s.started_at).num_seconds();
    Json(LiveResponse {
        status: "ok",
        uptime_secs: uptime,
    })
}

async fn ready(State(s): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    let (status_code, overall, db_status) = match &s.db {
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "degraded",
            "unavailable".to_string(),
        ),
        Some(handle) => match handle.query("RETURN 1").await {
            Ok(_) => (StatusCode::OK, "ok", "ok".to_string()),
            Err(e) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "degraded",
                format!("error: {e}"),
            ),
        },
    };
    (
        status_code,
        Json(ReadyResponse {
            status: overall,
            checks: ReadyChecks { db: db_status },
        }),
    )
}
