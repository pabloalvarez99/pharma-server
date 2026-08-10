//! Cash register (caja) — apertura, cierre, arqueo, movimientos.
//!
//! A session holds the cash drawer state from opening to closing. The expected
//! close is computed deterministically as:
//!
//!   expected = opening_cash
//!            + Σ cash_movement(ingreso, in session)
//!            - Σ cash_movement(retiro,  in session)
//!            + Σ cash_into_drawer(order) where order.tenant=t AND
//!              order.created_at between opened_at..close_time AND
//!              payment_method IN ('pos_cash','pos_mixed') AND status NOT IN
//!              ('refunded','cancelled')
//!
//! `cash_into_drawer` es el efectivo NETO DE VUELTO
//! ([`crate::invariants::cash_into_drawer`]), **no** `order.cash_amount`:
//! `cash_amount` es lo que el cliente entregó, y el vuelto sale del mismo
//! cajón. Sumarlo crudo — lo que hacía la migración 0030 — declaraba un
//! faltante del tamaño del vuelto en cada venta (corregido en 0046).
//!
//! The pharmacist counts physical cash on close (`closing_cash_counted`); the
//! `discrepancia` = counted - expected is recorded but never enforced — a
//! short/over drawer is a fact to investigate, not a transition error.

pub mod model;
pub mod service;

pub use model::*;
