//! Money types. CLP-only in v0.2.0; `currency` reserved for forward-compat.
//!
//! Use `rust_decimal::Decimal` for prices, totals, taxes, costs. JSON serializes
//! as string to avoid float drift in clients.

pub use rust_decimal::Decimal;

/// ISO-4217 currency code. CLP fixed in v0.2.0.
pub const CURRENCY_CLP: &str = "CLP";

/// IVA Chile, default 19% — overrideable per-tenant via `setting`.
pub const IVA_DEFAULT_PERCENT: u8 = 19;
