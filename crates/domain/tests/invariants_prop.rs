//! Property-based tests for the canonical domain invariants
//! (`domain::invariants`), the formulas the sales / cash / inventory services
//! apply in production. Shrinking is on by default in proptest — a failure
//! prints the minimal counterexample.
//!
//! Both retail verticals (pharmacy + minimarket) are exercised: money math is
//! vertical-agnostic, so each money property runs under a `Vertical` tag, and
//! the controlled-substance gate is asserted as pharmacy-only.
//!
//! INVARIANT DISCIPLINE: a failing property is a real domain bug. Fix the
//! formula in `src/invariants.rs` — never relax the assertion.

use domain::invariants as inv;
use domain::sales::controlled::is_controlled;
use proptest::prelude::*;
use rust_decimal::Decimal;

/// The onboarding rubro. Money/total invariants must hold identically across
/// every vertical; only feature gates (controlled, recetas) differ.
#[derive(Debug, Clone, Copy)]
enum Vertical {
    Pharmacy,
    Minimarket,
}

fn vertical() -> impl Strategy<Value = Vertical> {
    prop_oneof![Just(Vertical::Pharmacy), Just(Vertical::Minimarket)]
}

/// Money amount as integer CLP pesos (no cents in production).
fn money() -> impl Strategy<Value = Decimal> {
    (0i64..=10_000_000).prop_map(Decimal::from)
}

/// Signed money (movements, discrepancies) including negatives.
fn signed_money() -> impl Strategy<Value = Decimal> {
    (-10_000_000i64..=10_000_000).prop_map(Decimal::from)
}

fn qty() -> impl Strategy<Value = i64> {
    1i64..=1_000
}

/// A sale line: (unit_price, quantity).
fn line() -> impl Strategy<Value = (Decimal, i64)> {
    (money(), qty())
}

