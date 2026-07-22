//! Tauri command surface, one module per domain. Every command is a thin HTTP
//! wrapper over the running `pharma-server` (`crates/api`) — the JWT stays in
//! [`crate::state::SessionState`] (memory only) and money crosses the wire as
//! STRINGS (see [`crate::types`]).

pub mod assist;
pub mod audit;
pub mod auth;
pub mod cash;
pub mod catalog;
pub mod credit;
pub mod customers;
pub mod dte;
pub mod expenses;
pub mod license;
pub mod pos;
pub mod prescriptions;
pub mod print;
pub mod purchases;
pub mod reports;
pub mod rubro;
pub mod seed;
pub mod settings;
