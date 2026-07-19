//! Sucursales (branch) + cajas (register) — config-center entities.
//!
//! Tenant-scoped CRUD the UI uses to run multi-sucursal / multi-caja from
//! Configuración instead of CLI/seed only (migration 0032). A `register` may
//! belong to a `branch`; standalone registers (no branch) are allowed for
//! single-site installs. Live `cash_session` rows keep their free-string
//! `register_name` (migration 0011) — wiring sessions to register records is a
//! deliberate forward step kept out of scope here.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
