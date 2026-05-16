//! `/api/v1` domain routes. Each context (catalog, inventory, …) owns a
//! submodule that exposes `router()`. `api` orchestrates only: extract
//! claims/tenant, call `domain::*::service`, map `DomainError` → envelope.

pub mod catalog;
pub mod purchasing;

use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    catalog::router(state.clone()).merge(purchasing::router(state))
}
