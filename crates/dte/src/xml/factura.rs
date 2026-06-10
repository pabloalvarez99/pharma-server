//! Factura electrónica tipo 33 — renderer XML SII xsd 1.0 (subtask 9.1.f).
//!
//! Además de `render` (33), este módulo expone los builders compartidos que
//! reusan notas (56/61) y guía de despacho (52): los cuatro tipos comparten
//! la misma estructura `Documento` (Encabezado con Receptor completo +
//! Detalle + Referencia opcional); difieren sólo en validaciones de entrada
//! (notas exigen referencias, guía exige `IndTraslado`).

use crate::types::{Dte, DteItem, EmisorConfig};
use crate::xml::boleta::{clp_int, decimal_str};
use crate::xml::schema::*;
use crate::xml::writer::to_xml_string;
use crate::{DteError, DteTipo};

/// Renderiza una factura electrónica 33 sin firma. Igual que boleta, el XML
/// resultante NO es despachable a SII hasta TED (9.1.b) + firma XML DSig
/// (9.1.b.2).
pub fn render(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    expect_tipo(dte, DteTipo::FacturaElectronica)?;
    require_receptor_completo(dte)?;
    // Factura afecta exige neto + IVA desglosados (a diferencia de boleta,
    // que admite modo monto-total). Una venta 100% exenta no va en tipo 33.
    if clp_int(dte.monto_neto) <= 0 || clp_int(dte.iva) <= 0 {
        return Err(DteError::XmlInvalid(format!(
            "render tipo {}: factura afecta requiere monto_neto e IVA desglosados (> 0)",
            dte.tipo.code()
        )));
    }
    render_documento(dte, emisor)
}

/// Valida que `dte.tipo` coincida con el tipo que el renderer espera.
pub(crate) fn expect_tipo(dte: &Dte, esperado: DteTipo) -> Result<(), DteError> {
    if dte.tipo != esperado {
        return Err(DteError::XmlInvalid(format!(
            "render tipo {}: tipo esperado {}, recibido {}",
            esperado.code(),
            esperado.code(),
            dte.tipo.code()
        )));
    }
    Ok(())
}

/// Los tipos 33/56/61/52 exigen identificación completa del receptor
/// (GiroRecep + DirRecep + CmnaRecep) — boleta 39 es el único que no.
pub(crate) fn require_receptor_completo(dte: &Dte) -> Result<(), DteError> {
    let falta_o_vacio = |v: &Option<String>| v.as_deref().is_none_or(|s| s.trim().is_empty());
    if falta_o_vacio(&dte.giro_receptor)
        || falta_o_vacio(&dte.direccion_receptor)
        || falta_o_vacio(&dte.comuna_receptor)
    {
        return Err(DteError::XmlInvalid(format!(
            "render tipo {}: receptor requiere giro, dirección y comuna (giro_receptor/direccion_receptor/comuna_receptor)",
            dte.tipo.code()
        )));
    }
    Ok(())
}

/// Arma y serializa el `<DTE>` para 33/56/61/52. Asume validaciones de tipo
/// ya hechas por el caller.
pub(crate) fn render_documento(dte: &Dte, emisor: &EmisorConfig) -> Result<String, DteError> {
    let id = format!("F{}T{}", dte.folio, dte.tipo.code());
    let doc = Documento {
        id,
        encabezado: build_encabezado(dte, emisor),
        detalle: dte.items.iter().map(build_detalle).collect(),
        referencia: build_referencias(dte),
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
            ind_traslado: dte.ind_traslado,
            ind_servicio: None,
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
            giro_recep: dte.giro_receptor.clone(),
            dir_recep: dte.direccion_receptor.clone(),
            cmna_recep: dte.comuna_receptor.clone(),
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
        ind_exe: item.exento.then_some(1),
        nmb_item: item.nombre.clone(),
        qty_item: decimal_str(item.cantidad),
        prc_item: decimal_str(item.precio_unitario),
        monto_item: clp_int(item.monto_item),
    }
}

fn build_referencias(dte: &Dte) -> Vec<Referencia> {
    dte.referencias
        .iter()
        .enumerate()
        .map(|(i, r)| Referencia {
            nro_lin_ref: (i + 1) as u32,
            tpo_doc_ref: r.tipo_doc_ref.clone(),
            folio_ref: r.folio_ref.clone(),
            fch_ref: r.fecha_ref.format("%Y-%m-%d").to_string(),
            cod_ref: r.cod_ref,
            razn_ref: r.razon_ref.clone(),
        })
        .collect()
}
