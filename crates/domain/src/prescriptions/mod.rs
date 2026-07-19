//! Prescriptions context: prescription (Ley 20.000 immutable log) and
//! pharmacist_shift (turno químico — same regulatory domain).
//!
//! Layering: [`model`] · [`repo`] · [`service`]. The API crate orchestrates.
//!
//! ## Immutability
//! `prescription` rows are append-only at the application layer. The service
//! exposes only `create` / `get` / `list` — no update, no delete. Controlled-
//! drug entries (`controlled = true`) also enforce mandatory doctor fields.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
