//! Subtask 9.1.f — transiciones cancel/resend.

mod common;

use dte::{cancel_dte, resend_dte, DteError, DteEstado};

use crate::common::dte_boleta_minimal;

fn dte_in(estado: DteEstado) -> dte::Dte {
    let mut d = dte_boleta_minimal(1);
    d.estado = estado;
    d
}

// ---------- válidas ----------

#[test]
fn cancel_from_draft_records_metadata_and_transitions() {
    let mut d = dte_in(DteEstado::Draft);
    cancel_dte(&mut d, "operador anula pre-envío").expect("cancel debe pasar");
    assert_eq!(d.estado, DteEstado::Cancelled);
    let meta = d.metadata.as_ref().expect("metadata escrita");
    assert_eq!(
        meta.get("cancelled_reason").and_then(|v| v.as_str()),
        Some("operador anula pre-envío")
    );
    assert!(meta.get("cancelled_at").and_then(|v| v.as_str()).is_some());
}

#[test]
fn cancel_from_sent_is_valid() {
    let mut d = dte_in(DteEstado::Sent);
    cancel_dte(&mut d, "envío SII fallido — operador anula").expect("cancel desde Sent OK");
    assert_eq!(d.estado, DteEstado::Cancelled);
}

#[test]
fn resend_from_rejected_to_signed_clears_track() {
    let mut d = dte_in(DteEstado::Rejected);
    d.track_id = Some(123_456);
    d.sii_glosa = Some("glosa rechazo".to_string());
    resend_dte(&mut d).expect("resend Rejected→Signed OK");
    assert_eq!(d.estado, DteEstado::Signed);
    assert!(d.track_id.is_none(), "track viejo limpiado");
    assert!(d.sii_glosa.is_none(), "glosa vieja limpiada");
    let meta = d.metadata.as_ref().expect("metadata escrita");
    assert!(meta.get("resent_at").is_some());
}

// ---------- inválidas ----------

#[test]
fn cancel_already_cancelled_fails() {
    let mut d = dte_in(DteEstado::Cancelled);
    let err = cancel_dte(&mut d, "intento doble cancel").expect_err("debe fallar");
    match err {
        DteError::InvalidStateTransition { from, to } => {
            assert_eq!(from, DteEstado::Cancelled);
            assert_eq!(to, DteEstado::Cancelled);
        }
        other => panic!("se esperaba InvalidStateTransition, got {other:?}"),
    }
    // estado intacto
    assert_eq!(d.estado, DteEstado::Cancelled);
}

#[test]
fn resend_from_accepted_fails() {
    let mut d = dte_in(DteEstado::Accepted);
    let err = resend_dte(&mut d).expect_err("Accepted no se reenvía");
    match err {
        DteError::InvalidStateTransition { from, to } => {
            assert_eq!(from, DteEstado::Accepted);
            assert_eq!(to, DteEstado::Signed);
        }
        other => panic!("se esperaba InvalidStateTransition, got {other:?}"),
    }
    assert_eq!(d.estado, DteEstado::Accepted);
}

#[test]
fn resend_from_draft_fails() {
    let mut d = dte_in(DteEstado::Draft);
    let err = resend_dte(&mut d).expect_err("Draft no se reenvía");
    assert!(matches!(err, DteError::InvalidStateTransition { .. }));
    assert_eq!(d.estado, DteEstado::Draft);
}

#[test]
fn cancel_empty_reason_fails() {
    let mut d = dte_in(DteEstado::Signed);
    let err = cancel_dte(&mut d, "   ").expect_err("reason vacío rechaza");
    assert!(matches!(err, DteError::XmlInvalid(_)));
    assert_eq!(d.estado, DteEstado::Signed);
}
