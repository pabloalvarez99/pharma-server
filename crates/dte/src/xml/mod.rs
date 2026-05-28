//! XML serializer for SII DTE schema (xsd 1.0).
//!
//! Subtask 9.1.a — boleta 39 implementada. Factura/notas/guía vienen en
//! subtask 9.1.f.

pub mod boleta;
pub mod factura;
pub mod guia;
pub mod libro;
pub mod nota_credito;
pub mod nota_debito;
pub mod schema;
pub(crate) mod writer;

use crate::types::{Dte, DteTipo, EmisorConfig};
use crate::DteError;

/// Renderiza el `<DTE>` raíz sin firma (pre-TED). Despacha al renderer del
/// tipo correspondiente. El XML resultante NO es despachable a SII; sólo es
/// válido como input para `timbre::generate` y luego `sign::sign_xml`.
pub fn render_unsigned(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    match dte.tipo {
        DteTipo::BoletaElectronica => boleta::render(dte, emisor),
        DteTipo::FacturaElectronica => Err(DteError::XmlInvalid(
            "xml::render_unsigned tipo 33: pendiente subtask 9.1.f".to_string(),
        )),
        DteTipo::NotaDebito => Err(DteError::XmlInvalid(
            "xml::render_unsigned tipo 56: pendiente subtask 9.1.f".to_string(),
        )),
        DteTipo::NotaCredito => Err(DteError::XmlInvalid(
            "xml::render_unsigned tipo 61: pendiente subtask 9.1.f".to_string(),
        )),
        DteTipo::GuiaDespacho => Err(DteError::XmlInvalid(
            "xml::render_unsigned tipo 52: pendiente subtask 9.1.f".to_string(),
        )),
    }
}
