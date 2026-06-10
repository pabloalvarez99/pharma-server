//! Construcción de un `Dte` de factura/notas/guía (33/56/61/52) desde una
//! especificación de entrada — compartido por la API
//! (`POST /api/v1/dte/documentos`) y la CLI (`pharma dte emit-doc`) para que
//! la matemática de IVA y las validaciones no diverjan.
//!
//! El spec trae items con precios IVA-incluido (convención retail CL); acá se
//! calculan los montos por línea (CLP entero, truncado), el desglose
//! neto/IVA del total afecto y el exento aparte. Las validaciones
//! estructurales por tipo (receptor completo, referencias de notas,
//! `IndTraslado` de guía) viven en los renderers de `xml::*` y corren al
//! renderizar — este módulo valida lo previo: tipo soportado, items no
//! vacíos, cantidades y precios sanos.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::types::{Dte, DteEstado, DteItem, DteReferencia, DteTipo};
use crate::DteError;

/// Línea de entrada. `precio_unitario` IVA-incluido; `exento` marca la línea
/// fuera del desglose de IVA.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemSpec {
    pub nombre: String,
    pub cantidad: Decimal,
    pub precio_unitario: Decimal,
    #[serde(default)]
    pub exento: bool,
}

/// Referencia a otro documento (obligatoria en notas 56/61).
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenciaSpec {
    pub tipo_doc_ref: String,
    pub folio_ref: String,
    /// Fecha del documento referenciado.
    pub fecha_ref: DateTime<Utc>,
    #[serde(default)]
    pub cod_ref: Option<i32>,
    #[serde(default)]
    pub razon_ref: Option<String>,
}

/// Receptor completo (33/56/61/52 lo exigen; ver `xml::factura`).
#[derive(Debug, Clone, Deserialize)]
pub struct ReceptorSpec {
    pub rut: String,
    pub razon_social: String,
    pub giro: String,
    pub direccion: String,
    pub comuna: String,
}

/// Especificación de un documento a emitir.
#[derive(Debug, Clone)]
pub struct DocumentoSpec {
    pub tipo: DteTipo,
    pub receptor: ReceptorSpec,
    pub items: Vec<ItemSpec>,
    pub referencias: Vec<ReferenciaSpec>,
    pub ind_traslado: Option<i32>,
}

/// IVA CL 19%: neto = round(afecto / 1.19, half-up), IVA = afecto − neto
/// (convención SII: el IVA absorbe el redondeo, neto + IVA == total afecto).
pub fn desglose_iva(total_afecto: Decimal) -> (Decimal, Decimal) {
    use rust_decimal::RoundingStrategy;
    let tasa = Decimal::new(119, 2); // 1.19
    let neto =
        (total_afecto / tasa).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    (neto, total_afecto - neto)
}

