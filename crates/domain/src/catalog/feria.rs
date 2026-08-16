//! Productos simples de feria: nombre + precio, sin inventario.
//!
//! La dueña anota «tomates $2000». No hay barcode, laboratorio ni stock físico:
//! el ítem se vende como servicio (`physical_stock = false`) para que el POS no
//! muera con «Te quedaste sin stock». Idempotente por nombre (case-insensitive):
//! un segundo ensure no toca el precio (eso es AjustarPrecio del agente).

use rust_decimal::Decimal;
use serde_json::json;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::{NewProduct, ProductDto, ProductFilters};
use super::service;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Busca o crea un producto simple (feria): nombre + precio, sin inventario.
///
/// - Nombre vacío → `Invalid("nombre requerido")`
/// - Precio negativo → `Invalid("precio no puede ser negativo")`
/// - Match exacto case-insensitive de `name` activo → devuelve ese (sin patch)
/// - Si no existe → `create_product` con `physical_stock=false`, `stock=0`,
///   `attrs.rb_simple = true` (no reutiliza el centinela `rb_venta_suelta`)
pub async fn ensure_simple_product(
    db: &Db,
    tenant: &Thing,
    name: &str,
    price: Decimal,
) -> DomainResult<ProductDto> {
    let name = name.trim();
    if name.is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    if price < Decimal::ZERO {
        return Err(DomainError::Invalid("precio no puede ser negativo".into()));
    }

    let candidates = service::list_products(
        db,
        tenant,
        ProductFilters {
            search: Some(name.to_string()),
            active: Some(true),
            limit: Some(10),
            ..Default::default()
        },
    )
    .await?;

    if let Some(existing) = candidates
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
    {
        return Ok(existing);
    }

    service::create_product(
        db,
        tenant,
        NewProduct {
            name: name.to_string(),
            slug: None,
            description: None,
            price,
            cost_price: None,
            stock: 0,
            physical_stock: Some(false),
            category: None,
            image_url: None,
            external_id: None,
            laboratory: None,
            therapeutic_action: None,
            active_ingredient: None,
            prescription_type: None,
            presentation: None,
            discount_percent: None,
            attrs: Some(json!({ "rb_simple": true })),
        },
    )
    .await
}
