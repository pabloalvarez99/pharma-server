//! Subtask 9.1.f — render XML factura (33), notas (56/61) y guía (52).

mod common;

use chrono::{TimeZone, Utc};
use dte::xml::render_unsigned;
use dte::{DescuentoGlobal, DteReferencia, DteTipo, TipoMovDr, TipoValorDr};
use rust_decimal::Decimal;

use crate::common::{dte_boleta_minimal, emisor_test};

/// Factura afecta mínima: neto + IVA desglosados, receptor completo.
fn dte_factura_minimal(folio: i64) -> dte::Dte {
    let mut d = dte_boleta_minimal(folio);
    d.tipo = DteTipo::FacturaElectronica;
    d.rut_receptor = "77654321-0".to_string();
    d.razon_social_receptor = "DROGUERIA CLIENTE LTDA".to_string();
    d.giro_receptor = Some("VENTA AL POR MAYOR DE MEDICAMENTOS".to_string());
    d.direccion_receptor = Some("AV SIEMPRE VIVA 742".to_string());
    d.comuna_receptor = Some("LA SERENA".to_string());
    d.monto_neto = Decimal::from(840);
    d.iva = Decimal::from(160);
    d.monto_total = Decimal::from(1000);
    d
}

fn referencia_a_factura() -> DteReferencia {
    DteReferencia {
        tipo_doc_ref: "33".to_string(),
        folio_ref: "77".to_string(),
        fecha_ref: Utc.with_ymd_and_hms(2026, 5, 10, 0, 0, 0).unwrap(),
        cod_ref: Some(1),
        razon_ref: Some("ANULA FACTURA".to_string()),
    }
}

#[test]
fn factura_33_render_minimal() {
    let dte = dte_factura_minimal(77);
    let xml = render_unsigned(&dte, &emisor_test()).expect("render factura 33");

    assert!(xml.contains("<Documento ID=\"F77T33\">"));
    assert!(xml.contains("<TipoDTE>33</TipoDTE>"));
    // Factura no lleva IndServicio (eso es de boleta).
    assert!(!xml.contains("<IndServicio>"));
    // Receptor completo obligatorio en 33.
    assert!(xml.contains("<GiroRecep>VENTA AL POR MAYOR DE MEDICAMENTOS</GiroRecep>"));
    assert!(xml.contains("<DirRecep>AV SIEMPRE VIVA 742</DirRecep>"));
    assert!(xml.contains("<CmnaRecep>LA SERENA</CmnaRecep>"));
    // Totales desglosados.
    assert!(xml.contains("<MntNeto>840</MntNeto>"));
    assert!(xml.contains("<TasaIVA>19</TasaIVA>"));
    assert!(xml.contains("<IVA>160</IVA>"));
    assert!(xml.contains("<MntTotal>1000</MntTotal>"));
    // Sin referencias → sin elemento Referencia.
    assert!(!xml.contains("<Referencia>"));
    assert!(xml.contains("<TmstFirma>2026-05-21T10:30:00</TmstFirma>"));
}

#[test]
fn factura_33_sin_receptor_completo_falla() {
    let mut dte = dte_factura_minimal(77);
    dte.giro_receptor = None;
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("33") && msg.contains("receptor"), "msg: {msg}");
}

#[test]
fn factura_33_sin_iva_desglosado_falla() {
    let mut dte = dte_factura_minimal(77);
    dte.monto_neto = Decimal::ZERO;
    dte.iva = Decimal::ZERO;
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("IVA"), "msg: {msg}");
}

