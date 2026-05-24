//! DTE (Documentos Tributarios Electrónicos) — SII Chile, nativo Rust.
//!
//! Soporta boleta electrónica (39), factura electrónica (33), nota débito (56),
//! nota crédito (61), guía de despacho (52). Genera XML schema SII xsd 1.0,
//! TimbreElectrónico (SHA1 + RSA-SHA1 con cert empresa), firma del XML completo,
//! envío al endpoint SII (sandbox `maullin.sii.cl` / prod `palena.sii.cl`),
//! polling de estado y libro ventas mensual.
//!
//! Tier monetización (gate al nivel `crates/api` vía `license::require`):
//! - **Free**: genera DTE local + export. Cliente envía manual al portal SII.
//! - **Pro**: envío SII automático boleta (39) + polling + resend.
//! - **Business**: todos los tipos + libro ventas + X/Z + multi-cert.
//!
//! Diseño + decisiones: ADR-0011 (`docs/adr/0011-dte-provider-native-rust.md`).
//! Schema persistente: migración `migrations/0017_dte.surql` (tablas `dte`,
//! `caf`, `cert_digital`).

pub mod caf;
pub mod cancel;
pub mod cert;
pub mod error;
pub mod reports;
pub mod sign;
pub mod sii;
pub mod timbre;
pub mod types;
pub mod xml;

pub use cancel::{cancel_dte, resend_dte};
pub use error::DteError;
pub use reports::{x_report, z_report, DailyReport, ReportKind, TipoBreakdown};
pub use types::{Caf, CertDigital, Dte, DteEstado, DteItem, DteTipo, EmisorConfig, SiiEnv};
