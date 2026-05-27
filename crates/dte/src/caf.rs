//! CAF (Código de Autorización de Folios) — parse XML SII + asignación atómica
//! de folio (subtask 9.1.c).
//!
//! El CAF es entregado por SII al contribuyente con un rango de folios
//! autorizados para un tipo de DTE específico, firmado por SII. Estructura
//! relevante:
//! ```xml
//! <AUTORIZACION>
//!   <CAF version="1.0">
//!     <DA>
//!       <RE>76.123.456-7</RE>
//!       <RS>RAZON SOCIAL</RS>
//!       <TD>39</TD>
//!       <RNG><D>1</D><H>1000</H></RNG>
//!       <FA>2026-01-01</FA>
//!       <RSAPK><M>...</M><E>...</E></RSAPK>
//!       <IDK>100</IDK>
//!     </DA>
//!     <FRMA algoritmo="SHA1withRSA">...</FRMA>
//!   </CAF>
//!   <RSASK>-----BEGIN RSA PRIVATE KEY-----...</RSASK>
//!   <RSAPUBK>-----BEGIN PUBLIC KEY-----...</RSAPUBK>
//! </AUTORIZACION>
//! ```
//!
//! El XML completo se persiste en `caf.xml` porque el TED (9.1.b) lo embebe
//! inline en su `<DD>`.
//!
//! Folio assignment es serializado por un Mutex global (ver `ASSIGN_LOCK`):
//! SurrealKV no garantiza serializable isolation cross-process en kv-mem y
//! deja pasar write-write races sobre el mismo record. El overhead del lock
//! es despreciable vs la demanda real POS (decenas de folios/seg pico).
//! Pruebas de concurrencia en `tests/caf_folio_atomic.rs` contra kv-mem.

use crate::types::{Caf, DteTipo};
use crate::DteError;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::sync::OnceLock;
use surrealdb::sql::Thing;
use surrealdb::{Connection, Surreal};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

/// Lock global de assignment de folios. Serializa la fase SELECT + UPDATE
/// dentro del proceso para garantizar atomicidad sin depender de la
/// detección MVCC de SurrealKV (que en pruebas concurrentes deja pasar
/// write-write races sobre el mismo record). El throughput resultante
/// (~10k folios/seg en debug, mucho más en release) es órdenes de magnitud
/// superior a la demanda real de un POS (decenas/seg pico). Si en el futuro
/// la contención mide en POS hot path, granularizar a lock por
/// `(tenant_id, tipo_dte)`.
static ASSIGN_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn assign_lock() -> &'static AsyncMutex<()> {
    ASSIGN_LOCK.get_or_init(|| AsyncMutex::new(()))
}

/// Estructura mínima del XML CAF para extraer metadatos. Solo lee los campos
/// que necesitamos; el XML entero se conserva por separado para el TED.
#[derive(Debug, Deserialize)]
struct AutorizacionXml {
    #[serde(rename = "CAF")]
    caf: CafXml,
}

#[derive(Debug, Deserialize)]
struct CafXml {
    #[serde(rename = "DA")]
    da: DaXml,
}

#[derive(Debug, Deserialize)]
struct DaXml {
    #[serde(rename = "RE")]
    re: String,
    #[serde(rename = "TD")]
    td: i32,
    #[serde(rename = "RNG")]
    rng: RngXml,
    #[serde(rename = "FA")]
    fa: String,
}

#[derive(Debug, Deserialize)]
struct RngXml {
    #[serde(rename = "D")]
    desde: i64,
    #[serde(rename = "H")]
    hasta: i64,
}

/// Parsea el XML CAF entregado por SII a una struct `Caf` con el XML completo
/// conservado en `xml` (necesario para inyectar en el TED).
pub fn parse_xml(xml: &str) -> Result<Caf, DteError> {
    let parsed: AutorizacionXml = quick_xml::de::from_str(xml)
        .map_err(|e| DteError::CafInvalid(format!("XML inválido: {e}")))?;
    let da = parsed.caf.da;
    let tipo = DteTipo::from_code(da.td)?;
    if da.rng.desde < 1 || da.rng.hasta < da.rng.desde {
        return Err(DteError::CafInvalid(format!(
            "rango folios inválido: {}..{}",
            da.rng.desde, da.rng.hasta
        )));
    }
    let fecha = NaiveDate::parse_from_str(&da.fa, "%Y-%m-%d")
        .map_err(|e| DteError::CafInvalid(format!("FA con formato inválido: {e}")))?;
    let fecha_autorizacion =
        Utc.from_utc_datetime(&fecha.and_hms_opt(0, 0, 0).expect("00:00:00 válido"));
    Ok(Caf {
        id: Uuid::new_v4(),
        tipo_dte: tipo,
        folio_desde: da.rng.desde,
        folio_hasta: da.rng.hasta,
        next_folio: da.rng.desde,
        fecha_autorizacion,
        rut_emisor: da.re,
        xml: xml.to_string(),
        activo: true,
    })
}

