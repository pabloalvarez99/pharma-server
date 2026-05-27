//! `/api/v1` domain routes. Each context (catalog, inventory, …) owns a
//! submodule that exposes `router()`. `api` orchestrates only: extract
//! claims/tenant, call `domain::*::service`, map `DomainError` → envelope.

pub mod agent;
pub mod agent_orders;
pub mod audit;
pub mod backup;
pub use backup::{backup_now, prune_backups};
pub mod cash_register;
pub mod catalog;
pub mod customers;
pub mod expenses;
pub mod inventory;
pub mod license;
pub mod prescriptions;
pub mod purchasing;
pub mod sales;

use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    catalog::router(state.clone())
        .merge(inventory::router(state.clone()))
        .merge(customers::router(state.clone()))
        .merge(prescriptions::router(state.clone()))
        .merge(purchasing::router(state.clone()))
        .merge(sales::router(state.clone()))
        .merge(agent_orders::router(state.clone()))
        .merge(cash_register::router(state.clone()))
        .merge(expenses::router(state.clone()))
        .merge(backup::router(state.clone()))
        .merge(license::router(state.clone()))
        .merge(audit::router(state.clone()))
        .merge(agent::router(state))
}
