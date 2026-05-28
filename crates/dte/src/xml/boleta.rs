//! Boleta electrónica tipo 39 — renderer XML SII xsd 1.0 (subtask 9.1.a).
//!
//! Convierte `Dte` + `EmisorConfig` a la estructura `schema::DteXml` y
//! delega serialización a `writer::to_xml_string`. Sin TED ni firma DSig
//! (esos van en 9.1.b y 9.1.b.2 respectivamente).

use crate::types::{Dte, DteItem, EmisorConfig};
use crate::xml::schema::*;
use crate::xml::writer::to_xml_string;
use crate::DteError;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Renderiza una boleta 39 sin firma. El XML resultante NO es despachable a
/// SII hasta que pasen TED (9.1.b) y firma XML DSig (9.1.b.2). Usable para
/// preview y para emisión local (Free tier).
pub fn render(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    if dte.tipo != crate::DteTipo::BoletaElectronica {
        return Err(DteError::XmlInvalid(format!(
            "render boleta: tipo esperado 39, recibido {}",
            dte.tipo.code()
        )));
    }
    let id = format!("F{}T{}", dte.folio, dte.tipo.code());
    let doc = Documento {
        id,
        encabezado: build_encabezado(dte, emisor),
        detalle: dte.items.iter().map(build_detalle).collect(),
        tmst_firma: dte.fecha_emision.format("%Y-%m-%dT%H:%M:%S").to_string(),
    };
    let root = DteXml {
        version: "1.0".to_string(),
        documento: doc,
    };
    to_xml_string(&root)
}

fn build_encabezado(dte: &Dte, emisor: &EmisorConfig) -> Encabezado {
    let neto = clp_int(dte.monto_neto);
    let iva = clp_int(dte.iva);
    let exento = clp_int(dte.monto_exento);
    Encabezado {
        id_doc: IdDoc {
            tipo_dte: dte.tipo.code(),
            folio: dte.folio,
            fch_emis: dte.fecha_emision.format("%Y-%m-%d").to_string(),
            // Boleta retail farmacia: venta de bienes/servicios = 3. SII
            // recomienda omitir si MntTotal == MntExe; conservador: incluir.
            ind_servicio: Some(3),
        },
        emisor: Emisor {
            rut_emisor: emisor.rut.clone(),
            rzn_soc_emisor: emisor.razon_social.clone(),
            giro_emisor: emisor.giro.clone(),
            acteco: emisor.acteco,
            dir_origen: emisor.direccion.clone(),
            cmna_origen: emisor.comuna.clone(),
            ciudad_origen: emisor.ciudad.clone(),
        },
        receptor: Receptor {
            rut_recep: dte.rut_receptor.clone(),
            rzn_soc_recep: dte.razon_social_receptor.clone(),
        },
        totales: Totales {
            mnt_neto: (neto > 0).then_some(neto),
            mnt_exe: (exento > 0).then_some(exento),
            tasa_iva: (iva > 0).then_some(19),
            iva: (iva > 0).then_some(iva),
            mnt_total: clp_int(dte.monto_total),
        },
    }
}

fn build_detalle(item: &DteItem) -> Detalle {
    Detalle {
        nro_lin_det: item.nro_linea,
        nmb_item: item.nombre.clone(),
        qty_item: decimal_str(item.cantidad),
        prc_item: decimal_str(item.precio_unitario),
        monto_item: clp_int(item.monto_item),
    }
}

/// CLP siempre entero. Trunca decimales (SII redondea internamente, pero el
/// caller debe entregar montos ya cuadrados — neto + iva == total).
fn clp_int(d: Decimal) -> i64 {
    d.trunc().to_i64().unwrap_or(0)
}

/// Cantidades/precios admiten decimales en SII (xsd `PrcItem` xs:decimal).
/// Formato normalizado: sin trailing zeros.
fn decimal_str(d: Decimal) -> String {
    d.normalize().to_string()
}
