//! Offline-first license verification for pharma-server.
//!
//! Licenses are JSON documents signed Ed25519 by the licenser
//! (`pharma-license-server`, separate repo). Verification is 100% local: the
//! binary embeds the licenser public keys in [`keys::LICENSER_KEYS`] and uses
//! the canonical-JSON + Ed25519 primitives already in the `agent` crate.
//!
//! Spec: `docs/strategy/license-architecture.md`. Invariants: ADR-0002,
//! ADR-0005, ADR-0006, ADR-0007.

pub mod gate;
pub mod keys;
pub mod schema;
pub mod store;
pub mod verify;

pub use gate::{entitled, is_expired, is_in_grace, require, GateError};
pub use schema::{BoughtAddon, License, Metadata, Tier, SCHEMA_VERSION};
pub use store::{default_license_path, load_from_disk, save_to_disk};
pub use verify::{parse_and_verify, LicenseError};
