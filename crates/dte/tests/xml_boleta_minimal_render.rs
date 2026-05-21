//! Subtask 9.1.a — render boleta 39 mínima. Verifica que el XML producido sea
//! determinístico y contenga los campos clave en orden.

mod common;

use common::{dte_boleta_minimal, emisor_test};
use dte::xml::render_unsigned;

#[test]
fn boleta_minimal_xml_contiene_campos_clave() {
    let dte = dte_boleta_minimal(1);
    let emisor = emisor_test();
    let xml = render_unsigned(&dte, &emisor).expect("render boleta 39");

    assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    assert!(xml.contains(r#"<DTE version="1.0">"#));
    assert!(xml.contains(r#"<Documento ID="F1T39">"#));
    assert!(xml.contains("<TipoDTE>39</TipoDTE>"));
    assert!(xml.contains("<Folio>1</Folio>"));
    assert!(xml.contains("<FchEmis>2026-05-21</FchEmis>"));
    assert!(xml.contains("<RUTEmisor>76123456-7</RUTEmisor>"));
    assert!(xml.contains("<RznSocEmisor>FARMACIA TEST SPA</RznSocEmisor>"));
    assert!(xml.contains("<GiroEmisor>FARMACIA</GiroEmisor>"));
    assert!(xml.contains("<Acteco>477310</Acteco>"));
    assert!(xml.contains("<DirOrigen>CALLE 123</DirOrigen>"));
    assert!(xml.contains("<CmnaOrigen>COQUIMBO</CmnaOrigen>"));
    assert!(xml.contains("<RUTRecep>66666666-6</RUTRecep>"));
    assert!(xml.contains("<RznSocRecep>SIN INFORMACION</RznSocRecep>"));
    assert!(xml.contains("<MntTotal>1000</MntTotal>"));
    assert!(xml.contains("<NroLinDet>1</NroLinDet>"));
    assert!(xml.contains("<NmbItem>Paracetamol 500mg</NmbItem>"));
    assert!(xml.contains("<QtyItem>2</QtyItem>"));
    assert!(xml.contains("<PrcItem>500</PrcItem>"));
    assert!(xml.contains("<MontoItem>1000</MontoItem>"));
    assert!(xml.contains("<TmstFirma>2026-05-21T10:30:00</TmstFirma>"));
}

#[test]
fn boleta_orden_canonical_emisor_antes_de_receptor() {
    let dte = dte_boleta_minimal(7);
    let emisor = emisor_test();
    let xml = render_unsigned(&dte, &emisor).unwrap();
    let pos_id = xml.find("<IdDoc>").unwrap();
    let pos_emisor = xml.find("<Emisor>").unwrap();
    let pos_receptor = xml.find("<Receptor>").unwrap();
    let pos_totales = xml.find("<Totales>").unwrap();
    let pos_detalle = xml.find("<Detalle>").unwrap();
    let pos_tmst = xml.find("<TmstFirma>").unwrap();
    assert!(
        pos_id < pos_emisor
            && pos_emisor < pos_receptor
            && pos_receptor < pos_totales
            && pos_totales < pos_detalle
            && pos_detalle < pos_tmst,
        "orden xsd violado: {pos_id} {pos_emisor} {pos_receptor} {pos_totales} {pos_detalle} {pos_tmst}"
    );
}

#[test]
fn boleta_tipo_distinto_rechazado() {
    let mut dte = dte_boleta_minimal(1);
    dte.tipo = dte::DteTipo::FacturaElectronica;
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    assert!(format!("{err}").contains("9.1.f") || format!("{err}").contains("33"));
}
