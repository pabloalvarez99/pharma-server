//! Catalog context: product, category, product_barcode, barcode_catalog,
//! therapeutic_category_mapping.
//!
//! Layering: [`model`] (DTOs/inputs) · [`repo`] (tenant-scoped persistence) ·
//! [`service`] (slug, validation, bulk reprice, stock, **variantes**).
//! `api` orchestrates.
//!
//! # Variantes multi-SKU (migración 0034, Opción A)
//!
//! Una variante es un `product` hijo con `parent_id` apuntando al padre,
//! `attrs` (talla/color/sku), barcode y stock propios. Ver
//! `docs/product/variants-design.md`. No hay tabla `product_variant`.
//!
//! Delete: [`service::delete_variant`] (soft + free barcode). Edit: generic
//! [`service::update_product`] (incluye `barcode` opcional).

pub mod feria;
pub mod model;
pub mod repo;
pub mod service;

pub use feria::ensure_simple_product;
pub use model::*;
