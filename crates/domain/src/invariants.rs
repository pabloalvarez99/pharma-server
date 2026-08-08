//! Canonical domain arithmetic — the single source of truth for the money,
//! cash-drawer, stock and refund formulas that the services apply.
//!
//! These functions are **pure** (no DB, no clock) so they can be exhaustively
//! property-tested (`tests/invariants_prop.rs`) and reused verbatim by the
//! `*::service` layer. A property failure here is a real domain bug — never
//! weaken the assertion to go green; fix the formula or report it.
//!
//! Money is `rust_decimal::Decimal`. El redondeo NO se asume: las funciones
//! que redondean reciben los decimales de la moneda del tenant
//! ([`crate::money::Currency::decimals`]), 0 para CLP y 2 para USD/EUR/MXN.
//! Stock is `i64` units.

use rust_decimal::Decimal;

use crate::errors::DomainError;

// --- sales / boleta totals -------------------------------------------------

/// One receipt line's total: `unit_price * qty`.
#[inline]
pub fn line_total(unit_price: Decimal, qty: i64) -> Decimal {
    unit_price * Decimal::from(qty)
}

/// Order subtotal = Σ of every line's `unit_price * qty`.
pub fn order_subtotal<I>(lines: I) -> Decimal
where
    I: IntoIterator<Item = (Decimal, i64)>,
{
    lines
        .into_iter()
        .map(|(price, qty)| line_total(price, qty))
        .sum()
}

/// Clamp the requested discount into `[0, subtotal]`:
/// * over the subtotal → capped at subtotal (total never goes negative),
/// * negative → zero (a discount cannot *add* money).
pub fn clamp_discount(discount_in: Decimal, subtotal: Decimal) -> Decimal {
    if discount_in > subtotal {
        subtotal
    } else if discount_in.is_sign_negative() {
        Decimal::ZERO
    } else {
        discount_in
    }
}

/// Order total = subtotal − clamped discount. Always in `[0, subtotal]`.
#[inline]
pub fn order_total(subtotal: Decimal, discount: Decimal) -> Decimal {
    subtotal - discount
}

/// Loyalty points awarded for a sale: `floor(total / clp_per_point)`.
/// `clp_per_point <= 0` is a misconfiguration → zero points (never panics).
pub fn loyalty_points(total: Decimal, clp_per_point: i64) -> i64 {
    if clp_per_point <= 0 {
        return 0;
    }
    use rust_decimal::prelude::ToPrimitive;
    (total / Decimal::from(clp_per_point))
        .trunc()
        .to_i64()
        .unwrap_or(0)
}

/// Cash change for a pure-cash counter sale: `cash − total`.
#[inline]
pub fn cash_change(cash: Decimal, total: Decimal) -> Decimal {
    cash - total
}

/// Tax breakdown of a **tax-inclusive** total (shelf prices already include
/// tax — convención de Chile y de la mayoría del retail del mundo). Returns
/// `(net, tax)` with `net + tax == total` **exactly** by construction.
/// `tax_percent == 0` (rubro exento) yields `(total, 0)`.
///
/// `decimals` is the currency's minor-unit exponent
/// ([`crate::money::Currency::decimals`]): 0 para CLP, 2 para USD/EUR/MXN. Es
/// un parámetro y no una constante porque el redondeo es aritmética, no
/// formato: con `round_dp(0)` sobre USD, `12.50` se convierte en `12` y el
/// negocio pierde 50 centavos por línea.
///
/// El neto se redondea y el impuesto sale por diferencia — nunca al revés.
/// Redondear ambos por separado rompe `net + tax == total`, que es la
/// identidad sobre la que se arma cualquier declaración de impuestos.
pub fn tax_breakdown(
    total_inclusive: Decimal,
    tax_percent: Decimal,
    decimals: u32,
) -> (Decimal, Decimal) {
    if tax_percent.is_zero() {
        return (total_inclusive, Decimal::ZERO);
    }
    let rate = (Decimal::from(100i64) + tax_percent) / Decimal::from(100i64);
    let net = (total_inclusive / rate).round_dp(decimals);
    let tax = total_inclusive - net;
    (net, tax)
}

