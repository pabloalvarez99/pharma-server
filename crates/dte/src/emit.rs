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

use crate::types::{
    DescuentoGlobal, Dte, DteEstado, DteItem, DteReferencia, DteTipo, TipoMovDr, TipoValorDr,
};
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

/// Descuento/recargo global a nivel documento (elemento `DscRcgGlobal`).
/// Se aplica sobre el total afecto o exento (gross, IVA-incluido — la misma
/// convención que el resto del crate) y reajusta el desglose de IVA.
#[derive(Debug, Clone, Deserialize)]
pub struct DescuentoGlobalSpec {
    /// "D" descuento (resta) | "R" recargo (suma).
    pub tipo_mov: TipoMovDr,
    #[serde(default)]
    pub glosa: Option<String>,
    /// "%" porcentaje | "$" monto CLP.
    pub tipo_valor: TipoValorDr,
    /// Porcentaje (0 < v ≤ 100 si %) o monto CLP (> 0 si $).
    pub valor: Decimal,
    /// `true` = aplica al monto exento (`IndExeDR=1`); ausente = al afecto.
    #[serde(default)]
    pub sobre_exento: bool,
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
    /// Descuentos/recargos globales (`DscRcgGlobal`). Hasta 20. Vacío = ninguno.
    pub descuentos_globales: Vec<DescuentoGlobalSpec>,
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

/// Máximo de líneas `DscRcgGlobal` por documento (límite xsd SII).
const MAX_DSC_RCG_GLOBAL: usize = 20;

/// Valida y aplica los descuentos/recargos globales sobre las bases `afecto` y
/// `exento` (mutadas in-place), y devuelve los `DescuentoGlobal` resultantes
/// para el `Dte`. El monto de cada línea se calcula sobre la base ORIGINAL de
/// su categoría (no acumulativo entre líneas) — convención SII para %.
fn aplicar_dsc_rcg_global(
    spec: &DocumentoSpec,
    afecto: &mut Decimal,
    exento: &mut Decimal,
) -> Result<Vec<DescuentoGlobal>, DteError> {
    if spec.descuentos_globales.is_empty() {
        return Ok(Vec::new());
    }
    if spec.descuentos_globales.len() > MAX_DSC_RCG_GLOBAL {
        return Err(DteError::XmlInvalid(format!(
            "máximo {MAX_DSC_RCG_GLOBAL} descuentos/recargos globales (DscRcgGlobal) por documento"
        )));
    }
    let cien = Decimal::from(100);
    let base_afecto = *afecto;
    let base_exento = *exento;
    let mut out = Vec::with_capacity(spec.descuentos_globales.len());
    for (i, dr) in spec.descuentos_globales.iter().enumerate() {
        let n = i + 1;
        if dr.valor <= Decimal::ZERO {
            return Err(DteError::XmlInvalid(format!(
                "DscRcgGlobal {n}: el valor debe ser > 0"
            )));
        }
        if dr.tipo_valor == TipoValorDr::Porcentaje && dr.valor > cien {
            return Err(DteError::XmlInvalid(format!(
                "DscRcgGlobal {n}: el porcentaje no puede superar 100"
            )));
        }
        let base = if dr.sobre_exento {
            base_exento
        } else {
            base_afecto
        };
        let monto = match dr.tipo_valor {
            TipoValorDr::Porcentaje => (base * dr.valor / cien).trunc(),
            TipoValorDr::Monto => dr.valor.trunc(),
        };
        let delta = match dr.tipo_mov {
            TipoMovDr::Descuento => -monto,
            TipoMovDr::Recargo => monto,
        };
        if dr.sobre_exento {
            *exento += delta;
        } else {
            *afecto += delta;
        }
        out.push(DescuentoGlobal {
            nro_linea: n as u32,
            tipo_mov: dr.tipo_mov,
            glosa: dr.glosa.clone(),
            tipo_valor: dr.tipo_valor,
            valor: dr.valor,
            ind_exe: dr.sobre_exento.then_some(1),
        });
    }
    if *afecto < Decimal::ZERO || *exento < Decimal::ZERO {
        return Err(DteError::XmlInvalid(
            "los descuentos globales exceden el monto del documento (base negativa)".to_string(),
        ));
    }
    Ok(out)
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
    validate_estructura(spec)?;
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
    // Descuentos/recargos globales (DscRcgGlobal). Se aplican sobre el total
    // afecto o exento (gross, IVA-incluido) según `sobre_exento`; % se calcula
    // sobre la base ORIGINAL del detalle, $ es monto directo. El desglose de
    // IVA se hace sobre la base ya ajustada.
    let descuentos_globales = aplicar_dsc_rcg_global(spec, &mut afecto, &mut exento)?;

    // Factura afecta (33) exige base afecta > 0 (neto + IVA desglosados). Un
    // documento 100% exento no va en tipo 33 — el renderer lo rechaza igual,
    // pero acá evita quemar un folio. Notas/guía no tienen esta restricción.
    // Se evalúa DESPUÉS del D/R global: un descuento podría anular la base.
    if spec.tipo == DteTipo::FacturaElectronica && afecto <= Decimal::ZERO {
        return Err(DteError::XmlInvalid(
            "factura (33) afecta requiere monto neto e IVA desglosados (> 0); \
             un documento 100% exento no corresponde a tipo 33"
                .to_string(),
        ));
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
        descuentos_globales,
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

/// Valida las precondiciones estructurales por tipo que de otro modo sólo
/// fallarían al renderizar (`xml::factura/nota_credito/guia`) — para entonces
/// el folio CAF ya se quemó. Espejando esas reglas acá, la validación previa
/// del handler (`build_documento` con folio dummy) las atrapa antes de
/// `caf::assign_next`. Las validaciones de `xml::*` siguen siendo la barrera
/// autoritativa (defensa en profundidad); esto sólo adelanta el error.
fn validate_estructura(spec: &DocumentoSpec) -> Result<(), DteError> {
    let vacio = |s: &str| s.trim().is_empty();
    // Receptor completo: 33/56/61/52 lo exigen (boleta 39 ya fue rechazada).
    if vacio(&spec.receptor.rut) {
        return Err(DteError::XmlInvalid("receptor requiere RUT".to_string()));
    }
    if vacio(&spec.receptor.razon_social) {
        return Err(DteError::XmlInvalid(
            "receptor requiere razón social".to_string(),
        ));
    }
    if vacio(&spec.receptor.giro) || vacio(&spec.receptor.direccion) || vacio(&spec.receptor.comuna)
    {
        return Err(DteError::XmlInvalid(format!(
            "receptor de documento tipo {} requiere giro, dirección y comuna",
            spec.tipo.code()
        )));
    }
    match spec.tipo {
        // Notas (56/61) referencian SIEMPRE el documento que corrigen.
        DteTipo::NotaCredito | DteTipo::NotaDebito => {
            if spec.referencias.is_empty() {
                return Err(DteError::XmlInvalid(format!(
                    "documento tipo {}: una nota requiere al menos una referencia al documento original",
                    spec.tipo.code()
                )));
            }
            for (i, r) in spec.referencias.iter().enumerate() {
                if !matches!(r.cod_ref, Some(1..=3)) {
                    return Err(DteError::XmlInvalid(format!(
                        "documento tipo {}: referencia {} requiere cod_ref 1 (anula), 2 (corrige texto) o 3 (corrige montos)",
                        spec.tipo.code(),
                        i + 1
                    )));
                }
            }
        }
        // Guía de despacho (52) exige el motivo del traslado (1..9).
        DteTipo::GuiaDespacho if !matches!(spec.ind_traslado, Some(1..=9)) => {
            return Err(DteError::XmlInvalid(format!(
                "documento tipo {}: la guía requiere ind_traslado entre 1 y 9 (motivo del traslado)",
                spec.tipo.code()
            )));
        }
        _ => {}
    }
    Ok(())
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
            descuentos_globales: vec![],
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
            descuentos_globales: vec![],
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
            descuentos_globales: vec![],
        };
        assert!(build_documento(&base(vec![]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("", 1, 100, false)]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("A", 0, 100, false)]), 1, "r", fecha()).is_err());
        assert!(build_documento(&base(vec![item("A", 1, -1, false)]), 1, "r", fecha()).is_err());
    }

    fn referencia(cod: Option<i32>) -> ReferenciaSpec {
        ReferenciaSpec {
            tipo_doc_ref: "33".into(),
            folio_ref: "100".into(),
            fecha_ref: fecha(),
            cod_ref: cod,
            razon_ref: Some("anula".into()),
        }
    }

    fn spec_tipo(
        tipo: DteTipo,
        refs: Vec<ReferenciaSpec>,
        ind_traslado: Option<i32>,
    ) -> DocumentoSpec {
        DocumentoSpec {
            tipo,
            receptor: receptor(),
            items: vec![item("A", 1, 1190, false)],
            referencias: refs,
            ind_traslado,
            descuentos_globales: vec![],
        }
    }

    #[test]
    fn nota_sin_referencia_rechazada_antes_de_folio() {
        // Espejo del check de `xml::nota_credito::require_referencias_nota`,
        // adelantado para no quemar folio (folio dummy = 1).
        for tipo in [DteTipo::NotaCredito, DteTipo::NotaDebito] {
            let err = build_documento(&spec_tipo(tipo, vec![], None), 1, "r", fecha()).unwrap_err();
            assert!(
                format!("{err}").contains("referencia"),
                "tipo {}",
                tipo.code()
            );
        }
    }

    #[test]
    fn nota_con_cod_ref_invalido_rechazada() {
        for cod in [None, Some(0), Some(4)] {
            let spec = spec_tipo(DteTipo::NotaCredito, vec![referencia(cod)], None);
            let err = build_documento(&spec, 1, "r", fecha()).unwrap_err();
            assert!(format!("{err}").contains("cod_ref"), "cod {cod:?}");
        }
    }

    #[test]
    fn nota_con_referencia_valida_ok() {
        let spec = spec_tipo(DteTipo::NotaCredito, vec![referencia(Some(1))], None);
        let d = build_documento(&spec, 5, "76123456-7", fecha()).unwrap();
        assert_eq!(d.tipo, DteTipo::NotaCredito);
        assert_eq!(d.referencias.len(), 1);
    }

    #[test]
    fn guia_sin_ind_traslado_rechazada() {
        let err = build_documento(
            &spec_tipo(DteTipo::GuiaDespacho, vec![], None),
            1,
            "r",
            fecha(),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("ind_traslado"));
    }

    #[test]
    fn guia_ind_traslado_fuera_de_rango_rechazada() {
        for ind in [0, 10] {
            let spec = spec_tipo(DteTipo::GuiaDespacho, vec![], Some(ind));
            assert!(
                build_documento(&spec, 1, "r", fecha()).is_err(),
                "ind {ind}"
            );
        }
    }

    #[test]
    fn guia_ind_traslado_valido_ok() {
        let spec = spec_tipo(DteTipo::GuiaDespacho, vec![], Some(1));
        let d = build_documento(&spec, 9, "76123456-7", fecha()).unwrap();
        assert_eq!(d.ind_traslado, Some(1));
    }

    #[test]
    fn receptor_incompleto_rechazado() {
        let mut spec = spec_tipo(DteTipo::FacturaElectronica, vec![], None);
        spec.receptor.comuna = "  ".into();
        let err = build_documento(&spec, 1, "r", fecha()).unwrap_err();
        assert!(format!("{err}").contains("comuna"));
    }

    #[test]
    fn factura_totalmente_exenta_rechazada() {
        // afecto = 0 (única línea exenta) ⇒ tipo 33 no corresponde.
        let spec = DocumentoSpec {
            tipo: DteTipo::FacturaElectronica,
            receptor: receptor(),
            items: vec![item("EXENTO", 1, 5000, true)],
            referencias: vec![],
            ind_traslado: None,
            descuentos_globales: vec![],
        };
        let err = build_documento(&spec, 1, "r", fecha()).unwrap_err();
        assert!(format!("{err}").contains("exento"));
    }

    fn dr(
        tipo_mov: TipoMovDr,
        tipo_valor: TipoValorDr,
        valor: i64,
        sobre_exento: bool,
    ) -> DescuentoGlobalSpec {
        DescuentoGlobalSpec {
            tipo_mov,
            glosa: Some("PROMO".into()),
            tipo_valor,
            valor: Decimal::from(valor),
            sobre_exento,
        }
    }

    fn spec_con_dr(
        items: Vec<ItemSpec>,
        descuentos_globales: Vec<DescuentoGlobalSpec>,
    ) -> DocumentoSpec {
        DocumentoSpec {
            tipo: DteTipo::FacturaElectronica,
            receptor: receptor(),
            items,
            referencias: vec![],
            ind_traslado: None,
            descuentos_globales,
        }
    }

    #[test]
    fn dscrcg_descuento_porcentaje_afecto() {
        // afecto bruto 11900 − 10% (1190) = 10710 → neto 9000, IVA 1710.
        let spec = spec_con_dr(
            vec![item("A", 1, 11900, false)],
            vec![dr(TipoMovDr::Descuento, TipoValorDr::Porcentaje, 10, false)],
        );
        let d = build_documento(&spec, 1, "76123456-7", fecha()).unwrap();
        assert_eq!(d.monto_neto, Decimal::from(9000));
        assert_eq!(d.iva, Decimal::from(1710));
        assert_eq!(d.monto_total, Decimal::from(10710));
        assert_eq!(d.monto_neto + d.iva, Decimal::from(10710));
        assert_eq!(d.descuentos_globales.len(), 1);
        assert_eq!(d.descuentos_globales[0].nro_linea, 1);
        assert_eq!(d.descuentos_globales[0].ind_exe, None);
    }

    #[test]
    fn dscrcg_descuento_monto_fijo_afecto() {
        // afecto 11900 − $900 = 11000 → neto round(11000/1.19)=9244, IVA 1756.
        let spec = spec_con_dr(
            vec![item("A", 1, 11900, false)],
            vec![dr(TipoMovDr::Descuento, TipoValorDr::Monto, 900, false)],
        );
        let d = build_documento(&spec, 1, "76123456-7", fecha()).unwrap();
        assert_eq!(d.monto_total, Decimal::from(11000));
        assert_eq!(d.monto_neto + d.iva, Decimal::from(11000));
    }

    #[test]
    fn dscrcg_recargo_afecto_suma() {
        // afecto 10000 + 5% (500) = 10500.
        let spec = spec_con_dr(
            vec![item("A", 1, 10000, false)],
            vec![dr(TipoMovDr::Recargo, TipoValorDr::Porcentaje, 5, false)],
        );
        let d = build_documento(&spec, 1, "76123456-7", fecha()).unwrap();
        assert_eq!(d.monto_total, Decimal::from(10500));
    }

    #[test]
    fn dscrcg_sobre_exento_marca_ind_exe() {
        // Línea exenta 5000 − 20% (1000) = 4000 exento; afecto intacto.
        let spec = spec_con_dr(
            vec![item("A", 1, 11900, false), item("EX", 1, 5000, true)],
            vec![dr(TipoMovDr::Descuento, TipoValorDr::Porcentaje, 20, true)],
        );
        let d = build_documento(&spec, 1, "76123456-7", fecha()).unwrap();
        assert_eq!(d.monto_exento, Decimal::from(4000));
        assert_eq!(d.monto_neto, Decimal::from(10000));
        assert_eq!(d.iva, Decimal::from(1900));
        // total = neto + IVA + exento = 10000 + 1900 + 4000.
        assert_eq!(d.monto_total, Decimal::from(15900));
        assert_eq!(d.descuentos_globales[0].ind_exe, Some(1));
    }

    #[test]
    fn dscrcg_varias_lineas_sobre_base_original() {
        // Dos descuentos % se calculan sobre la base ORIGINAL (no acumulativo):
        // 10000 − 10%(1000) − 5%(500) = 8500.
        let spec = spec_con_dr(
            vec![item("A", 1, 10000, false)],
            vec![
                dr(TipoMovDr::Descuento, TipoValorDr::Porcentaje, 10, false),
                dr(TipoMovDr::Descuento, TipoValorDr::Porcentaje, 5, false),
            ],
        );
        let d = build_documento(&spec, 1, "76123456-7", fecha()).unwrap();
        assert_eq!(d.monto_total, Decimal::from(8500));
        assert_eq!(d.descuentos_globales[1].nro_linea, 2);
    }

    #[test]
    fn dscrcg_valor_invalido_rechazado() {
        // valor 0 y porcentaje > 100.
        let cero = spec_con_dr(
            vec![item("A", 1, 10000, false)],
            vec![dr(TipoMovDr::Descuento, TipoValorDr::Monto, 0, false)],
        );
        assert!(
            format!("{}", build_documento(&cero, 1, "r", fecha()).unwrap_err()).contains("> 0")
        );
        let sobre = spec_con_dr(
            vec![item("A", 1, 10000, false)],
            vec![dr(
                TipoMovDr::Descuento,
                TipoValorDr::Porcentaje,
                101,
                false,
            )],
        );
        assert!(
            format!("{}", build_documento(&sobre, 1, "r", fecha()).unwrap_err())
                .contains("porcentaje")
        );
    }

    #[test]
    fn dscrcg_descuento_excede_base_rechazado() {
        let spec = spec_con_dr(
            vec![item("A", 1, 10000, false)],
            vec![dr(TipoMovDr::Descuento, TipoValorDr::Monto, 20000, false)],
        );
        let err = build_documento(&spec, 1, "r", fecha()).unwrap_err();
        assert!(format!("{err}").contains("exceden"));
    }

    #[test]
    fn dscrcg_maximo_20_lineas() {
        let muchos: Vec<DescuentoGlobalSpec> = (0..21)
            .map(|_| dr(TipoMovDr::Descuento, TipoValorDr::Monto, 1, false))
            .collect();
        let spec = spec_con_dr(vec![item("A", 1, 100000, false)], muchos);
        let err = build_documento(&spec, 1, "r", fecha()).unwrap_err();
        assert!(format!("{err}").contains("20"));
    }
}
