//! Cash register (caja) — apertura, cierre, arqueo, movimientos.
//!
//! A session holds the cash drawer state from opening to closing. The expected
//! close is computed deterministically as:
//!
//!   expected = opening_cash
//!            + Σ cash_movement(ingreso, in session)
//!            - Σ cash_movement(retiro,  in session)
//!            + Σ order.cash_amount where order.tenant=t AND order.created_at
//!              between opened_at..close_time AND payment_method IN
//!              ('pos_cash','pos_mixed') AND status NOT IN ('refunded','cancelled')
//!
//! The pharmacist counts physical cash on close (`closing_cash_counted`); the
//! `discrepancia` = counted - expected is recorded but never enforced — a
//! short/over drawer is a fact to investigate, not a transition error.

pub mod model;
pub mod service;

pub use model::*;
