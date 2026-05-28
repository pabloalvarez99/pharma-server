//! Smoke tests para los tipos públicos del crate DTE. Verifica round-trip
//! JSON + helpers básicos. Lógica real va en tests por subtask (9.1.a-m).

use chrono::{Duration, TimeZone, Utc};
use dte::{Caf, CertDigital, DteEstado, DteTipo, SiiEnv};
use uuid::Uuid;

#[test]
fn dte_tipo_round_trip_codigo() {
    for tipo in [
        DteTipo::BoletaElectronica,
        DteTipo::FacturaElectronica,
        DteTipo::NotaDebito,
        DteTipo::NotaCredito,
        DteTipo::GuiaDespacho,
    ] {
        let code = tipo.code();
        let back = DteTipo::from_code(code).expect("código válido");
        assert_eq!(back, tipo);
    }
}

#[test]
fn dte_tipo_codigo_invalido_rechazado() {
    let err = DteTipo::from_code(99).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("99"),
        "mensaje debe mencionar el código: {msg}"
    );
}

#[test]
fn dte_estado_serializa_lowercase() {
    let json = serde_json::to_string(&DteEstado::Sent).unwrap();
    assert_eq!(json, "\"sent\"");
}

#[test]
fn caf_has_folios_respeta_rango_y_activo() {
    let mut caf = Caf {
        id: Uuid::nil(),
        tipo_dte: DteTipo::BoletaElectronica,
        folio_desde: 1,
        folio_hasta: 10,
        next_folio: 5,
        fecha_autorizacion: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        rut_emisor: "76.123.456-7".to_string(),
        xml: String::new(),
        activo: true,
    };
    assert!(caf.has_folios());

    caf.next_folio = 11;
    assert!(!caf.has_folios(), "next_folio > folio_hasta agota CAF");

    caf.next_folio = 5;
    caf.activo = false;
    assert!(!caf.has_folios(), "CAF inactivo no entrega folios");
}

#[test]
fn cert_is_valid_at_window() {
    let desde = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let hasta = desde + Duration::days(365);
    let cert = CertDigital {
        id: Uuid::nil(),
        rut_propietario: "11.111.111-1".to_string(),
        nombre_propietario: None,
        pfx_blob: vec![],
        password_encrypted: vec![],
        vigencia_desde: desde,
        vigencia_hasta: hasta,
        activo: true,
    };
    assert!(cert.is_valid_at(desde + Duration::days(10)));
    assert!(!cert.is_valid_at(desde - Duration::days(1)));
    assert!(!cert.is_valid_at(hasta + Duration::days(1)));
}

#[test]
fn sii_env_endpoints() {
    assert!(SiiEnv::Sandbox.upload_endpoint().contains("maullin.sii.cl"));
    assert!(SiiEnv::Prod.upload_endpoint().contains("palena.sii.cl"));
    assert_eq!(SiiEnv::default(), SiiEnv::Sandbox);
}
