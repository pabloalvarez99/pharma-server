//! Subtask 9.1.g — libro de ventas mensual.

mod common;

use chrono::NaiveDate;
use dte::xml::libro::render_libro_ventas;
use dte::{DteEstado, DteTipo};
use pharma_core::tenant::TenantId;
use rust_decimal::Decimal;

use crate::common::{dte_boleta_minimal, emisor_test};

fn periodo() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()
}

#[test]
fn libro_vacio_emite_caratula_y_resumen_cero() {
    let tenant = TenantId::new("t1");
    let xml = render_libro_ventas(&emisor_test(), tenant, periodo(), &[]).expect("render libro");
    assert!(xml.contains("<RutEmisorLibro>76123456-7</RutEmisorLibro>"));
    assert!(xml.contains("<PeriodoTributario>2026-05</PeriodoTributario>"));
    assert!(xml.contains("<TipoOperacion>VENTA</TipoOperacion>"));
    // Resumen sin TotalesPeriodo (lista vacía); DocumentosLista sin Detalle.
    assert!(
        !xml.contains("<TotalesPeriodo>"),
        "no debe haber breakdown en libro vacío"
    );
    assert!(
        !xml.contains("<Detalle>"),
        "no debe haber detalle en libro vacío"
    );
}

#[test]
fn libro_con_dtes_aceptados_suma_totales_y_agrupa_por_tipo() {
    let tenant = TenantId::new("t1");
    let mut d1 = dte_boleta_minimal(1);
    d1.estado = DteEstado::Accepted;
    d1.monto_neto = Decimal::from(1000);
    d1.iva = Decimal::from(190);
    d1.monto_total = Decimal::from(1190);

    let mut d2 = dte_boleta_minimal(2);
    d2.estado = DteEstado::Accepted;
    d2.monto_neto = Decimal::from(2000);
    d2.iva = Decimal::from(380);
    d2.monto_total = Decimal::from(2380);

    // Factura tipo 33 — debe quedar en grupo separado.
    let mut d3 = dte_boleta_minimal(100);
    d3.tipo = DteTipo::FacturaElectronica;
    d3.estado = DteEstado::Accepted;
    d3.monto_neto = Decimal::from(5000);
    d3.iva = Decimal::from(950);
    d3.monto_total = Decimal::from(5950);

    // Cancelado → NO entra al libro.
    let mut d4 = dte_boleta_minimal(3);
    d4.estado = DteEstado::Cancelled;
    d4.monto_total = Decimal::from(9999);

    let dtes = vec![d1, d2, d3, d4];
    let xml = render_libro_ventas(&emisor_test(), tenant, periodo(), &dtes).expect("render libro");

    // Caratula presente.
    assert!(xml.contains("<PeriodoTributario>2026-05</PeriodoTributario>"));

    // Totales boleta 39: 2 docs, neto 3000, iva 570, total 3570.
    assert!(xml.contains("<TpoDoc>39</TpoDoc>"));
    assert!(xml.contains("<TotDoc>2</TotDoc>"));
    assert!(xml.contains("<TotMntNeto>3000</TotMntNeto>"));
    assert!(xml.contains("<TotMntIVA>570</TotMntIVA>"));
    assert!(xml.contains("<TotMntTotal>3570</TotMntTotal>"));

    // Totales factura 33: 1 doc, total 5950.
    assert!(xml.contains("<TpoDoc>33</TpoDoc>"));
    assert!(xml.contains("<TotMntTotal>5950</TotMntTotal>"));

    // Detalle: incluye folios 1, 2, 100 — no el cancelado (3).
    assert!(xml.contains("<NroDoc>1</NroDoc>"));
    assert!(xml.contains("<NroDoc>2</NroDoc>"));
    assert!(xml.contains("<NroDoc>100</NroDoc>"));
    assert!(
        !xml.contains("<MntTotal>9999</MntTotal>"),
        "DTE cancelado NO debe aparecer"
    );
}
