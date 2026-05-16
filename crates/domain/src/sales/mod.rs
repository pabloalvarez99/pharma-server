//! Sales context (Fase 4 · POS): order, order_item, devolucion, devolucion_item,
//! admin_setting (key/value), idempotency_key.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (atomic POST /pos/sale, refund, settings) · [`controlled`]
//! (Decreto 404 CL set lookup) · [`interactions`] (Beers + Vademécum CL pair
//! checker).
//!
//! Invariants:
//! * `POST /pos/sale` is a single multi-statement SurrealQL tx:
//!   `BEGIN; CREATE order; CREATE order_item×N; UPDATE product SET stock-=qty;
//!   CREATE stock_movement(reason='sale', ref=order.id)×N;
//!   [CREATE prescription if controlled]; [CREATE loyalty_transaction]; COMMIT;`
//! * FEFO consumption: when `product_batch` rows exist, decrement oldest-expiry
//!   first via [`crate::inventory::service::plan_fefo`]. Atomic batch updates
//!   piggyback on the same tx (one statement per allocation).
//! * Idempotency: header `Idempotency-Key` → row in `idempotency_key` with
//!   24h TTL; replay returns cached `response_json` + `status_code`.
//! * Stock writes NEVER bypass `inventory::service::add_movement` — sales
//!   composes inventory's primitives rather than writing `product.stock` direct.
//!
//! Out of scope (deferred):
//! * Online payment providers (Webpay/MercadoPago/Stripe) — Fase 4.5 opt-in.
//! * SII boleta electrónica DTE emission — Fase 9 stub (501 until cert flow).

pub mod controlled;
pub mod interactions;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
