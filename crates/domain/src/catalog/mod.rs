//! Catalog context: product, category, product_barcode, barcode_catalog,
//! therapeutic_category_mapping.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (slug, validation, bulk reprice, stock). `api` orchestrates.

pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
