//! Cuenta corriente / fiado (V1) — el retail chico chileno fía al cliente
//! habitual. Ledger APPEND-ONLY inmutable (`customer_ledger`, migración 0039):
//! cada `cargo` (venta fiada) y cada `abono` (pago) es una fila; el saldo se
//! calcula sumando, nunca se muta un total.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (persistencia tenant-scoped) ·
//! [`service`] (validación de abono). El sale service llama a
//! [`repo::post_cargo`] cuando una venta se paga con `pos_fiado`.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
