//! Tipos públicos del módulo DTE. Espejo de la migración `0017_dte.surql`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tipo de DTE según codificación SII (códigos oficiales SII §A.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum DteTipo {
    BoletaElectronica = 39,
    FacturaElectronica = 33,
    NotaDebito = 56,
    NotaCredito = 61,
    GuiaDespacho = 52,
}

impl DteTipo {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Result<Self, crate::DteError> {
        match code {
            39 => Ok(DteTipo::BoletaElectronica),
            33 => Ok(DteTipo::FacturaElectronica),
            56 => Ok(DteTipo::NotaDebito),
            61 => Ok(DteTipo::NotaCredito),
            52 => Ok(DteTipo::GuiaDespacho),
            other => Err(crate::DteError::UnsupportedTipo(other)),
        }
    }
}

impl From<DteTipo> for i32 {
    fn from(t: DteTipo) -> i32 {
        t.code()
    }
}

impl TryFrom<i32> for DteTipo {
    type Error = crate::DteError;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        DteTipo::from_code(v)
    }
}

/// Estado del ciclo de vida de un DTE. Transiciones permitidas:
/// - `Draft → Signed` (folio asignado + XML firmado + TED).
/// - `Signed → Sent` (POST al SII hecho, track_id recibido).
/// - `Sent → Accepted | Rejected` (polling resultado SII).
/// - `Draft | Signed → Cancelled` (operador anula pre-envío).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DteEstado {
    Draft,
    Signed,
    Sent,
    Accepted,
    Rejected,
    Cancelled,
}

/// Item line del DTE. Cantidad obligatoria; precio y descuento por unidad.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DteItem {
    pub nro_linea: u32,
    pub nombre: String,
    pub cantidad: rust_decimal::Decimal,
    pub precio_unitario: rust_decimal::Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descuento_pct: Option<rust_decimal::Decimal>,
    pub monto_item: rust_decimal::Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codigo_sku: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unidad_medida: Option<String>,
    #[serde(default)]
    pub exento: bool,
}

/// DTE completo. Representación in-memory; `xml_firmado` y `timbre` se llenan
/// al firmar; `track_id` cuando hay envío SII.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dte {
    pub id: Uuid,
    pub tipo: DteTipo,
    pub folio: i64,
    pub fecha_emision: DateTime<Utc>,
    pub rut_emisor: String,
    pub rut_receptor: String,
    pub razon_social_receptor: String,
    pub monto_neto: rust_decimal::Decimal,
    pub iva: rust_decimal::Decimal,
    pub monto_exento: rust_decimal::Decimal,
    pub monto_total: rust_decimal::Decimal,
    pub items: Vec<DteItem>,
    pub estado: DteEstado,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml_firmado: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timbre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sii_glosa: Option<String>,
}

/// CAF (Código de Autorización de Folios) — XML que SII entrega al contribuyente
/// autorizando un rango de folios para un tipo de DTE. `next_folio` es el
/// puntero del próximo folio a usar; assignment atómico via transacción
/// SurrealDB (ver `caf::assign_next`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caf {
    pub id: Uuid,
    pub tipo_dte: DteTipo,
    pub folio_desde: i64,
    pub folio_hasta: i64,
    pub next_folio: i64,
    pub fecha_autorizacion: DateTime<Utc>,
    pub rut_emisor: String,
    pub xml: String,
    pub activo: bool,
}

impl Caf {
    /// `true` si quedan folios disponibles en este CAF.
    pub fn has_folios(&self) -> bool {
        self.activo && self.next_folio <= self.folio_hasta
    }
}

/// Cert digital de la empresa (PFX). `pfx_blob` y `password_encrypted` van
/// cifrados con clave derivada `argon2id(tenant_id, master_key)` — ver
/// `cert::encrypt_at_rest` / `decrypt_for_sign`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertDigital {
    pub id: Uuid,
    pub rut_propietario: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nombre_propietario: Option<String>,
    pub pfx_blob: Vec<u8>,
    pub password_encrypted: Vec<u8>,
    pub vigencia_desde: DateTime<Utc>,
    pub vigencia_hasta: DateTime<Utc>,
    pub activo: bool,
}

impl CertDigital {
    /// `true` si el cert está vigente para la fecha indicada.
    pub fn is_valid_at(&self, at: DateTime<Utc>) -> bool {
        self.activo && self.vigencia_desde <= at && at <= self.vigencia_hasta
    }
}

/// Datos del emisor (farmacia) necesarios para armar el XML DTE. Vienen del
/// tenant config; el caller los pasa al renderer. Mantener separado del `Dte`
/// porque cambian por tenant y no por documento.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmisorConfig {
    pub rut: String,
    pub razon_social: String,
    pub giro: String,
    pub direccion: String,
    pub comuna: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciudad: Option<String>,
    /// Código SII actividad económica (acteco). Opcional para boleta (39).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acteco: Option<i32>,
}

/// Entorno SII al que se envían los DTEs. Default sandbox (test contra
/// `maullin.sii.cl`). Prod requiere flag explícito (`PHARMA__DTE__SII_ENV=prod`
/// + CLI `--confirm-prod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SiiEnv {
    #[default]
    Sandbox,
    Prod,
}

impl SiiEnv {
    pub fn upload_endpoint(self) -> &'static str {
        match self {
            SiiEnv::Sandbox => "https://maullin.sii.cl/cgi_dte/UPL/DTEUpload",
            SiiEnv::Prod => "https://palena.sii.cl/cgi_dte/UPL/DTEUpload",
        }
    }
}
