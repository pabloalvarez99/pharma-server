//! Subtask 9.1.h — cierres X / Z.

mod common;

use chrono::NaiveDate;
use dte::{x_report, z_report, DteEstado, DteTipo, ReportKind};
use rust_decimal::Decimal;

use crate::common::dte_boleta_minimal;

fn fecha() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
}

#[test]
fn x_report_agrega_aceptados_y_calcula_folio_range() {
    let mut d1 = dte_boleta_minimal(10);
    d1.estado = DteEstado::Accepted;
    d1.monto_neto = Decimal::from(1000);
    d1.iva = Decimal::from(190);
    d1.monto_total = Decimal::from(1190);

    let mut d2 = dte_boleta_minimal(11);
    d2.estado = DteEstado::Accepted;
    d2.monto_neto = Decimal::from(500);
    d2.iva = Decimal::from(95);
    d2.monto_total = Decimal::from(595);

    let mut d3 = dte_boleta_minimal(12);
    d3.tipo = DteTipo::FacturaElectronica;
    d3.estado = DteEstado::Accepted;
    d3.monto_neto = Decimal::from(2000);
    d3.iva = Decimal::from(380);
    d3.monto_total = Decimal::from(2380);

    // Rechazado: no cuenta.
    let mut d4 = dte_boleta_minimal(13);
    d4.estado = DteEstado::Rejected;
    d4.monto_total = Decimal::from(9999);

    let r = x_report(fecha(), &[d1, d2, d3, d4]);
    assert_eq!(r.kind, ReportKind::X);
    assert_eq!(r.fecha, fecha());
    assert_eq!(r.count_dtes, 3);
    assert_eq!(r.primer_folio, Some(10));
    assert_eq!(r.ultimo_folio, Some(12));
    assert_eq!(r.monto_neto, Decimal::from(3500));
    assert_eq!(r.monto_iva, Decimal::from(665));
    assert_eq!(r.monto_total, Decimal::from(4165));
    // Breakdown por tipo.
    let boleta = r.por_tipo.get(&DteTipo::BoletaElectronica).unwrap();
    assert_eq!(boleta.count, 2);
    assert_eq!(boleta.monto_total, Decimal::from(1785));
    let factura = r.por_tipo.get(&DteTipo::FacturaElectronica).unwrap();
    assert_eq!(factura.count, 1);
    assert_eq!(factura.monto_total, Decimal::from(2380));
}

#[test]
fn z_report_marca_kind_z_y_es_idempotente_sin_dtes() {
    let r = z_report(fecha(), &[]);
    assert_eq!(r.kind, ReportKind::Z);
    assert_eq!(r.fecha, fecha());
    assert_eq!(r.count_dtes, 0);
    assert!(r.primer_folio.is_none());
    assert!(r.ultimo_folio.is_none());
    assert_eq!(r.monto_total, Decimal::ZERO);
    assert!(r.por_tipo.is_empty());
}