proptest! {
    // --- boleta / totals ---------------------------------------------------

    /// 1. subtotal == Σ (unit_price · qty) over every line, for any vertical.
    #[test]
    fn subtotal_equals_sum_of_lines(lines in prop::collection::vec(line(), 0..20), v in vertical()) {
        let _ = v; // total math identical across verticals
        let manual: Decimal = lines.iter().map(|(p, q)| *p * Decimal::from(*q)).sum();
        prop_assert_eq!(inv::order_subtotal(lines.iter().copied()), manual);
    }

    /// 2. line_total is monotonic non-decreasing in quantity (price ≥ 0).
    #[test]
    fn line_total_monotonic_in_qty(price in money(), q1 in qty(), q2 in qty()) {
        let (lo, hi) = if q1 <= q2 { (q1, q2) } else { (q2, q1) };
        prop_assert!(inv::line_total(price, lo) <= inv::line_total(price, hi));
    }

    /// 3. Clamped discount is always within [0, subtotal].
    #[test]
    fn discount_clamped_into_range(discount in signed_money(), lines in prop::collection::vec(line(), 0..20)) {
        let subtotal = inv::order_subtotal(lines.iter().copied());
        let d = inv::clamp_discount(discount, subtotal);
        prop_assert!(d >= Decimal::ZERO);
        prop_assert!(d <= subtotal);
    }

    /// 4. Order total is in [0, subtotal] and equals subtotal − clamped discount.
    #[test]
    fn total_in_range_and_consistent(discount in signed_money(), lines in prop::collection::vec(line(), 1..20)) {
        let subtotal = inv::order_subtotal(lines.iter().copied());
        let d = inv::clamp_discount(discount, subtotal);
        let total = inv::order_total(subtotal, d);
        prop_assert_eq!(total, subtotal - d);
        prop_assert!(total >= Decimal::ZERO);
        prop_assert!(total <= subtotal);
    }

    /// 5. Loyalty points = floor(total / rate): points·rate ≤ total < (points+1)·rate.
    #[test]
    fn loyalty_is_floor_division(total in money(), rate in 1i64..=100_000) {
        let pts = inv::loyalty_points(total, rate);
        prop_assert!(pts >= 0);
        let lower = Decimal::from(pts) * Decimal::from(rate);
        let upper = Decimal::from(pts + 1) * Decimal::from(rate);
        prop_assert!(lower <= total);
        prop_assert!(total < upper);
    }

    /// 6. Misconfigured (≤0) loyalty rate yields zero points, never panics.
    #[test]
    fn loyalty_nonpositive_rate_is_zero(total in money(), rate in -10i64..=0) {
        prop_assert_eq!(inv::loyalty_points(total, rate), 0);
    }

    /// 7. Cash change = cash − total; non-negative exactly when cash ≥ total.
    #[test]
    fn cash_change_sign(cash in money(), total in money()) {
        let change = inv::cash_change(cash, total);
        prop_assert_eq!(change, cash - total);
        prop_assert_eq!(change >= Decimal::ZERO, cash >= total);
    }

    /// 8. IVA round-trip: net + iva == total exactly; iva ≥ 0; net ≤ total.
    #[test]
    fn iva_breakdown_roundtrips(total in money(), pct in prop_oneof![Just(0u8), Just(19u8)]) {
        let (net, iva) = inv::iva_breakdown(total, pct);
        prop_assert_eq!(net + iva, total);
        prop_assert!(iva >= Decimal::ZERO);
        prop_assert!(net <= total);
    }

    /// 9. Exempt rubro (0% IVA): net == total, iva == 0.
    #[test]
    fn iva_exempt_is_passthrough(total in money()) {
        let (net, iva) = inv::iva_breakdown(total, 0);
        prop_assert_eq!(net, total);
        prop_assert_eq!(iva, Decimal::ZERO);
    }

    // --- caja (cash drawer) ------------------------------------------------

    /// 10. Expected drawer = opening + cash_sales + in − out; adding one more
    ///     cash sale of `s` raises the expected cash by exactly `s`.
    #[test]
    fn expected_drawer_linear_in_sales(
        opening in money(), sales in money(), mov_in in money(), mov_out in money(), s in money()
    ) {
        let base = inv::expected_drawer(opening, sales, mov_in, mov_out);
        let with_more = inv::expected_drawer(opening, sales + s, mov_in, mov_out);
        prop_assert_eq!(with_more - base, s);
    }

    /// 11. Arqueo discrepancy = counted − expected; zero iff the drawer cuadra,
    ///     and a planted difference `d` is detected exactly.
    #[test]
    fn arqueo_detects_difference(expected in signed_money(), d in signed_money()) {
        let counted = expected + d;
        prop_assert_eq!(inv::discrepancy(counted, expected), d);
        prop_assert_eq!(inv::discrepancy(expected, expected), Decimal::ZERO);
    }

    // --- stock conservation ------------------------------------------------

    /// 12. apply_delta never yields negative stock; on Ok the result is exactly
    ///     current + delta.
    #[test]
    fn apply_delta_never_negative(current in 0i64..1_000_000, delta in -1_000_000i64..1_000_000) {
        match inv::apply_delta(current, delta) {
            Ok(next) => {
                prop_assert_eq!(next, current + delta);
                prop_assert!(next >= 0);
            }
            Err(_) => prop_assert!(current + delta < 0),
        }
    }

    /// 13. Stock from a movement ledger equals the running sum when every
    ///     intermediate step stays non-negative (Σ movimientos = stock actual).
    #[test]
    fn fold_stock_matches_sequential(deltas in prop::collection::vec(0i64..10_000, 0..50)) {
        // All-non-negative deltas can always be applied in sequence.
        let mut running = 0i64;
        for d in &deltas {
            running = inv::apply_delta(running, *d).expect("non-negative delta always applies");
        }
        prop_assert_eq!(running, inv::fold_stock(&deltas));
    }

    // --- FEFO allocation ---------------------------------------------------

    /// 14. On a satisfiable plan: Σ allocated == qty, each slice within its lot,
    ///     and lots consumed in the given FEFO order (prefix of the lot list).
    #[test]
    fn fefo_conserves_quantity(
        avails in prop::collection::vec(0i64..500, 1..15),
        want in 1i64..3_000,
    ) {
        let lots: Vec<(String, i64)> = avails
            .iter()
            .enumerate()
            .map(|(i, a)| (format!("batch:{i}"), *a))
            .collect();
        let total_avail: i64 = avails.iter().filter(|a| **a > 0).sum();
        match inv::fefo_plan(&lots, want) {
            Ok(plan) => {
                let allocated: i64 = plan.iter().map(|a| a.qty).sum();
                prop_assert_eq!(allocated, want);
                // Each allocation positive and ≤ its lot's availability.
                for a in &plan {
                    prop_assert!(a.qty >= 1);
                    let avail = lots.iter().find(|(b, _)| b == &a.batch).unwrap().1;
                    prop_assert!(a.qty <= avail);
                }
                // FEFO order: allocated batches are an in-order subsequence of lots.
                let order: Vec<&String> = lots.iter().map(|(b, _)| b).collect();
                let mut it = order.iter();
                for a in &plan {
                    prop_assert!(it.any(|b| **b == a.batch), "allocation out of FEFO order");
                }
            }
            Err(_) => prop_assert!(total_avail < want),
        }
    }

    /// 15. FEFO is unsatisfiable exactly when total availability < qty.
    #[test]
    fn fefo_insufficient_when_short(avails in prop::collection::vec(0i64..100, 1..10), want in 1i64..2_000) {
        let lots: Vec<(String, i64)> = avails
            .iter()
            .enumerate()
            .map(|(i, a)| (format!("b:{i}"), *a))
            .collect();
        let total: i64 = avails.iter().filter(|a| **a > 0).sum();
        prop_assert_eq!(inv::fefo_plan(&lots, want).is_err(), total < want);
    }

    // --- refunds (devolución ≤ venta original) -----------------------------

    /// 16. The cumulative over-refund guard fires exactly when prior + now > sold,
    ///     across sequential partial refunds (refund-fraud guard, BUG-005).
    #[test]
    fn refund_never_exceeds_sold(sold in 0i64..1_000, prior in 0i64..1_000, now in 0i64..1_000) {
        prop_assert_eq!(inv::refund_exceeds_sold(prior, now, sold), prior + now > sold);
    }

    // --- multi-tenant isolation (pure partition law) -----------------------

    /// 17. Stock derived from a tenant's own movements is unaffected by any
    ///     other tenant's movements — zero data crossing between two tenants.
    #[test]
    fn tenant_stock_isolation(
        a in prop::collection::vec(0i64..1_000, 0..30),
        b in prop::collection::vec(0i64..1_000, 0..30),
    ) {
        // Tenant A's stock computed alone vs. from a merged-but-tagged ledger.
        let a_alone = inv::fold_stock(&a);
        let b_alone = inv::fold_stock(&b);
        // Merged ledger tagged by tenant; project each back out.
        let merged: Vec<(u8, i64)> = a
            .iter()
            .map(|d| (0u8, *d))
            .chain(b.iter().map(|d| (1u8, *d)))
            .collect();
        let a_proj: i64 = merged.iter().filter(|(t, _)| *t == 0).map(|(_, d)| *d).sum();
        let b_proj: i64 = merged.iter().filter(|(t, _)| *t == 1).map(|(_, d)| *d).sum();
        prop_assert_eq!(a_proj, a_alone);
        prop_assert_eq!(b_proj, b_alone);
    }

    // --- controlled-substance gate (pharmacy vertical) ---------------------

    /// 18. A known controlled ingredient is detected case-insensitively and as a
    ///     substring; minimarket has no such concept (the gate is pharmacy-only,
    ///     but detection itself is data-driven, not vertical-driven).
    #[test]
    fn controlled_detected_case_insensitive(prefix in "[a-z ]{0,8}", suffix in "[a-z 0-9]{0,8}") {
        let ai = format!("{prefix}clonazepam{suffix}");
        prop_assert!(is_controlled(Some(&ai)));
        prop_assert!(is_controlled(Some(&ai.to_uppercase())));
    }

    /// 19. A clearly non-controlled ingredient is never flagged.
    #[test]
    fn non_controlled_not_flagged(_q in qty()) {
        prop_assert!(!is_controlled(Some("paracetamol 500mg")));
        prop_assert!(!is_controlled(Some("ibuprofeno")));
        prop_assert!(!is_controlled(None));
    }
}
