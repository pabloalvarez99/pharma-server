//! Customers context: customer (cliente farmacia), loyalty_transaction.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (validation, RUT-unique-per-tenant, loyalty read aggregates).
//! Sales (Fase 4) writes `loyalty_transaction` records; here it's read-only.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