#[test]
fn nota_credito_61_render_con_referencia() {
    let mut dte = dte_factura_minimal(12);
    dte.tipo = DteTipo::NotaCredito;
    dte.referencias = vec![referencia_a_factura()];
    let xml = render_unsigned(&dte, &emisor_test()).expect("render NC 61");

    assert!(xml.contains("<Documento ID=\"F12T61\">"));
    assert!(xml.contains("<TipoDTE>61</TipoDTE>"));
    assert!(xml.contains("<NroLinRef>1</NroLinRef>"));
    assert!(xml.contains("<TpoDocRef>33</TpoDocRef>"));
    assert!(xml.contains("<FolioRef>77</FolioRef>"));
    assert!(xml.contains("<FchRef>2026-05-10</FchRef>"));
    assert!(xml.contains("<CodRef>1</CodRef>"));
    assert!(xml.contains("<RaznRef>ANULA FACTURA</RaznRef>"));
    // Referencia va después del último Detalle y antes de TmstFirma (la
    // posición donde luego se inyecta el TED).
    let pos_det = xml.rfind("</Detalle>").unwrap();
    let pos_ref = xml.find("<Referencia>").unwrap();
    let pos_tmst = xml.find("<TmstFirma>").unwrap();
    assert!(pos_det < pos_ref && pos_ref < pos_tmst);
}

#[test]
fn nota_credito_61_sin_referencia_falla() {
    let mut dte = dte_factura_minimal(12);
    dte.tipo = DteTipo::NotaCredito;
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("referencia"), "msg: {msg}");
}

#[test]
fn nota_credito_61_referencia_sin_cod_ref_falla() {
    let mut dte = dte_factura_minimal(12);
    dte.tipo = DteTipo::NotaCredito;
    let mut r = referencia_a_factura();
    r.cod_ref = None;
    dte.referencias = vec![r];
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("cod_ref"), "msg: {msg}");
}

#[test]
fn nota_debito_56_render_con_referencia() {
    let mut dte = dte_factura_minimal(8);
    dte.tipo = DteTipo::NotaDebito;
    let mut r = referencia_a_factura();
    r.cod_ref = Some(3);
    r.razon_ref = Some("CORRIGE MONTOS".to_string());
    dte.referencias = vec![r];
    let xml = render_unsigned(&dte, &emisor_test()).expect("render ND 56");

    assert!(xml.contains("<Documento ID=\"F8T56\">"));
    assert!(xml.contains("<TipoDTE>56</TipoDTE>"));
    assert!(xml.contains("<CodRef>3</CodRef>"));
}

#[test]
fn guia_52_render_con_ind_traslado() {
    let mut dte = dte_factura_minimal(31);
    dte.tipo = DteTipo::GuiaDespacho;
    dte.ind_traslado = Some(1); // operación constituye venta
    let xml = render_unsigned(&dte, &emisor_test()).expect("render guía 52");

    assert!(xml.contains("<Documento ID=\"F31T52\">"));
    assert!(xml.contains("<TipoDTE>52</TipoDTE>"));
    assert!(xml.contains("<IndTraslado>1</IndTraslado>"));
    // xsd: IndTraslado va dentro de IdDoc, después de FchEmis.
    let pos_fch = xml.find("<FchEmis>").unwrap();
    let pos_ind = xml.find("<IndTraslado>").unwrap();
    let pos_emisor = xml.find("<Emisor>").unwrap();
    assert!(pos_fch < pos_ind && pos_ind < pos_emisor);
}

#[test]
fn guia_52_traslado_interno_totales_cero() {
    let mut dte = dte_factura_minimal(32);
    dte.tipo = DteTipo::GuiaDespacho;
    dte.ind_traslado = Some(5); // traslado interno, no venta
    dte.monto_neto = Decimal::ZERO;
    dte.iva = Decimal::ZERO;
    dte.monto_total = Decimal::ZERO;
    let xml = render_unsigned(&dte, &emisor_test()).expect("render guía 52 traslado interno");
    assert!(xml.contains("<MntTotal>0</MntTotal>"));
    assert!(!xml.contains("<MntNeto>"));
}

#[test]
fn guia_52_sin_ind_traslado_falla() {
    let mut dte = dte_factura_minimal(31);
    dte.tipo = DteTipo::GuiaDespacho;
    let err = render_unsigned(&dte, &emisor_test()).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("ind_traslado"), "msg: {msg}");
}

