//! Purchasing context: supplier, supplier_product_mapping,
//! supplier_price_list (Fase 5-subset) + `purchase_order` /
//! `purchase_order_item` (Fase 5-full slice 1, migración 0015) + recepción
//! `draft→received` con stock + costo promedio ponderado + audit movement
//! (slice 2, sin migración nueva — enum `received` ya en 0015) +
//! `purchase_payment` accounts-payable (slice 3, migración 0016).
//!
//! Ciclo completo PO local: crear (`POST /api/v1/purchase-orders`) →
//! recibir (`POST /{id}/receive`) → pagar (`POST /{id}/payments`) →
//! consultar AP (`GET /{id}/payments`).
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (validación, comparación, import). `api` orquesta.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