/// Valida el spec y arma el `Dte` in-memory con `folio` ya asignado
/// (el caller hace `caf::assign_next` antes). El resultado queda en estado
/// `Draft`, listo para `sign::build_signed_dte`.
pub fn build_documento(
    spec: &DocumentoSpec,
    folio: i64,
    rut_emisor: &str,
    fecha_emision: DateTime<Utc>,
) -> Result<Dte, DteError> {
    if spec.tipo == DteTipo::BoletaElectronica {
        return Err(DteError::XmlInvalid(
            "la boleta (39) se emite desde su orden POS, no como documento genérico".to_string(),
        ));
    }
    if spec.items.is_empty() {
        return Err(DteError::XmlInvalid(
            "el documento requiere al menos un item".to_string(),
        ));
    }
    let mut afecto = Decimal::ZERO;
    let mut exento = Decimal::ZERO;
    let mut items: Vec<DteItem> = Vec::with_capacity(spec.items.len());
    for (i, it) in spec.items.iter().enumerate() {
        if it.nombre.trim().is_empty() {
            return Err(DteError::XmlInvalid(format!(
                "item {}: nombre vacío",
                i + 1
            )));
        }
        if it.cantidad <= Decimal::ZERO {
            return Err(DteError::XmlInvalid(format!(
                "item {}: cantidad debe ser > 0",
                i + 1
            )));
        }
        if it.precio_unitario < Decimal::ZERO {
            return Err(DteError::XmlInvalid(format!(
                "item {}: precio_unitario no puede ser negativo",
                i + 1
            )));
        }
        let monto = (it.cantidad * it.precio_unitario).trunc();
        if it.exento {
            exento += monto;
        } else {
            afecto += monto;
        }
        items.push(DteItem {
            nro_linea: (i + 1) as u32,
            nombre: it.nombre.trim().to_string(),
            cantidad: it.cantidad,
            precio_unitario: it.precio_unitario,
            descuento_pct: None,
            monto_item: monto,
            codigo_sku: None,
            unidad_medida: None,
            exento: it.exento,
        });
    }
    let (neto, iva) = desglose_iva(afecto);

    let referencias: Vec<DteReferencia> = spec
        .referencias
        .iter()
        .map(|r| DteReferencia {
            tipo_doc_ref: r.tipo_doc_ref.clone(),
            folio_ref: r.folio_ref.clone(),
            fecha_ref: r.fecha_ref,
            cod_ref: r.cod_ref,
            razon_ref: r.razon_ref.clone(),
        })
        .collect();

    Ok(Dte {
        id: Uuid::new_v4(),
        tipo: spec.tipo,
        folio,
        fecha_emision,
        rut_emisor: rut_emisor.to_string(),
        rut_receptor: spec.receptor.rut.trim().to_string(),
        razon_social_receptor: spec.receptor.razon_social.trim().to_string(),
        giro_receptor: Some(spec.receptor.giro.trim().to_string()),
        direccion_receptor: Some(spec.receptor.direccion.trim().to_string()),
        comuna_receptor: Some(spec.receptor.comuna.trim().to_string()),
        ind_traslado: spec.ind_traslado,
        referencias,
        monto_neto: neto,
        iva,
        monto_exento: exento,
        monto_total: afecto + exento,
        items,
        estado: DteEstado::Draft,
        xml_firmado: None,
        timbre: None,
        track_id: None,
        sii_glosa: None,
        metadata: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn receptor() -> ReceptorSpec {
        ReceptorSpec {
            rut: "77654321-0".into(),
            razon_social: "CLIENTE LTDA".into(),
            giro: "COMERCIO".into(),
            direccion: "CALLE 1".into(),
            comuna: "COQUIMBO".into(),
        }
    }

    fn item(nombre: &str, qty: i64, precio: i64, exento: bool) -> ItemSpec {
        ItemSpec {
            nombre: nombre.into(),
            cantidad: Decimal::from(qty),
            precio_unitario: Decimal::from(precio),
            exento,
        }
    }

    fn fecha() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap()
    }

    #[test]
    fn desglose_exacto() {
        // 11900 / 1.19 = 10000 exacto.
        let (neto, iva) = desglose_iva(Decimal::from(11900));
        assert_eq!(neto, Decimal::from(10000));
        assert_eq!(iva, Decimal::from(1900));
    }

    #[test]
    fn desglose_redondeo_iva_absorbe() {
        // 1000 / 1.19 = 840.336… → neto 840, IVA 160; neto+IVA == 1000.
        let (neto, iva) = desglose_iva(Decimal::from(1000));
        assert_eq!(neto, Decimal::from(840));
        assert_eq!(iva, Decimal::from(160));
        assert_eq!(neto + iva, Decimal::from(1000));
    }

    #[test]
    fn factura_totales_y_lineas() {
        let spec = DocumentoSpec {
            tipo: DteTipo::FacturaElectronica,
            receptor: receptor(),
            items: vec![item("A", 10, 1190, false), item("B", 1, 5000, true)],
            referencias: vec![],
            ind_traslado: None,
        };
        let d = build_documento(&spec, 7, "76123456-7", fecha()).unwrap();
        assert_eq!(d.folio, 7);
        assert_eq!(d.monto_neto, Decimal::from(10000));
        assert_eq!(d.iva, Decimal::from(1900));
        assert_eq!(d.monto_exento, Decimal::from(5000));
        assert_eq!(d.monto_total, Decimal::from(16900));
        assert_eq!(d.items.len(), 2);
        assert_eq!(d.items[0].nro_linea, 1);
        assert!(d.items[1].exento);
        assert_eq!(d.giro_receptor.as_deref(), Some("COMERCIO"));
    }

    #[test]
    fn boleta_rechazada() {
        let spec = DocumentoSpec {
            tipo: DteTipo::BoletaElectronica,
            receptor: receptor(),
            items: vec![item("A", 1, 100, false)],
            referencias: vec![],
            ind_traslado: None,
        };
        let err = build_documento(&spec, 1, "76123456-7", fecha()).unwrap_err();
        assert!(format!("{err}").contains("boleta"));
    }

    #[test]
    fn validaciones_items() {
        let base = |items: Vec<ItemSpec>| DocumentoSpec {
            tipo: DteTipo::FacturaElectronica,
            receptor: receptor(),
            items,
            referencias: vec![],
            ind_traslado: None,
        };
        assert!(build_documento(&base(vec![]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("", 1, 100, false)]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("A", 0, 100, false)]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("A", 1, -1, false)]), 1, "r", fecha()).is_err());
    }
}
