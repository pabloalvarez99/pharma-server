//! Inventory context: stock_movement, product_batch, falta.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence)
//! · [`service`] (atomic stock changes, FEFO, summaries, ABC, reorder).
//!
//! Invariant: `product.stock = SUM(stock_movement.delta)` per (tenant,product).
//! All stock-mutating paths route through [`service::add_movement`]; the
//! catalog/sales contexts delegate to it (no direct `product.stock` writes).

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
