//! pharma-server domain layer.
//!
//! Bounded contexts. Each submodule owns its `model`, `repo`, `service`, `errors`.
//! API crate orchestrates only; no axum types live here.

pub mod agent_orders;
pub mod branches;
pub mod cash_register;
pub mod catalog;
pub mod customers;
pub mod expenses;
pub mod finance;
pub mod inventory;
pub mod operations;
pub mod prescriptions;
pub mod purchasing;
pub mod reports;
pub mod rubro;
pub mod sales;
pub mod seed;
pub mod settings;
pub mod web_keys;

pub mod errors;
pub mod invariants;
pub mod money;

pub use errors::{DomainError, DomainResult};