/// Record persistido en SurrealDB tabla `caf`. Mirror de la migración 0017
/// con `Thing` para `id` y `tenant`.
#[derive(Debug, Clone, Deserialize)]
pub struct CafRecord {
    pub id: Thing,
    pub tenant: Thing,
    pub tipo_dte: i32,
    pub folio_desde: i64,
    pub folio_hasta: i64,
    pub next_folio: i64,
    pub fecha_autorizacion: DateTime<Utc>,
    pub rut_emisor: String,
    pub xml: String,
    pub activo: bool,
}

/// Asigna atómicamente el próximo folio del CAF activo del tenant para el
/// tipo de DTE indicado. Retorna el record CAF actualizado y el folio
/// asignado.
///
/// Serializa el SELECT+UPDATE con un Mutex global (`ASSIGN_LOCK`). El
/// throughput resultante cubre demanda POS con margen (decenas de folios/seg
/// pico vs miles posibles bajo lock).
///
/// Errores:
/// - `FolioExhausted` si no hay CAF activo con folios disponibles.
pub async fn assign_next<C: Connection>(
    db: &Surreal<C>,
    tenant: &Thing,
    tipo: DteTipo,
) -> Result<(CafRecord, i64), DteError> {
    let _guard = assign_lock().lock().await;
    // Patrón atómico:
    //   1. SELECT del CAF activo con menor folio_desde (fase de descubrimiento).
    //   2. UPDATE $id SET next_folio = next_folio + 1 WHERE next_folio <=
    //      folio_hasta RETURN BEFORE — SurrealDB ejecuta el SET sobre el valor
    //      actual del campo en una sola operación atómica; dos callers
    //      concurrentes sobre el mismo CAF obtienen folios distintos.
    //   3. Si el UPDATE no afectó nada (CAF agotado entre fase 1 y 2),
    //      reintentamos: otro CAF activo puede estar disponible o el rango
    //      se acaba de agotar realmente.
    //
    // Cap 256 reintentos cubre escenarios POS holgado (10 cajas simultáneas
    // → peor caso 9 races por folio, latencia ms-scale).
    for intento in 0..256 {
        let res = db
            .query(
                "SELECT * FROM caf \
                 WHERE tenant = $tenant AND tipo_dte = $tipo AND activo = true \
                   AND next_folio <= folio_hasta \
                 ORDER BY folio_desde ASC LIMIT 1",
            )
            .bind(("tenant", tenant.clone()))
            .bind(("tipo", tipo.code()))
            .await;
        let mut q = match res {
            Ok(q) => q,
            Err(e) if is_mvcc_conflict(&e) => {
                backoff(intento).await;
                continue;
            }
            Err(e) => return Err(DteError::CafInvalid(format!("query CAF activo: {e}"))),
        };
        let candidatos: Vec<CafRecord> = match q.take(0) {
            Ok(v) => v,
            Err(e) => return Err(DteError::CafInvalid(format!("decode CAF: {e}"))),
        };
        let Some(caf) = candidatos.into_iter().next() else {
            return Err(DteError::FolioExhausted {
                tipo: tipo.code(),
                folio: 0,
            });
        };
        let upd_res = db
            .query(
                "UPDATE $id SET next_folio = next_folio + 1 \
                 WHERE next_folio <= folio_hasta RETURN BEFORE",
            )
            .bind(("id", caf.id.clone()))
            .await;
        let mut upd = match upd_res {
            Ok(r) => r,
            Err(e) if is_mvcc_conflict(&e) => {
                backoff(intento).await;
                continue;
            }
            Err(e) => return Err(DteError::CafInvalid(format!("update folio: {e}"))),
        };
        let antes: Vec<CafRecord> = match upd.take(0) {
            Ok(v) => v,
            Err(e) if is_mvcc_conflict_str(&e.to_string()) => {
                backoff(intento).await;
                continue;
            }
            Err(e) => return Err(DteError::CafInvalid(format!("decode update: {e}"))),
        };
        if let Some(record_antes) = antes.into_iter().next() {
            if record_antes.next_folio <= record_antes.folio_hasta {
                // Folio asignado = el valor next_folio ANTES del incremento.
                let folio_asignado = record_antes.next_folio;
                let mut record_post = record_antes;
                record_post.next_folio += 1;
                return Ok((record_post, folio_asignado));
            }
        }
        // CAF que parecía libre se agotó entre fase 1 y 2: reintentamos para
        // buscar otro CAF activo o confirmar FolioExhausted.
        backoff(intento).await;
    }
    Err(DteError::CafInvalid(
        "asignación de folio: 256 reintentos sin éxito (contención extrema)".to_string(),
    ))
}

fn is_mvcc_conflict(e: &surrealdb::Error) -> bool {
    is_mvcc_conflict_str(&e.to_string())
}

fn is_mvcc_conflict_str(s: &str) -> bool {
    s.contains("read or write conflict") || s.contains("failed transaction")
}

/// Backoff lineal corto (max ~5ms) — el conflict window de SurrealKV es de
/// microsegundos; backoff largo sólo desperdicia latencia POS.
async fn backoff(intento: usize) {
    let micros = std::cmp::min((intento as u64 + 1) * 50, 5_000);
    tokio::time::sleep(std::time::Duration::from_micros(micros)).await;
}
