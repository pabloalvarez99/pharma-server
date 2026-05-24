//! Subtask 9.1.j — gating del *envío automático al SII* por tier de licencia.
//!
//! Política freemium (CLAUDE.md § "Modelo de negocio", matrix de tiers):
//! - **Free**: DTE local-only (render + firma + store). NO envío automático al
//!   SII; el contribuyente sube el XML al portal SII manualmente.
//! - **Pro**: envío automático de boleta electrónica (tipo 39).
//! - **Business+**: envío multi-tipo (factura 33, NC 56, ND 61, guía 52) además
//!   de la boleta.
//!
//! # Límite de dependencias (deliberado)
//!
//! Este módulo es **pura lógica** y NO depende de `crates/license`. Define su
//! propio [`SendTier`] (`Free|Pro|Business|Enterprise`) para mantener `crates/dte`
//! standalone (sin red, sin axum, sin acoplar al schema de licencias). El caller
//! en `crates/api` es quien mapea `license::Tier` → [`SendTier`] antes de invocar
//! [`require_send_allowed`]. Así el crate de DTE se puede testear y reusar sin
//! arrastrar el stack de licenciamiento, y la frontera tier↔feature vive en un
//! solo lugar (la capa HTTP), no esparcida.

use crate::error::DteError;
use crate::types::DteTipo;

/// Tier de licencia, espejo *local* de `license::Tier`. Definido aquí a
/// propósito para no acoplar `crates/dte` a `crates/license` (ver doc del
/// módulo). El caller traduce desde su propio tipo de tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SendTier {
    Free,
    Pro,
    Business,
    Enterprise,
}

impl SendTier {
    /// Etiqueta canónica (`free|pro|business|enterprise`). Coincide con
    /// `license::Tier::as_str` para que los mensajes de error sean consistentes
    /// entre crates.
    pub fn as_str(self) -> &'static str {
        match self {
            SendTier::Free => "free",
            SendTier::Pro => "pro",
            SendTier::Business => "business",
            SendTier::Enterprise => "enterprise",
        }
    }
}

/// Tier mínimo requerido para enviar automáticamente un tipo de DTE al SII.
///
/// - Boleta (39) → [`SendTier::Pro`].
/// - Factura (33), NC (61), ND (56), guía (52) → [`SendTier::Business`].
pub fn min_tier_for(tipo: DteTipo) -> SendTier {
    match tipo {
        DteTipo::BoletaElectronica => SendTier::Pro,
        DteTipo::FacturaElectronica
        | DteTipo::NotaCredito
        | DteTipo::NotaDebito
        | DteTipo::GuiaDespacho => SendTier::Business,
    }
}

/// Verifica que el `tier` dado tenga permiso de enviar `tipo` al SII.
///
/// Free siempre falla (local-only). Pro habilita sólo boleta (39). Business y
/// Enterprise habilitan todos los tipos soportados.
///
/// # Errores
///
/// Retorna [`DteError::SendNotEntitled`] (que el caller mapea a HTTP 402
/// `FEATURE_REQUIRES_UPGRADE`) cuando el tier es insuficiente, con `tier`,
/// `tipo` y `required_tier` para el mensaje user-facing.
pub fn require_send_allowed(tier: SendTier, tipo: DteTipo) -> Result<(), DteError> {
    let required = min_tier_for(tipo);
    if tier >= required {
        Ok(())
    } else {
        Err(DteError::SendNotEntitled {
            tier: tier.as_str().to_string(),
            tipo: tipo.code(),
            required_tier: required.as_str().to_string(),
        })
    }
}
