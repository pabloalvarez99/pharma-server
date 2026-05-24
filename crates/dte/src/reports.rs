//! Cierres diarios X (parcial, no resetea) y Z (cierra día, resetea
//! contadores) — subtask 9.1.h.
//!
//! Reportes puros sobre `&[Dte]`. NO hacen I/O: el caller filtra (por
//! tenant + fecha) y entrega el slice; las funciones agregan totales,
//! folio range y breakdown por tipo DTE. El "reset" del cierre Z lo aplica
//! el caller (e.g. `crates/api` actualiza un contador de jornada en DB).
//!
//! Diferencia X vs Z:
//! - **X** (`x_report`): foto in-medio del día. Lo emite el cajero para
//!   ver ventas hasta el momento sin afectar el ciclo de la jornada.
//! - **Z** (`z_report`): cierre fiscal del día. Mismo cálculo que X pero
//!   marcado como cierre definitivo — el caller persiste y lockea el
//!   rango de folios consumidos. La función misma es idempotente; el
//!   semantic "reset" es responsabilidad del caller.
//!
//! Ambos toman sólo DTEs en estado `Accepted` (los `Rejected`/`Cancelled`
//! no cuentan en arqueo). El filtrado por fecha lo hace el caller.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::types::{Dte, DteEstado, DteTipo};

/// Tipo de cierre. Las funciones de cálculo son las mismas; el tipo viaja
/// en el struct resultado para que el caller sepa qué reporte emitir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    /// Cierre parcial. No resetea contadores.
    X,
    /// Cierre fiscal. El caller resetea contadores de jornada al persistir.
    Z,
}

/// Breakdown por tipo de DTE.
#[derive(Debug, Clone, PartialEq)]
pub struct TipoBreakdown {
    pub count: u32,
    pub monto_total: Decimal,
}

/// Resultado de un cierre X/Z.
#[derive(Debug, Clone)]
pub struct DailyReport {
    pub kind: ReportKind,
    pub fecha: NaiveDate,
    pub primer_folio: Option<i64>,
    pub ultimo_folio: Option<i64>,
    pub count_dtes: u32,
    pub monto_neto: Decimal,
    pub monto_iva: Decimal,
    pub monto_total: Decimal,
    pub por_tipo: BTreeMap<DteTipo, TipoBreakdown>,
}

/// Calcula el reporte. Funcionalmente igual a `z_report` salvo el flag.
pub fn x_report(fecha: NaiveDate, dtes: &[Dte]) -> DailyReport {
    build_report(ReportKind::X, fecha, dtes)
}

/// Calcula el cierre Z. El caller es responsable de persistir + resetear.
pub fn z_report(fecha: NaiveDate, dtes: &[Dte]) -> DailyReport {
    build_report(ReportKind::Z, fecha, dtes)
}

fn build_report(kind: ReportKind, fecha: NaiveDate, dtes: &[Dte]) -> DailyReport {
    let mut primer: Option<i64> = None;
    let mut ultimo: Option<i64> = None;
    let mut count: u32 = 0;
    let mut neto = Decimal::ZERO;
    let mut iva = Decimal::ZERO;
    let mut total = Decimal::ZERO;
    let mut por_tipo: BTreeMap<DteTipo, TipoBreakdown> = BTreeMap::new();

    for d in dtes.iter().filter(|d| d.estado == DteEstado::Accepted) {
        count += 1;
        neto += d.monto_neto;
        iva += d.iva;
        total += d.monto_total;
        primer = Some(primer.map_or(d.folio, |p| p.min(d.folio)));
        ultimo = Some(ultimo.map_or(d.folio, |p| p.max(d.folio)));
        let entry = por_tipo.entry(d.tipo).or_insert(TipoBreakdown {
            count: 0,
            monto_total: Decimal::ZERO,
        });
        entry.count += 1;
        entry.monto_total += d.monto_total;
    }

    DailyReport {
        kind,
        fecha,
        primer_folio: primer,
        ultimo_folio: ultimo,
        count_dtes: count,
        monto_neto: neto,
        monto_iva: iva,
        monto_total: total,
        por_tipo,
    }
}

// Necesario para que DteTipo sea key de BTreeMap.
impl PartialOrd for DteTipo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DteTipo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.code().cmp(&other.code())
    }
}
