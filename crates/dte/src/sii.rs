//! Envío de DTEs al endpoint SII (sandbox/prod) y polling de estado.
//!
//! Subtask 9.1.d (envío) y 9.1.e (polling) pendientes.

use crate::types::SiiEnv;
use crate::DteError;

/// Sube un XML DTE firmado al endpoint SII correspondiente al entorno.
/// Devuelve `track_id` cuando SII acepta el envío inicial.
pub async fn upload(_env: SiiEnv, _xml_firmado: &str) -> Result<i64, DteError> {
    Err(DteError::SiiNetwork(
        "sii::upload: pendiente subtask 9.1.d".to_string(),
    ))
}

/// Consulta el estado del envío `track_id` en SII.
pub async fn estado(_env: SiiEnv, _track_id: i64) -> Result<SiiEstado, DteError> {
    Err(DteError::SiiNetwork(
        "sii::estado: pendiente subtask 9.1.e".to_string(),
    ))
}

/// Estado devuelto por SII al consultar `track_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiiEstado {
    EnProceso,
    Aceptado,
    Rechazado { glosa: String },
}
