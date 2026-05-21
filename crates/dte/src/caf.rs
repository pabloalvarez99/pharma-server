//! CAF parsing + folio assignment.
//!
//! Subtask 9.1.c pendiente. Aquí va el parser de XML CAF (formato SII) y la
//! transacción atómica de asignación de folio.

use crate::types::Caf;
use crate::DteError;

/// Parsea el XML CAF entregado por SII a la struct in-memory.
pub fn parse_xml(_xml: &str) -> Result<Caf, DteError> {
    Err(DteError::CafInvalid(
        "caf::parse_xml: pendiente subtask 9.1.c".to_string(),
    ))
}

/// Asigna atómicamente el próximo folio del CAF activo del tenant. Hace
/// `BEGIN; UPDATE caf SET next_folio = next_folio + 1 WHERE id = $caf RETURN
/// BEFORE; COMMIT` en SurrealDB.
pub async fn assign_next(_caf: &Caf) -> Result<i64, DteError> {
    Err(DteError::FolioExhausted { tipo: 0, folio: 0 })
}
