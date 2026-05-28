//! Subtask 9.1.j — gating del envío al SII por tier de licencia.
//!
//! Free = local-only (cualquier tipo bloqueado). Pro = boleta 39. Business+ =
//! multi-tipo (33/56/61/52). Ver `crates/dte/src/gating.rs`.

use dte::error::DteError;
use dte::gating::{min_tier_for, require_send_allowed, SendTier};
use dte::DteTipo;

#[test]
fn free_bloquea_boleta_39_requiere_pro() {
    let err = require_send_allowed(SendTier::Free, DteTipo::BoletaElectronica)
        .expect_err("Free no puede enviar al SII");
    match err {
        DteError::SendNotEntitled {
            tier,
            tipo,
            required_tier,
        } => {
            assert_eq!(tier, "free");
            assert_eq!(tipo, 39);
            assert_eq!(required_tier, "pro", "boleta requiere Pro");
        }
        other => panic!("variante inesperada: {other:?}"),
    }
}

#[test]
fn free_bloquea_factura_33_requiere_business() {
    let err = require_send_allowed(SendTier::Free, DteTipo::FacturaElectronica)
        .expect_err("Free no puede enviar al SII");
    match err {
        DteError::SendNotEntitled { required_tier, .. } => {
            assert_eq!(required_tier, "business", "factura requiere Business");
        }
        other => panic!("variante inesperada: {other:?}"),
    }
}

#[test]
fn pro_permite_boleta_39() {
    require_send_allowed(SendTier::Pro, DteTipo::BoletaElectronica).expect("Pro envía boleta 39");
}

#[test]
fn pro_bloquea_factura_nc_nd_requiere_business() {
    for tipo in [
        DteTipo::FacturaElectronica,
        DteTipo::NotaCredito,
        DteTipo::NotaDebito,
    ] {
        let err =
            require_send_allowed(SendTier::Pro, tipo).expect_err(&format!("Pro no envía {tipo:?}"));
        match err {
            DteError::SendNotEntitled {
                tier,
                tipo: t,
                required_tier,
            } => {
                assert_eq!(tier, "pro");
                assert_eq!(t, tipo.code());
                assert_eq!(required_tier, "business");
            }
            other => panic!("variante inesperada para {tipo:?}: {other:?}"),
        }
    }
}

#[test]
fn pro_bloquea_guia_52() {
    let err =
        require_send_allowed(SendTier::Pro, DteTipo::GuiaDespacho).expect_err("Pro no envía guía");
    assert!(matches!(err, DteError::SendNotEntitled { .. }));
}

#[test]
fn business_permite_factura_33_y_boleta_39() {
    require_send_allowed(SendTier::Business, DteTipo::FacturaElectronica)
        .expect("Business envía factura 33");
    require_send_allowed(SendTier::Business, DteTipo::BoletaElectronica)
        .expect("Business envía boleta 39");
}

#[test]
fn enterprise_permite_todos_los_tipos() {
    for tipo in [
        DteTipo::BoletaElectronica,
        DteTipo::FacturaElectronica,
        DteTipo::NotaCredito,
        DteTipo::NotaDebito,
        DteTipo::GuiaDespacho,
    ] {
        require_send_allowed(SendTier::Enterprise, tipo)
            .unwrap_or_else(|e| panic!("Enterprise debe enviar {tipo:?}: {e}"));
    }
}

#[test]
fn min_tier_for_mapping_39_vs_33() {
    assert_eq!(min_tier_for(DteTipo::BoletaElectronica), SendTier::Pro);
    assert_eq!(
        min_tier_for(DteTipo::FacturaElectronica),
        SendTier::Business
    );
    assert_eq!(min_tier_for(DteTipo::NotaCredito), SendTier::Business);
    assert_eq!(min_tier_for(DteTipo::NotaDebito), SendTier::Business);
    assert_eq!(min_tier_for(DteTipo::GuiaDespacho), SendTier::Business);
}

#[test]
fn error_lleva_tier_tipo_y_required_tier_correctos() {
    // Free intentando guía (52): tier=free, tipo=52, required=business.
    let err = require_send_allowed(SendTier::Free, DteTipo::GuiaDespacho).unwrap_err();
    let DteError::SendNotEntitled {
        tier,
        tipo,
        required_tier,
    } = err
    else {
        panic!("se esperaba SendNotEntitled");
    };
    assert_eq!(tier, "free");
    assert_eq!(tipo, 52);
    assert_eq!(required_tier, "business");
}
