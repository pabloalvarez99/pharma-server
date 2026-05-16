//! Purchasing context (Fase 5-subset): supplier, supplier_product_mapping,
//! supplier_price_list.
//!
//! `purchase_order` / `purchase_order_item` / `purchase_payment` quedan para
//! fase posterior (dependen de inventory Fase 3 — stock_movement / product_batch
//! + costo promedio ponderado).
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (validación, comparación, import). `api` orquesta.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
