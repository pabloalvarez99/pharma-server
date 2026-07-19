//! Subtask 9.1.b — TED sign + verify roundtrip y tamper detection.

mod common;

use common::{caf_xml_synthetic, dte_boleta_minimal};
use dte::timbre::{generate, verify};
use dte::DteTipo;

#[test]
fn ted_roundtrip_se_verifica() {
    let (_, caf) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    let dte = dte_boleta_minimal(1);
    let ted = generate(&dte, &caf).expect("generar TED");
    assert!(ted.contains(r#"<TED version="1.0">"#));
    assert!(ted.contains("<DD>"));
    assert!(ted.contains("</DD>"));
    assert!(ted.contains(r#"<FRMT algoritmo="SHA1withRSA">"#));
    verify(&dte, &caf, &ted).expect("verify TED ok");
}

#[test]
fn ted_tamper_dd_invalida_firma() {
    let (_, caf) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    let dte = dte_boleta_minimal(1);
    let ted = generate(&dte, &caf).unwrap();
    // Tamper: cambiar el RR (rut receptor) en DD post-firma.
    let tampered = ted.replace("<RR>66666666-6</RR>", "<RR>99999999-9</RR>");
    let err = verify(&dte, &caf, &tampered).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("no coincide") || msg.contains("verificar"),
        "esperado falla por DD modificado, msg: {msg}"
    );
}

#[test]
fn ted_folio_fuera_de_rango_rechazado() {
    let (_, caf) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    let mut dte = dte_boleta_minimal(101);
    dte.folio = 101;
    let err = generate(&dte, &caf).unwrap_err();
    assert!(format!("{err}").contains("rango") || format!("{err}").contains("CAF"));
}

#[test]
fn ted_tipo_dte_no_coincide_rechazado() {
    let (_, mut caf) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    caf.tipo_dte = DteTipo::FacturaElectronica;
    let dte = dte_boleta_minimal(1);
    let err = generate(&dte, &caf).unwrap_err();
    assert!(format!("{err}").contains("no coincide"));
}
