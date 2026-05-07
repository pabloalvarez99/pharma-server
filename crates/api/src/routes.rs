use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::middleware::auth::AuthUser;
use crate::AppState;

#[derive(Serialize)]
struct Me {
    sub: String,
    tenant_id: String,
    roles: Vec<String>,
    exp: i64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/me", get(me))
}

async fn me(AuthUser(claims): AuthUser) -> Json<Me> {
    Json(Me {
        sub: claims.sub,
        tenant_id: claims.tenant_id,
        roles: claims.roles,
        exp: claims.exp,
    })
}
