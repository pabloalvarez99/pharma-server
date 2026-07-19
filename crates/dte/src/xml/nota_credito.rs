//! Nota de crédito electrónica tipo 61 (subtask 9.1.f). Reusa los builders
//! de `factura`; agrega la validación de referencias obligatorias.

use crate::types::{Dte, EmisorConfig};
use crate::xml::factura::{expect_tipo, render_documento, require_receptor_completo};
use crate::{DteError, DteTipo};

/// Renderiza una nota de crédito 61 sin firma. Exige ≥1 referencia al
/// documento corregido/anulado, cada una con `cod_ref` (1 anula, 2 corrige
/// texto, 3 corrige montos).
pub fn render(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    expect_tipo(dte, DteTipo::NotaCredito)?;
    require_receptor_completo(dte)?;
    require_referencias_nota(dte)?;
    render_documento(dte, emisor)
}

/// Notas (56/61) referencian SIEMPRE el documento que corrigen: ≥1
/// `Referencia` con `CodRef` ∈ {1, 2, 3}.
pub(crate) fn require_referencias_nota(dte: &Dte) -> Result<(), DteError> {
    if dte.referencias.is_empty() {
        return Err(DteError::XmlInvalid(format!(
            "render tipo {}: nota requiere al menos una referencia al documento original",
            dte.tipo.code()
        )));
    }
    for (i, r) in dte.referencias.iter().enumerate() {
        match r.cod_ref {
            Some(1..=3) => {}
            _ => {
                return Err(DteError::XmlInvalid(format!(
                    "render tipo {}: referencia {} requiere cod_ref 1 (anula), 2 (corrige texto) o 3 (corrige montos)",
                    dte.tipo.code(),
                    i + 1
                )));
            }
        }
    }
    Ok(())
}
