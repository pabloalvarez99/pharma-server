//! Subtask 9.1.c — parser CAF XML.

mod common;

use common::caf_xml_synthetic;
use dte::caf::parse_xml;
use dte::DteTipo;

#[test]
fn caf_parse_extrae_campos_basicos() {
    let (xml, _) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 1000);
    let caf = parse_xml(&xml).expect("parse CAF válido");
    assert_eq!(caf.tipo_dte, DteTipo::BoletaElectronica);
    assert_eq!(caf.folio_desde, 1);
    assert_eq!(caf.folio_hasta, 1000);
    assert_eq!(caf.next_folio, 1, "next_folio inicia en folio_desde");
    assert_eq!(caf.rut_emisor, "76123456-7");
    assert!(caf.activo);
    assert!(caf.xml.contains("<AUTORIZACION>"), "xml entero se conserva");
}

#[test]
fn caf_parse_xml_invalido_rechazado() {
    let err = parse_xml("<not-caf/>").unwrap_err();
    assert!(format!("{err}").contains("inválido") || format!("{err}").contains("CAF"));
}

#[test]
fn caf_parse_rng_invertido_rechazado() {
    // El helper synthetic respeta los args; pasarle desde > hasta crea XML
    // con D>H, que el parser debe rechazar.
    let (xml, _) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 100, 1);
    let err = parse_xml(&xml).unwrap_err();
    assert!(format!("{err}").contains("rango"));
}

#[test]
fn caf_parse_tipo_dte_no_soportado() {
    let (xml, _) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    let xml_mod = xml.replace("<TD>39</TD>", "<TD>99</TD>");
    let err = parse_xml(&xml_mod).unwrap_err();
    assert!(format!("{err}").contains("99") || format!("{err}").contains("no soportado"));
}
