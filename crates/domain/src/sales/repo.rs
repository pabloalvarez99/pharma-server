//! Sales persistence (Fase 4). Implementation lands in the next slice — this
//! module currently exposes only the public function signatures the service
//! layer will call, so downstream callers (api crate) can wire types now.
//!
//! All write paths SHALL be implemented as multi-statement SurrealQL
//! `BEGIN; …; COMMIT;` blocks routed through `db.query(...).bind(...)`. See
//! `inventory::repo::apply_movement` for the atomic-tx pattern to mirror.
