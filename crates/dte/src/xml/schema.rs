//! Structs serde para el XML SII xsd 1.0. Estos son DTOs, NO el dominio:
//! convertir desde `Dte` + `EmisorConfig` en cada renderer por tipo.
//!
//! Reglas SII relevantes:
//! - Los montos en CLP son enteros (sin decimales). Conversor `clp_int` en
//!   `xml::boleta` se encarga.
//! - `FchEmis` formato `YYYY-MM-DD`. `TmstFirma` formato `YYYY-MM-DDTHH:MM:SS`
//!   sin zona (SII trabaja hora local CL, no UTC; el caller convierte).
//! - El orden de los elementos importa (xsd con `xs:sequence`). El orden de
//!   campos en la struct = orden en el XML output.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename = "DTE")]
pub struct DteXml {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "Documento")]
    pub documento: Documento,
}

#[derive(Debug, Serialize)]
pub struct Documento {
    #[serde(rename = "@ID")]
    pub id: String,
    #[serde(rename = "Encabezado")]
    pub encabezado: Encabezado,
    #[serde(rename = "Detalle")]
    pub detalle: Vec<Detalle>,
    /// Referencias a otros documentos (xsd: tras Detalle, antes del TED).
    /// Obligatorias en notas 56/61; vacío = no se serializa.
    #[serde(rename = "Referencia", skip_serializing_if = "Vec::is_empty")]
    pub referencia: Vec<Referencia>,
    /// `TmstFirma` siempre se escribe; el TED se inyecta entre Referencia y
    /// `TmstFirma` en una pasada de string-replace (subtask 9.1.b) — el xsd
    /// pone el TED justo después de Referencia, antes de TmstFirma.
    #[serde(rename = "TmstFirma")]
    pub tmst_firma: String,
}

#[derive(Debug, Serialize)]
pub struct Encabezado {
    #[serde(rename = "IdDoc")]
    pub id_doc: IdDoc,
    #[serde(rename = "Emisor")]
    pub emisor: Emisor,
    #[serde(rename = "Receptor")]
    pub receptor: Receptor,
    #[serde(rename = "Totales")]
    pub totales: Totales,
}

#[derive(Debug, Serialize)]
pub struct IdDoc {
    #[serde(rename = "TipoDTE")]
    pub tipo_dte: i32,
    #[serde(rename = "Folio")]
    pub folio: i64,
    #[serde(rename = "FchEmis")]
    pub fch_emis: String,
    /// `IndTraslado` guía 52 (1 venta … 9 venta exportación). xsd: va antes
    /// de `IndServicio`.
    #[serde(rename = "IndTraslado", skip_serializing_if = "Option::is_none")]
    pub ind_traslado: Option<i32>,
    /// `IndServicio`: 1 servicios periódicos, 2 servicios periódicos domiciliarios,
    /// 3 venta de bienes/servicios (default boleta retail farmacia).
    #[serde(rename = "IndServicio", skip_serializing_if = "Option::is_none")]
    pub ind_servicio: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct Emisor {
    #[serde(rename = "RUTEmisor")]
    pub rut_emisor: String,
    #[serde(rename = "RznSocEmisor")]
    pub rzn_soc_emisor: String,
    #[serde(rename = "GiroEmisor")]
    pub giro_emisor: String,
    #[serde(rename = "Acteco", skip_serializing_if = "Option::is_none")]
    pub acteco: Option<i32>,
    #[serde(rename = "DirOrigen")]
    pub dir_origen: String,
    #[serde(rename = "CmnaOrigen")]
    pub cmna_origen: String,
    #[serde(rename = "CiudadOrigen", skip_serializing_if = "Option::is_none")]
    pub ciudad_origen: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Receptor {
    #[serde(rename = "RUTRecep")]
    pub rut_recep: String,
    #[serde(rename = "RznSocRecep")]
    pub rzn_soc_recep: String,
    /// Giro/dirección/comuna del receptor — obligatorios en 33/56/61/52,
    /// no aplican a boleta 39. xsd: GiroRecep → DirRecep → CmnaRecep.
    #[serde(rename = "GiroRecep", skip_serializing_if = "Option::is_none")]
    pub giro_recep: Option<String>,
    #[serde(rename = "DirRecep", skip_serializing_if = "Option::is_none")]
    pub dir_recep: Option<String>,
    #[serde(rename = "CmnaRecep", skip_serializing_if = "Option::is_none")]
    pub cmna_recep: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Totales {
    #[serde(rename = "MntNeto", skip_serializing_if = "Option::is_none")]
    pub mnt_neto: Option<i64>,
    #[serde(rename = "MntExe", skip_serializing_if = "Option::is_none")]
    pub mnt_exe: Option<i64>,
    #[serde(rename = "TasaIVA", skip_serializing_if = "Option::is_none")]
    pub tasa_iva: Option<i32>,
    #[serde(rename = "IVA", skip_serializing_if = "Option::is_none")]
    pub iva: Option<i64>,
    #[serde(rename = "MntTotal")]
    pub mnt_total: i64,
}

#[derive(Debug, Serialize)]
pub struct Detalle {
    #[serde(rename = "NroLinDet")]
    pub nro_lin_det: u32,
    /// `IndExe = 1`: línea exenta de IVA. xsd: entre NroLinDet y NmbItem.
    #[serde(rename = "IndExe", skip_serializing_if = "Option::is_none")]
    pub ind_exe: Option<i32>,
    #[serde(rename = "NmbItem")]
    pub nmb_item: String,
    #[serde(rename = "QtyItem")]
    pub qty_item: String,
    #[serde(rename = "PrcItem")]
    pub prc_item: String,
    #[serde(rename = "MontoItem")]
    pub monto_item: i64,
}

/// Elemento `Referencia` (xsd: NroLinRef → TpoDocRef → FolioRef → FchRef →
/// CodRef → RaznRef). Hasta 40 por documento.
#[derive(Debug, Serialize)]
pub struct Referencia {
    #[serde(rename = "NroLinRef")]
    pub nro_lin_ref: u32,
    #[serde(rename = "TpoDocRef")]
    pub tpo_doc_ref: String,
    #[serde(rename = "FolioRef")]
    pub folio_ref: String,
    #[serde(rename = "FchRef")]
    pub fch_ref: String,
    #[serde(rename = "CodRef", skip_serializing_if = "Option::is_none")]
    pub cod_ref: Option<i32>,
    #[serde(rename = "RaznRef", skip_serializing_if = "Option::is_none")]
    pub razn_ref: Option<String>,
}
