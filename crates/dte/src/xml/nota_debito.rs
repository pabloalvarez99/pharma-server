//! Nota de débito electrónica tipo 56 (subtask 9.1.f). Misma estructura y
//! validaciones que la nota de crédito; sólo cambia el código de tipo.

use crate::types::{Dte, EmisorConfig};
use crate::xml::factura::{expect_tipo, render_documento, require_receptor_completo};
use crate::xml::nota_credito::require_referencias_nota;
use crate::{DteError, DteTipo};

/// Renderiza una nota de débito 56 sin firma. Exige ≥1 referencia con
/// `cod_ref`, igual que la nota de crédito.
pub fn render(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    expect_tipo(dte, DteTipo::NotaDebito)?;
    require_receptor_completo(dte)?;
    require_referencias_nota(dte)?;
    render_documento(dte, emisor)
}
