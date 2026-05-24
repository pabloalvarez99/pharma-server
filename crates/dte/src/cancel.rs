//! Transiciones de estado para DTE — cancel/resend (subtask 9.1.f).
//!
//! Operaciones admin sobre `Dte` in-memory. Persistencia (UPDATE en
//! SurrealDB) corre a cargo del caller (`crates/api` o CLI). Estas funciones
//! sólo validan la transición y registran el evento en `metadata`.
//!
//! Reglas:
//! - **Cancel**: cualquier estado distinto de `Cancelled` puede pasar a
//!   `Cancelled`. Requiere `reason` no-vacío. Idempotencia: cancelar un DTE
//!   ya cancelado retorna `InvalidStateTransition`.
//! - **Resend**: sólo `Rejected → Signed`. Reabre el DTE tras corregir el
//!   issue que reportó SII; el caller debe re-firmar / reenviar después.
//! - Cualquier otra transición administrativa (e.g. `Accepted → Signed`,
//!   `Cancelled → Signed`) está prohibida y retorna
//!   `DteError::InvalidStateTransition { from, to }`.

use chrono::Utc;
use serde_json::json;

use crate::types::{Dte, DteEstado};
use crate::DteError;

/// Cancela un DTE registrando `cancelled_at` + `cancelled_reason` en
/// `metadata`. Falla si el DTE ya estaba cancelado o si `reason` está vacío.
pub fn cancel_dte(dte: &mut Dte, reason: impl Into<String>) -> Result<(), DteError> {
    let reason: String = reason.into();
    if reason.trim().is_empty() {
        return Err(DteError::XmlInvalid(
            "cancel_dte: reason no puede ser vacío".to_string(),
        ));
    }
    let from = dte.estado;
    if from == DteEstado::Cancelled {
        return Err(DteError::InvalidStateTransition {
            from,
            to: DteEstado::Cancelled,
        });
    }
    let now = Utc::now();
    let event = json!({
        "cancelled_at": now.to_rfc3339(),
        "cancelled_reason": reason,
        "cancelled_from": format!("{from:?}"),
    });
    merge_metadata(dte, event);
    dte.estado = DteEstado::Cancelled;
    Ok(())
}

/// Reabre un DTE rechazado por SII para reintento. Sólo `Rejected → Signed`.
/// El caller debe (a) corregir lo que dijo SII, (b) re-firmar si cambió el
/// XML, (c) re-postear al endpoint `DTEUpload`.
pub fn resend_dte(dte: &mut Dte) -> Result<(), DteError> {
    let from = dte.estado;
    if from != DteEstado::Rejected {
        return Err(DteError::InvalidStateTransition {
            from,
            to: DteEstado::Signed,
        });
    }
    let now = Utc::now();
    let event = json!({
        "resent_at": now.to_rfc3339(),
        "resent_from": format!("{from:?}"),
    });
    merge_metadata(dte, event);
    dte.estado = DteEstado::Signed;
    // Limpiar track_id viejo + glosa (el reenvío genera nuevo track).
    dte.track_id = None;
    dte.sii_glosa = None;
    Ok(())
}

/// Mergea `event` sobre `dte.metadata`. Si ya había metadata como object,
/// extiende; si era otro tipo (poco esperable), lo reemplaza preservando el
/// previo bajo `"_prev"`. Append-only: nunca borra claves existentes.
fn merge_metadata(dte: &mut Dte, event: serde_json::Value) {
    let merged = match dte.metadata.take() {
        Some(serde_json::Value::Object(mut map)) => {
            if let serde_json::Value::Object(ev) = event {
                for (k, v) in ev {
                    map.insert(k, v);
                }
            }
            serde_json::Value::Object(map)
        }
        Some(other) => json!({ "_prev": other, "current": event }),
        None => event,
    };
    dte.metadata = Some(merged);
}
