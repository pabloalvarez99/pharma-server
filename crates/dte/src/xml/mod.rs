//! XML serializer for SII DTE schema (xsd 1.0).
//!
//! Boleta 39 (subtask 9.1.a) + factura 33, notas 56/61 y guía 52
//! (subtask 9.1.f).

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
        DteTipo::FacturaElectronica => factura::render(dte, emisor),
        DteTipo::NotaDebito => nota_debito::render(dte, emisor),
        DteTipo::NotaCredito => nota_credito::render(dte, emisor),
        DteTipo::GuiaDespacho => guia::render(dte, emisor),
    }
}
