//! Libro de Ventas mensual — XML `LibroCompraVenta` SII (subtask 9.1.g).
//!
//! Agrupa todos los DTEs en estado `Accepted` para un tenant + mes y los
//! emite como XML `LibroCompraVenta` con:
//! - `Caratula`: RUT emisor, período `YYYY-MM`, tipo `VENTA`, folio
//!   notificación (placeholder hasta que el caller lo provea).
//! - `Resumen`: por tipo DTE — count + monto_neto + monto_exento +
//!   monto_iva + monto_total.
//! - `DocumentosLista`: una fila por DTE con tipo, folio, fecha emisión
//!   (`YYYY-MM-DD`), RUT receptor, monto total.
//!
//! Schema SII oficial: <https://www.sii.cl/factura_electronica/formato_libros.htm>.
//! Implementación pragmática: usa los nombres de elementos que SII exige
//! pero NO firma el XML aquí (firma `EnvioLibro` vive en subtask 9.1.k).
//!
//! Multi-tenant: el filtrado por tenant lo hace el caller (los `Dte` que
//! entran ya pertenecen al tenant). `TenantId` viaja en la signatura para
//! que el caller no se confunda — no se serializa al XML.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::types::{Dte, DteEstado, DteTipo, EmisorConfig};
use crate::xml::writer::to_xml_string;
use crate::DteError;
use pharma_core::tenant::TenantId;
use rust_decimal::prelude::ToPrimitive;

#[derive(Debug, Serialize)]
#[serde(rename = "LibroCompraVenta")]
struct LibroXml {
    #[serde(rename = "EnvioLibro")]
    envio: EnvioLibro,
}

#[derive(Debug, Serialize)]
struct EnvioLibro {
    #[serde(rename = "@ID")]
    id: String,
    #[serde(rename = "Caratula")]
    caratula: Caratula,
    #[serde(rename = "ResumenPeriodo")]
    resumen: ResumenPeriodo,
    #[serde(rename = "DocumentosLista")]
    documentos: DocumentosLista,
}

#[derive(Debug, Serialize)]
struct Caratula {
    #[serde(rename = "RutEmisorLibro")]
    rut_emisor: String,
    #[serde(rename = "PeriodoTributario")]
    periodo: String,
    #[serde(rename = "TipoOperacion")]
    tipo_operacion: String,
    #[serde(rename = "TipoLibro")]
    tipo_libro: String,
    #[serde(rename = "TipoEnvio")]
    tipo_envio: String,
    #[serde(rename = "FolioNotificacion")]
    folio_notificacion: i64,
}

#[derive(Debug, Serialize)]
struct ResumenPeriodo {
    #[serde(rename = "TotalesPeriodo")]
    totales: Vec<TotalesPorTipo>,
}

#[derive(Debug, Serialize)]
struct TotalesPorTipo {
    #[serde(rename = "TpoDoc")]
    tpo_doc: i32,
    #[serde(rename = "TotDoc")]
    tot_doc: u32,
    #[serde(rename = "TotMntExe")]
    tot_mnt_exe: i64,
    #[serde(rename = "TotMntNeto")]
    tot_mnt_neto: i64,
    #[serde(rename = "TotMntIVA")]
    tot_mnt_iva: i64,
    #[serde(rename = "TotMntTotal")]
    tot_mnt_total: i64,
}

#[derive(Debug, Serialize)]
struct DocumentosLista {
    #[serde(rename = "Detalle")]
    detalle: Vec<DetalleDoc>,
}

#[derive(Debug, Serialize)]
struct DetalleDoc {
    #[serde(rename = "TpoDoc")]
    tpo_doc: i32,
    #[serde(rename = "NroDoc")]
    nro_doc: i64,
    #[serde(rename = "FchDoc")]
    fch_doc: String,
    #[serde(rename = "RutDoc")]
    rut_doc: String,
    #[serde(rename = "MntTotal")]
    mnt_total: i64,
}

/// Renderiza el Libro de Ventas para `tenant` en el mes de `periodo`
/// (cualquier día del mes; sólo se usa `year`/`month`). Sólo entran DTEs
/// con `estado == Accepted`. Lista vacía produce caratula + resumen vacío
/// + documentos vacío (libro "sin movimientos" — SII lo acepta).
pub fn render_libro_ventas(
    emisor: &EmisorConfig,
    _tenant: TenantId,
    periodo: NaiveDate,
    dtes: &[Dte],
) -> Result<String, DteError> {
    let periodo_str = periodo.format("%Y-%m").to_string();

    // Agrupar por tipo DTE — orden estable por código SII.
    let mut by_tipo: BTreeMap<DteTipo, (u32, Decimal, Decimal, Decimal, Decimal)> = BTreeMap::new();
    let mut detalle: Vec<DetalleDoc> = Vec::new();

    for d in dtes.iter().filter(|d| d.estado == DteEstado::Accepted) {
        let entry = by_tipo.entry(d.tipo).or_insert((
            0,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ));
        entry.0 += 1;
        entry.1 += d.monto_exento;
        entry.2 += d.monto_neto;
        entry.3 += d.iva;
        entry.4 += d.monto_total;

        detalle.push(DetalleDoc {
            tpo_doc: d.tipo.code(),
            nro_doc: d.folio,
            fch_doc: d.fecha_emision.format("%Y-%m-%d").to_string(),
            rut_doc: d.rut_receptor.clone(),
            mnt_total: clp_int(d.monto_total),
        });
    }

    let totales: Vec<TotalesPorTipo> = by_tipo
        .into_iter()
        .map(|(tipo, (count, exe, neto, iva, total))| TotalesPorTipo {
            tpo_doc: tipo.code(),
            tot_doc: count,
            tot_mnt_exe: clp_int(exe),
            tot_mnt_neto: clp_int(neto),
            tot_mnt_iva: clp_int(iva),
            tot_mnt_total: clp_int(total),
        })
        .collect();

    let libro = LibroXml {
        envio: EnvioLibro {
            id: format!("LV{}", periodo_str.replace('-', "")),
            caratula: Caratula {
                rut_emisor: emisor.rut.clone(),
                periodo: periodo_str,
                tipo_operacion: "VENTA".to_string(),
                tipo_libro: "MENSUAL".to_string(),
                tipo_envio: "TOTAL".to_string(),
                folio_notificacion: 1,
            },
            resumen: ResumenPeriodo { totales },
            documentos: DocumentosLista { detalle },
        },
    };

    to_xml_string(&libro)
}

fn clp_int(d: Decimal) -> i64 {
    d.trunc().to_i64().unwrap_or(0)
}
