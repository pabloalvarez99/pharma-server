//! XML serializer for SII DTE schema (xsd 1.0).
//!
//! Subtask 9.1.a — implementación pendiente. Stubs definen la API que consumirán
//! `sign.rs` y `timbre.rs`.

pub mod boleta;
pub mod factura;
pub mod guia;
pub mod libro;
pub mod nota_credito;
pub mod nota_debito;

use crate::types::Dte;
use crate::DteError;

/// Renderiza el `<DTE>` raíz sin firma (pre-TED). Despacha al sub-renderer
/// según `dte.tipo`. Subtask 9.1.a llena esto.
pub fn render_unsigned(_dte: &Dte) -> Result<String, DteError> {
    Err(DteError::XmlInvalid(
        "xml::render_unsigned: pendiente subtask 9.1.a".to_string(),
    ))
}
