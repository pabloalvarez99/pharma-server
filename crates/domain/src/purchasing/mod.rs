//! Purchasing context: supplier, supplier_product_mapping,
//! supplier_price_list (Fase 5-subset) + `purchase_order` /
//! `purchase_order_item` creation (Fase 5-full, BACKLOG #8 slice 1,
//! migration 0015).
//!
//! Receipt (stock + product_batch + costo promedio ponderado) y
//! `purchase_payment` (cuentas por pagar) quedan para slices posteriores —
//! dependen de inventory Fase 3.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (validación, comparación, import). `api` orquesta.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
