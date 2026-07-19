//! Guía de despacho electrónica tipo 52 (subtask 9.1.f). Reusa los builders
//! de `factura`; agrega `IndTraslado` obligatorio en `IdDoc`.

use crate::types::{Dte, EmisorConfig};
use crate::xml::factura::{expect_tipo, render_documento, require_receptor_completo};
use crate::{DteError, DteTipo};

/// Renderiza una guía de despacho 52 sin firma. `ind_traslado` es
/// obligatorio (1 venta, 2 venta por efectuar, 3 consignación, 4 entrega
/// gratuita, 5 traslado interno, 6 otros no venta, 7 guía devolución,
/// 8 traslado exportación, 9 venta exportación). Los totales pueden ir en
/// cero para traslados que no constituyen venta.
pub fn render(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    expect_tipo(dte, DteTipo::GuiaDespacho)?;
    require_receptor_completo(dte)?;
    match dte.ind_traslado {
        Some(1..=9) => {}
        _ => {
            return Err(DteError::XmlInvalid(format!(
                "render tipo {}: guía requiere ind_traslado entre 1 y 9 (motivo del traslado)",
                dte.tipo.code()
            )));
        }
    }
    render_documento(dte, emisor)
}
