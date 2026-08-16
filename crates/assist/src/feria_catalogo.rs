//! Asegura un producto simple de feria para la primera venta sin SKU.
//!
//! Merge: si landed catalog/feria.rs, delegar a ensure_simple_product.

use db::Db;
use domain::catalog::model::{NewProduct, ProductDto, ProductFilters};
use domain::catalog::service as catalog;
use domain::{DomainError, DomainResult};
use rust_decimal::Decimal;
use surrealdb::sql::Thing;

/// Idempotente por nombre exacto (case-insensitive): si ya existe, lo devuelve
/// sin tocar el precio; si no, lo crea como ítem simple de feria
/// (`physical_stock=false`, `stock=0`, `attrs.rb_simple=true`).
pub async fn asegurar_cosa_feria(
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

    let products = catalog::list_products(
        db,
        tenant,
        ProductFilters {
            search: Some(name.to_string()),
            active: Some(true),
            limit: Some(10),
            ..ProductFilters::default()
        },
    )
    .await?;

    if let Some(existing) = products
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
    {
        return Ok(existing);
    }

    catalog::create_product(
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
            attrs: Some(serde_json::json!({ "rb_simple": true })),
        },
    )
    .await
}

/// Extrae un monto de colas tipo ` a 2000`, ` a $2.000`, ` por 1500`, ` precio 2000`.
/// Miles con punto chilenos: `2.000` → 2000. Si no hay monto, `None`.
pub fn precio_dicho(raw_linea: &str) -> Option<Decimal> {
    let lower = raw_linea.to_lowercase();
    // Orden: variantes con `$` primero para que ` a $` gane sobre ` a `.
    const CUES: &[&str] = &[
        " a $",
        " a ",
        " por $",
        " por ",
        " precio $",
        " precio ",
    ];

    let mut best: Option<(usize, usize)> = None; // (start_of_amount, cue_end)
    for cue in CUES {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(cue) {
            let pos = from + rel;
            let amount_at = pos + cue.len();
            // Prefer the rightmost cue so "x a y por 1500" takes the price tail.
            if best.map(|(a, _)| amount_at > a).unwrap_or(true) {
                if parse_monto_from(&raw_linea[amount_at..]).is_some() {
                    best = Some((amount_at, amount_at));
                }
            }
            from = pos + 1;
        }
    }

    best.and_then(|(at, _)| parse_monto_from(&raw_linea[at..]))
}

fn parse_monto_from(tail: &str) -> Option<Decimal> {
    let t = tail.trim_start().trim_start_matches('$').trim_start();
    // Take a leading money token (digits + CL thousands `.` / decimal `,`).
    let token: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    if token.is_empty() {
        return None;
    }
    // Chilean thousands: strip non-digits (`2.000` → `2000`). Same as agent
    // `parse_money` for integer CLP amounts said aloud.
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn precio_dicho_extrae_colas() {
        assert_eq!(precio_dicho("tomates a $2.000"), Some(d("2000")));
        assert_eq!(precio_dicho("tomates a 2000"), Some(d("2000")));
        assert_eq!(precio_dicho("cilantro por 1500"), Some(d("1500")));
        assert_eq!(precio_dicho("lechuga precio 2000"), Some(d("2000")));
        assert_eq!(precio_dicho("tomates"), None);
    }
}