/// IVA breakdown for a Chilean tenant: [`tax_breakdown`] fijado en CLP
/// (0 decimales). "IVA" es el nombre chileno del impuesto; las capas genéricas
/// usan `tax_breakdown` con la [`crate::money::MoneyConfig`] del tenant.
#[inline]
pub fn iva_breakdown(total_inclusive: Decimal, iva_percent: u8) -> (Decimal, Decimal) {
    tax_breakdown(total_inclusive, Decimal::from(iva_percent), 0)
}

// --- cash drawer (apertura / arqueo / cierre) ------------------------------

/// Expected drawer cash at close: `opening + cash_sales + ingresos − retiros`.
#[inline]
pub fn expected_drawer(
    opening: Decimal,
    cash_sales: Decimal,
    movements_in: Decimal,
    movements_out: Decimal,
) -> Decimal {
    opening + cash_sales + movements_in - movements_out
}

/// Arqueo discrepancy: `counted − expected`. Zero ⇒ cuadra; nonzero ⇒ the
/// drawer is over (`> 0`) or short (`< 0`).
#[inline]
pub fn discrepancy(counted: Decimal, expected: Decimal) -> Decimal {
    counted - expected
}

// --- stock -----------------------------------------------------------------

/// Apply a signed stock delta, rejecting any move that would drive stock
/// negative. The conservation law: applying every movement in order from a
/// non-negative start never yields a negative on-hand.
pub fn apply_delta(current: i64, delta: i64) -> Result<i64, DomainError> {
    let next = current
        .checked_add(delta)
        .ok_or_else(|| DomainError::Invalid("overflow de stock".into()))?;
    if next < 0 {
        return Err(DomainError::InsufficientStock);
    }
    Ok(next)
}

/// Stock implied by a movement ledger = Σ deltas (starting from zero).
#[inline]
pub fn fold_stock(deltas: &[i64]) -> i64 {
    deltas.iter().sum()
}

// --- FEFO allocation -------------------------------------------------------

/// Plan a FEFO consumption of `qty` over `lots` already sorted
/// earliest-expiry-first as `(batch_id, available)`. Pure core of
/// `inventory::repo::plan_fefo`.
///
/// Guarantees on `Ok(plan)`:
/// * `Σ plan.qty == qty`,
/// * every `plan.qty >= 1` and `<= lot.available`,
/// * lots are consumed in the given (FEFO) order.
///
/// `Err(InsufficientStock)` when `Σ available < qty`; `Err(Invalid)` when
/// `qty <= 0`.
pub fn fefo_plan(
    lots: &[(String, i64)],
    qty: i64,
) -> Result<Vec<crate::inventory::model::FefoAllocation>, DomainError> {
    if qty <= 0 {
        return Err(DomainError::Invalid("qty debe ser > 0".into()));
    }
    let mut remaining = qty;
    let mut plan = Vec::new();
    for (batch, available) in lots {
        if remaining == 0 {
            break;
        }
        if *available <= 0 {
            continue;
        }
        let take = remaining.min(*available);
        plan.push(crate::inventory::model::FefoAllocation {
            batch: batch.clone(),
            qty: take,
        });
        remaining -= take;
    }
    if remaining > 0 {
        return Err(DomainError::InsufficientStock);
    }
    Ok(plan)
}

// --- refunds ---------------------------------------------------------------

/// Cumulative over-refund guard: would refunding `requested_now` more units —
/// on top of `already_refunded` — exceed `sold`? Encodes "devolución ≤ venta
/// original" across *sequential* partial refunds (BUG-005 fraud vector).
#[inline]
pub fn refund_exceeds_sold(already_refunded: i64, requested_now: i64, sold: i64) -> bool {
    already_refunded + requested_now > sold
}