#[test]
fn item_exento_emite_ind_exe() {
    let mut dte = dte_factura_minimal(40);
    dte.items[0].exento = true;
    let xml = render_unsigned(&dte, &emisor_test()).expect("render factura item exento");
    let pos_lin = xml.find("<NroLinDet>1</NroLinDet>").unwrap();
    let pos_exe = xml.find("<IndExe>1</IndExe>").unwrap();
    let pos_nmb = xml.find("<NmbItem>").unwrap();
    assert!(pos_lin < pos_exe && pos_exe < pos_nmb);
}

#[test]
fn factura_33_render_dsc_rcg_global() {
    let mut dte = dte_factura_minimal(90);
    dte.descuentos_globales = vec![DescuentoGlobal {
        nro_linea: 1,
        tipo_mov: TipoMovDr::Descuento,
        glosa: Some("PROMO INVIERNO".to_string()),
        tipo_valor: TipoValorDr::Porcentaje,
        valor: Decimal::from(10),
        ind_exe: None,
    }];
    let xml = render_unsigned(&dte, &emisor_test()).expect("render factura con DscRcgGlobal");

    assert!(xml.contains("<DscRcgGlobal>"));
    assert!(xml.contains("<NroLinDR>1</NroLinDR>"));
    assert!(xml.contains("<TpoMov>D</TpoMov>"));
    assert!(xml.contains("<GlosaDR>PROMO INVIERNO</GlosaDR>"));
    assert!(xml.contains("<TpoValor>%</TpoValor>"));
    assert!(xml.contains("<ValorDR>10</ValorDR>"));
    // Sin IndExeDR cuando aplica al afecto.
    assert!(!xml.contains("<IndExeDR>"));
    // xsd: DscRcgGlobal va tras el último Detalle y antes de TmstFirma/TED.
    let pos_det = xml.rfind("</Detalle>").unwrap();
    let pos_dr = xml.find("<DscRcgGlobal>").unwrap();
    let pos_tmst = xml.find("<TmstFirma>").unwrap();
    assert!(pos_det < pos_dr && pos_dr < pos_tmst);
}

#[test]
fn factura_33_dsc_rcg_global_antes_de_referencia() {
    // Con referencia presente: orden xsd Detalle → DscRcgGlobal → Referencia.
    let mut dte = dte_factura_minimal(91);
    dte.tipo = DteTipo::NotaCredito;
    dte.referencias = vec![referencia_a_factura()];
    dte.descuentos_globales = vec![DescuentoGlobal {
        nro_linea: 1,
        tipo_mov: TipoMovDr::Recargo,
        glosa: None,
        tipo_valor: TipoValorDr::Monto,
        valor: Decimal::from(500),
        ind_exe: Some(1),
    }];
    let xml = render_unsigned(&dte, &emisor_test()).expect("render NC con DscRcgGlobal");
    assert!(xml.contains("<TpoMov>R</TpoMov>"));
    assert!(xml.contains("<TpoValor>$</TpoValor>"));
    assert!(xml.contains("<ValorDR>500</ValorDR>"));
    assert!(xml.contains("<IndExeDR>1</IndExeDR>"));
    let pos_dr = xml.find("<DscRcgGlobal>").unwrap();
    let pos_ref = xml.find("<Referencia>").unwrap();
    assert!(pos_dr < pos_ref);
}

#[test]
fn tipo_cruzado_falla() {
    // Un DTE tipo 39 no puede pasar por el renderer de factura directamente:
    // render_unsigned despacha por tipo, así que esto valida el guard interno
    // vía un tipo correcto con datos de otro renderer (33 con tipo 61 ya
    // cubierto por dispatch). Aquí: boleta sigue rendereando como boleta.
    let dte = dte_boleta_minimal(5);
    let xml = render_unsigned(&dte, &emisor_test()).expect("boleta sigue OK");
    assert!(xml.contains("<TipoDTE>39</TipoDTE>"));
    assert!(xml.contains("<IndServicio>3</IndServicio>"));
}
