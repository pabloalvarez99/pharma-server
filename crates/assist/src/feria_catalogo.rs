//! Primera venta feria sin SKU: parse de precio dicho + ensure de dominio.

use db::Db;
use domain::catalog::feria::ensure_simple_product;
use domain::catalog::model::ProductDto;
use domain::DomainResult;
use rust_decimal::Decimal;
use surrealdb::sql::Thing;

/// Delegado a [`domain::catalog::feria::ensure_simple_product`].
pub async fn asegurar_cosa_feria(
    db: &Db,
    tenant: &Thing,
    name: &str,
    price: Decimal,
) -> DomainResult<ProductDto> {
    ensure_simple_product(db, tenant, name, price).await
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

/// Quita la cola de precio (« a 2000», « a $2.000») del nombre del producto.
pub fn sin_precio_cola(name: &str) -> String {
    let lower = name.to_lowercase();
    const CUES: &[&str] = &[
        " a $",
        " a ",
        " por $",
        " por ",
        " precio $",
        " precio ",
    ];
    let mut cut_at: Option<usize> = None;
    for cue in CUES {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(cue) {
            let pos = from + rel;
            let amount_at = pos + cue.len();
            if parse_monto_from(&name[amount_at..]).is_some() {
                cut_at = Some(pos);
            }
            from = pos + 1;
        }
    }
    match cut_at {
        Some(i) => name[..i].trim().to_string(),
        None => name.trim().to_string(),
    }
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
        assert_eq!(sin_precio_cola("tomates a $2.000"), "tomates");
        assert_eq!(sin_precio_cola("tomates a 2000"), "tomates");
        assert_eq!(sin_precio_cola("arroz a granel"), "arroz a granel");
    }
}
