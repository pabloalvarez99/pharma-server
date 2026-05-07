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
    db: &'static str,
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

async fn ready() -> (StatusCode, Json<ReadyResponse>) {
    (
        StatusCode::OK,
        Json(ReadyResponse {
            status: "ok",
            checks: ReadyChecks { db: "skipped" },
        }),
    )
}
