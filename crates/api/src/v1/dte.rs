//! DTE (boleta electrónica SII) HTTP surface — Fase 9.1 wiring.
//!
//! Cablea `crates/dte` (XML + TED + XML-DSig + CAF + cert + SII) a rutas
//! `/api/v1/dte/*`. Flujo: el POS vende (`POST /pos/sale`) → se emite la
//! boleta de esa orden (`POST /dte/boletas`) → queda `signed` local (Free
//! tier OK, ADR-0005: genera + exporta sin pagar) → opcionalmente se envía
//! al SII (`POST /dte/{id}/send`, tier-gated Pro+ — subtask 9.1.j) y se
//! consulta el veredicto (`POST /dte/{id}/poll`).
//!
//! Decisiones de wiring:
//! * **Emisor** (RUT, razón social, giro, dirección, comuna) vive en el
//!   admin_setting `dte.emisor` como JSON (`EmisorConfig`) — reusa el CRUD
//!   `PUT /api/v1/settings/{key}` existente; cero schema nuevo.
//! * **Cert**: el blob cifrado de `cert_digital` contiene el PFX/PKCS#12 tal
//!   cual (`pharma cert import cert.pfx` — parse nativo, subtask 9.1.b.3) o,
//!   back-compat, un bundle PEM del workaround previo (`openssl pkcs12
//!   -nodes`). `KeyMaterial::from_keystore_bytes` detecta el formato.
//! * **Passphrase** del cert viaja en el request de emisión; nunca se
//!   persiste ni se loguea (encrypt-at-rest, ADR-0011 §cert).
//! * **Entorno SII**: admin_setting `dte.sii_env` = `sandbox` (default) |
//!   `prod`.
//! * **Folio burn**: el folio se asigna (atómico, `caf::assign_next`) recién
//!   después de validar orden + emisor + cert; si la persistencia posterior
//!   falla el folio queda consumido — aceptado (SII contempla folios
//!   anulados), igual criterio que un POS físico.
//!
//! Roles: emisión = cashier+ (operación de mesón); send/poll/cancel =
//! admin+; lecturas = JWT.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::{admin_plus, cashier_plus};
use crate::AppState;

/// admin_setting key con el JSON de `dte::EmisorConfig` del tenant.
const EMISOR_SETTING_KEY: &str = "dte.emisor";
/// admin_setting key del entorno SII (`sandbox` default | `prod`).
const SII_ENV_SETTING_KEY: &str = "dte.sii_env";
/// RUT receptor por defecto (consumidor final, convención SII boleta).
const RUT_CONSUMIDOR_FINAL: &str = "66666666-6";

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/dte", get(list_dtes))
        .route("/api/v1/dte/caf-status", get(caf_status))
        .route("/api/v1/dte/{id}", get(get_dte))
        .route("/api/v1/dte/{id}/xml", get(dte_xml));

    let emit = Router::new()
        .route("/api/v1/dte/boletas", post(emit_boleta))
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));

    let admin = Router::new()
        .route("/api/v1/dte/documentos", post(emit_documento))
        .route("/api/v1/dte/cert", post(upload_cert))
        .route("/api/v1/dte/caf", post(upload_caf))
        .route("/api/v1/dte/libro-ventas", get(libro_ventas))
        .route("/api/v1/dte/libro-ventas/signed", post(libro_ventas_signed))
        .route("/api/v1/dte/{id}/send", post(send_dte))
        .route("/api/v1/dte/{id}/poll", post(poll_dte))
        .route("/api/v1/dte/{id}/cancel", post(cancel_dte))
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(emit).merge(admin)
}

// --- shared helpers ---------------------------------------------------------

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn user_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.sub).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

fn db_err(ctx: &'static str) -> impl Fn(surrealdb::Error) -> ApiError {
    move |e| {
        tracing::error!(error = %e, ctx, "dte: db error");
        ApiError::service_unavailable()
    }
}

/// Parsea el id de path: acepta `dte:<key>` o `<key>` pelado.
fn dte_thing(raw: &str) -> Result<Thing, ApiError> {
    if raw.is_empty() {
        return Err(ApiError::invalid("id DTE vacío"));
    }
    if raw.contains(':') {
        let t = surrealdb::sql::thing(raw)
            .map_err(|_| ApiError::invalid(format!("id DTE inválido: {raw}")))?;
        if t.tb != "dte" {
            return Err(ApiError::invalid(format!(
                "id de tabla '{}', se esperaba 'dte:<key>'",
                t.tb
            )));
        }
        Ok(t)
    } else {
        Ok(Thing::from(("dte", raw)))
    }
}

fn parse_day(s: &str, label: &str) -> Result<DateTime<Utc>, ApiError> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        ApiError::invalid(format!("'{label}' inválido '{s}' (esperado YYYY-MM-DD)"))
    })?;
    Ok(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).expect("00:00:00 válido")))
}

/// Row persistido en `dte` (subset que la API expone; `items`/`created_*`
/// se ignoran al decodificar).
#[derive(Debug, Deserialize)]
struct DteRow {
    id: Thing,
    tipo: i32,
    folio: i64,
    fecha_emision: DateTime<Utc>,
    rut_emisor: String,
    rut_receptor: String,
    razon_social_receptor: String,
    monto_total: Decimal,
    estado: String,
    #[serde(default)]
    xml_firmado: Option<String>,
    #[serde(default)]
    track_id: Option<i64>,
    #[serde(default)]
    sii_glosa: Option<String>,
    #[serde(default)]
    order_id: Option<Thing>,
}

/// DTO público. `monto_total` como string (convención money del repo).
#[derive(Debug, Serialize)]
pub(crate) struct DteDto {
    id: String,
    tipo: i32,
    folio: i64,
    fecha_emision: DateTime<Utc>,
    rut_emisor: String,
    rut_receptor: String,
    razon_social_receptor: String,
    #[serde(with = "rust_decimal::serde::str")]
    monto_total: Decimal,
    estado: String,
    track_id: Option<i64>,
    sii_glosa: Option<String>,
    order_id: Option<String>,
    /// `true` si hay XML firmado descargable en `GET /api/v1/dte/{id}/xml`.
    has_xml: bool,
}

impl From<DteRow> for DteDto {
    fn from(r: DteRow) -> Self {
        Self {
            id: r.id.to_string(),
            tipo: r.tipo,
            folio: r.folio,
            fecha_emision: r.fecha_emision,
            rut_emisor: r.rut_emisor,
            rut_receptor: r.rut_receptor,
            razon_social_receptor: r.razon_social_receptor,
            monto_total: r.monto_total,
            estado: r.estado,
            track_id: r.track_id,
            sii_glosa: r.sii_glosa,
            order_id: r.order_id.map(|t| t.to_string()),
            has_xml: r.xml_firmado.is_some(),
        }
    }
}

async fn load_dte(db: &db::Db, tenant: &Thing, id: &Thing) -> Result<DteRow, ApiError> {
    let mut q = db
        .query("SELECT * FROM $id WHERE tenant = $t")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await
        .map_err(db_err("load_dte"))?;
    let row: Option<DteRow> = q.take(0).map_err(db_err("load_dte decode"))?;
    row.ok_or_else(ApiError::not_found)
}

async fn load_emisor(db: &db::Db, tenant: &Thing) -> Result<dte::EmisorConfig, ApiError> {
    let setting = domain::sales::service::get_setting(db, tenant, EMISOR_SETTING_KEY).await?;
    let Some(s) = setting else {
        return Err(ApiError::invalid(
            "Falta configurar el emisor DTE: PUT /api/v1/settings/dte.emisor con el JSON \
             {\"rut\",\"razon_social\",\"giro\",\"direccion\",\"comuna\"}.",
        ));
    };
    serde_json::from_str(&s.value).map_err(|e| {
        ApiError::invalid(format!(
            "admin_setting dte.emisor no es un JSON de emisor válido: {e}"
        ))
    })
}

async fn sii_env_of(db: &db::Db, tenant: &Thing) -> Result<dte::SiiEnv, ApiError> {
    let v = domain::sales::service::get_setting(db, tenant, SII_ENV_SETTING_KEY).await?;
    Ok(match v.map(|s| s.value.to_ascii_lowercase()) {
        Some(ref s) if s == "prod" => dte::SiiEnv::Prod,
        _ => dte::SiiEnv::Sandbox,
    })
}

/// Carga el cert vigente del tenant, lo descifra con `passphrase` y arma el
/// `KeyMaterial` de firma. El blob es el PFX/PKCS#12 tal-como-importado
/// (parse nativo 9.1.b.3) o, back-compat, un bundle PEM del workaround
/// `openssl pkcs12 -nodes` — `from_keystore_bytes` detecta el formato.
async fn load_keymaterial(
    db: &db::Db,
    tenant: &Thing,
    passphrase: &str,
) -> Result<dte::KeyMaterial, ApiError> {
    if passphrase.is_empty() {
        return Err(ApiError::invalid("cert_passphrase requerida para firmar."));
    }
    let tenant_id = pharma_core::tenant::TenantId::new(tenant.id.to_raw());
    let enc = dte::cert::load_cert(db, tenant_id).await?;
    let Some(enc) = enc else {
        return Err(ApiError::conflict(
            "No hay certificado digital vigente para el tenant. Importa uno con \
             `pharma cert import <cert.pfx>`.",
        ));
    };
    let plain = dte::cert::decrypt_pfx(&enc, passphrase)?;
    Ok(dte::KeyMaterial::from_keystore_bytes(&plain, passphrase)?)
}

fn caf_from_record(r: &dte::caf::CafRecord) -> Result<dte::Caf, ApiError> {
    Ok(dte::Caf {
        // In-memory only: el TED usa el XML + rango, no este uuid.
        id: uuid::Uuid::new_v4(),
        tipo_dte: dte::DteTipo::from_code(r.tipo_dte)?,
        folio_desde: r.folio_desde,
        folio_hasta: r.folio_hasta,
        next_folio: r.next_folio,
        fecha_autorizacion: r.fecha_autorizacion,
        rut_emisor: r.rut_emisor.clone(),
        xml: r.xml.clone(),
        activo: r.activo,
    })
}

/// Subtree `<TED>..</TED>` del XML firmado, persistido aparte para reimpresión
/// (el PDF417 del voucher se genera desde el TED).
fn extract_ted(xml: &str) -> Option<String> {
    let start = xml.find("<TED")?;
    let end = xml.find("</TED>")? + "</TED>".len();
    (end > start).then(|| xml[start..end].to_string())
}

/// Mapea `license::Tier` → `dte::SendTier` (frontera deliberada: `crates/dte`
/// no conoce `crates/license` — ver doc de `dte::gating`).
fn send_tier_of(t: license::Tier) -> dte::SendTier {
    match t {
        license::Tier::Free => dte::SendTier::Free,
        license::Tier::Pro => dte::SendTier::Pro,
        license::Tier::Business => dte::SendTier::Business,
        license::Tier::Enterprise => dte::SendTier::Enterprise,
    }
}

// --- POST /api/v1/dte/boletas ----------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct EmitBoletaRequest {
    /// Orden POS origen (`order:<key>`). La boleta toma items y total de ahí.
    order_id: String,
    /// Passphrase del cert digital (descifra el PFX/PEM encrypt-at-rest).
    cert_passphrase: String,
    /// RUT receptor; default consumidor final `66666666-6`.
    #[serde(default)]
    receptor_rut: Option<String>,
    #[serde(default)]
    razon_social_receptor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrderRow {
    status: String,
    total: Decimal,
}

#[derive(Debug, Deserialize)]
struct OrderItemRow {
    product_name: String,
    quantity: i64,
    unit_price: Decimal,
    subtotal: Decimal,
}

/// Emite la boleta electrónica (tipo 39) de una orden POS: asigna folio CAF
/// (atómico), renderiza el XML SII, inyecta el TED y firma XML-DSig con el
/// cert de la empresa. Queda `signed` local — enviable al SII vía
/// `POST /api/v1/dte/{id}/send`. Free tier OK (emisión local sin gate).
#[utoipa::path(post, path = "/api/v1/dte/boletas", tag = "DTE",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Boleta emitida y firmada (estado signed)", body = serde_json::Value),
        (status = 400, description = "Emisor/cert/CAF mal configurados", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere cashier+)", body = crate::error::ErrorEnvelope),
        (status = 404, description = "Orden no encontrada", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Boleta ya emitida / folios agotados / sin cert vigente", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn emit_boleta(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<EmitBoletaRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let created_by = user_of(&claims).ok();

    // 1. Orden origen (tenant-scoped) en estado vendible.
    let order_thing = surrealdb::sql::thing(&req.order_id)
        .map_err(|_| ApiError::invalid(format!("order_id inválido: {}", req.order_id)))?;
    let mut q = db
        .query("SELECT status, total FROM order WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", order_thing.clone()))
        .bind(("t", tenant.clone()))
        .await
        .map_err(db_err("load order"))?;
    let order: Option<OrderRow> = q.take(0).map_err(db_err("decode order"))?;
    let order = order.ok_or_else(ApiError::not_found)?;
    if order.status != "paid" {
        return Err(ApiError::conflict(format!(
            "La orden está en estado '{}' — sólo órdenes 'paid' emiten boleta.",
            order.status
        )));
    }

    // 2. Idempotencia de negocio: una boleta viva por orden.
    let mut q = db
        .query(
            "SELECT id FROM dte WHERE tenant = $t AND order_id = $o \
             AND estado != 'cancelled' LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("o", order_thing.clone()))
        .await
        .map_err(db_err("dedup dte"))?;
    #[derive(Deserialize)]
    struct IdOnly {
        id: Thing,
    }
    let existing: Option<IdOnly> = q.take(0).map_err(db_err("decode dedup"))?;
    if let Some(e) = existing {
        return Err(ApiError::conflict("Ya existe una boleta para esta orden.")
            .with_details(serde_json::json!({ "dte_id": e.id.to_string() })));
    }

    // 3. Items de la orden → líneas DTE.
    let mut q = db
        .query(
            "SELECT product_name, quantity, unit_price, subtotal FROM order_item \
             WHERE tenant = $t AND order = $o",
        )
        .bind(("t", tenant.clone()))
        .bind(("o", order_thing.clone()))
        .await
        .map_err(db_err("load order items"))?;
    let items: Vec<OrderItemRow> = q.take(0).map_err(db_err("decode order items"))?;
    if items.is_empty() {
        return Err(ApiError::conflict("La orden no tiene items."));
    }

    // 4. Emisor + cert ANTES de quemar folio (validar todo lo barato primero).
    let emisor = load_emisor(db.as_ref(), &tenant).await?;
    let key = load_keymaterial(db.as_ref(), &tenant, &req.cert_passphrase).await?;

    // 5. Folio atómico del CAF activo.
    let (caf_record, folio) =
        dte::caf::assign_next(db.as_ref(), &tenant, dte::DteTipo::BoletaElectronica).await?;
    let caf = caf_from_record(&caf_record)?;

    // 6. Dte in-memory → render + TED + firma.
    let dte_items: Vec<dte::DteItem> = items
        .iter()
        .enumerate()
        .map(|(i, it)| dte::DteItem {
            nro_linea: (i + 1) as u32,
            nombre: it.product_name.clone(),
            cantidad: Decimal::from(it.quantity),
            precio_unitario: it.unit_price,
            descuento_pct: None,
            monto_item: it.subtotal,
            codigo_sku: None,
            unidad_medida: None,
            exento: false,
        })
        .collect();
    let doc = dte::Dte {
        id: uuid::Uuid::new_v4(),
        tipo: dte::DteTipo::BoletaElectronica,
        folio,
        fecha_emision: Utc::now(),
        rut_emisor: emisor.rut.clone(),
        rut_receptor: req
            .receptor_rut
            .clone()
            .unwrap_or_else(|| RUT_CONSUMIDOR_FINAL.to_string()),
        razon_social_receptor: req
            .razon_social_receptor
            .clone()
            .unwrap_or_else(|| "SIN INFORMACION".to_string()),
        giro_receptor: None,
        direccion_receptor: None,
        comuna_receptor: None,
        ind_traslado: None,
        referencias: vec![],
        descuentos_globales: vec![],
        // Boleta en modo monto-total (IVA incluido); neto/iva desglosado es
        // opcional en el xsd 39 y el renderer lo soporta así.
        monto_neto: Decimal::ZERO,
        iva: Decimal::ZERO,
        monto_exento: Decimal::ZERO,
        monto_total: order.total,
        items: dte_items,
        estado: dte::DteEstado::Draft,
        xml_firmado: None,
        timbre: None,
        track_id: None,
        sii_glosa: None,
        metadata: None,
    };
    let signed_xml = dte::build_signed_dte(&doc, &emisor, &caf, &key)?;
    let ted = extract_ted(&signed_xml);

    // 7. Persistir como `signed`.
    let mut q = db
        .query(
            "CREATE dte SET tenant = $t, tipo = $tipo, folio = $folio, \
             fecha_emision = $fe, rut_emisor = $re, rut_receptor = $rr, \
             razon_social_receptor = $rs, monto_neto = $mn, iva = $iva, \
             monto_exento = $mx, monto_total = $mt, items = $items, \
             estado = 'signed', xml_firmado = $xml, timbre = $ted, \
             order_id = $ord, created_by = $user RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("tipo", doc.tipo.code()))
        .bind(("folio", folio))
        .bind(("fe", surrealdb::sql::Datetime::from(doc.fecha_emision)))
        .bind(("re", doc.rut_emisor.clone()))
        .bind(("rr", doc.rut_receptor.clone()))
        .bind(("rs", doc.razon_social_receptor.clone()))
        .bind(("mn", dec_val(doc.monto_neto)))
        .bind(("iva", dec_val(doc.iva)))
        .bind(("mx", dec_val(doc.monto_exento)))
        .bind(("mt", dec_val(doc.monto_total)))
        .bind((
            "items",
            serde_json::to_value(&doc.items).map_err(|e| {
                tracing::error!(error = %e, "dte: items serialize");
                ApiError::internal("Error interno al procesar el DTE.")
            })?,
        ))
        .bind(("xml", signed_xml))
        .bind(("ted", ted))
        .bind(("ord", order_thing))
        .bind(("user", created_by))
        .await
        .map_err(db_err("create dte"))?;
    let row: Option<DteRow> = q.take(0).map_err(db_err("decode created dte"))?;
    let row = row.ok_or_else(|| {
        tracing::error!("dte: CREATE no retornó fila (folio {folio} quemado)");
        ApiError::internal("Error interno al persistir el DTE.")
    })?;

    Ok((StatusCode::CREATED, Json(DteDto::from(row))).into_response())
}

// --- POST /api/v1/dte/documentos ----------------------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct ReceptorReq {
    rut: String,
    razon_social: String,
    giro: String,
    direccion: String,
    comuna: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct DocItemReq {
    nombre: String,
    #[schema(value_type = String)]
    cantidad: Decimal,
    #[schema(value_type = String)]
    precio_unitario: Decimal,
    #[serde(default)]
    exento: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct ReferenciaReq {
    /// Código SII del doc referenciado ("33", "39", "61", "52", "801"…).
    tipo_doc_ref: String,
    folio_ref: String,
    /// Fecha del documento referenciado, YYYY-MM-DD.
    fecha_ref: String,
    /// 1 anula, 2 corrige texto, 3 corrige montos (obligatorio en notas).
    #[serde(default)]
    cod_ref: Option<i32>,
    #[serde(default)]
    razon_ref: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct EmitDocumentoRequest {
    /// Tipo SII: 33 factura, 56 nota débito, 61 nota crédito, 52 guía.
    tipo: i32,
    cert_passphrase: String,
    receptor: ReceptorReq,
    items: Vec<DocItemReq>,
    #[serde(default)]
    referencias: Vec<ReferenciaReq>,
    /// Motivo del traslado (guía 52): 1 venta … 9 venta exportación.
    #[serde(default)]
    ind_traslado: Option<i32>,
    /// Link opcional a la orden POS de origen.
    #[serde(default)]
    order_id: Option<String>,
}

/// Emite factura (33), nota de débito (56), nota de crédito (61) o guía de
/// despacho (52): folio CAF atómico del tipo, render XML (subtask 9.1.f),
/// TED + firma XML-DSig. Queda `signed` local, enviable vía `/send`
/// (Business+ para tipos ≠ 39). Boleta 39 NO va por acá (usa
/// `POST /api/v1/dte/boletas`). Montos se calculan server-side de los items
/// (precios IVA-incluido, convención retail CL): neto = round(afecto/1.19).
#[utoipa::path(post, path = "/api/v1/dte/documentos", tag = "DTE",
    request_body = EmitDocumentoRequest,
    responses(
        (status = 201, description = "Documento emitido y firmado (estado signed)", body = serde_json::Value),
        (status = 400, description = "Tipo/receptor/items/referencias inválidos o emisor sin configurar", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 404, description = "Orden vinculada no encontrada", body = crate::error::ErrorEnvelope),
        (status = 409, description = "Folios agotados / sin cert vigente", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn emit_documento(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<EmitDocumentoRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let created_by = user_of(&claims).ok();

    // 1. Spec compartido (validaciones de items + desglose IVA viven en
    //    `dte::emit`, mismas para API y CLI). Tipo 39 lo rechaza el builder.
    let tipo = dte::DteTipo::from_code(req.tipo)?;
    let referencias: Vec<dte::emit::ReferenciaSpec> = req
        .referencias
        .iter()
        .map(|r| {
            Ok(dte::emit::ReferenciaSpec {
                tipo_doc_ref: r.tipo_doc_ref.clone(),
                folio_ref: r.folio_ref.clone(),
                fecha_ref: parse_day(&r.fecha_ref, "fecha_ref")?,
                cod_ref: r.cod_ref,
                razon_ref: r.razon_ref.clone(),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let spec = dte::emit::DocumentoSpec {
        tipo,
        receptor: dte::emit::ReceptorSpec {
            rut: req.receptor.rut.clone(),
            razon_social: req.receptor.razon_social.clone(),
            giro: req.receptor.giro.clone(),
            direccion: req.receptor.direccion.clone(),
            comuna: req.receptor.comuna.clone(),
        },
        items: req
            .items
            .iter()
            .map(|it| dte::emit::ItemSpec {
                nombre: it.nombre.clone(),
                cantidad: it.cantidad,
                precio_unitario: it.precio_unitario,
                exento: it.exento,
            })
            .collect(),
        referencias,
        ind_traslado: req.ind_traslado,
        descuentos_globales: vec![],
    };

    // 2. Orden vinculada (opcional, tenant-scoped).
    let order_thing = match &req.order_id {
        Some(raw) => {
            let t = surrealdb::sql::thing(raw)
                .map_err(|_| ApiError::invalid(format!("order_id inválido: {raw}")))?;
            let mut q = db
                .query("SELECT status, total FROM order WHERE id = $id AND tenant = $t LIMIT 1")
                .bind(("id", t.clone()))
                .bind(("t", tenant.clone()))
                .await
                .map_err(db_err("load order doc"))?;
            let order: Option<OrderRow> = q.take(0).map_err(db_err("decode order doc"))?;
            if order.is_none() {
                return Err(ApiError::not_found());
            }
            Some(t)
        }
        None => None,
    };

    // 3. Validar el spec ANTES de tocar emisor/cert/folio (errores baratos
    //    primero; el folio no se quema con un spec inválido).
    dte::emit::build_documento(&spec, 1, "validacion", Utc::now())?;

    // 4. Emisor + cert antes de quemar folio.
    let emisor = load_emisor(db.as_ref(), &tenant).await?;
    let key = load_keymaterial(db.as_ref(), &tenant, &req.cert_passphrase).await?;

    // 5. Folio atómico del CAF activo del tipo.
    let (caf_record, folio) = dte::caf::assign_next(db.as_ref(), &tenant, tipo).await?;
    let caf = caf_from_record(&caf_record)?;

    // 6. Dte in-memory → render + TED + firma (el renderer valida receptor
    //    completo, referencias de notas e ind_traslado de guía).
    let doc = dte::emit::build_documento(&spec, folio, &emisor.rut, Utc::now())?;
    let signed_xml = dte::build_signed_dte(&doc, &emisor, &caf, &key)?;
    let ted = extract_ted(&signed_xml);

    // 7. Persistir como `signed` (campos receptor/referencias: migración 0023).
    let mut q = db
        .query(
            "CREATE dte SET tenant = $t, tipo = $tipo, folio = $folio, \
             fecha_emision = $fe, rut_emisor = $re, rut_receptor = $rr, \
             razon_social_receptor = $rs, giro_receptor = $gr, \
             direccion_receptor = $dr, comuna_receptor = $cr, \
             ind_traslado = $it, referencias = $refs, \
             monto_neto = $mn, iva = $iva, monto_exento = $mx, \
             monto_total = $mt, items = $items, estado = 'signed', \
             xml_firmado = $xml, timbre = $ted, order_id = $ord, \
             created_by = $user RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("tipo", doc.tipo.code()))
        .bind(("folio", folio))
        .bind(("fe", surrealdb::sql::Datetime::from(doc.fecha_emision)))
        .bind(("re", doc.rut_emisor.clone()))
        .bind(("rr", doc.rut_receptor.clone()))
        .bind(("rs", doc.razon_social_receptor.clone()))
        .bind(("gr", doc.giro_receptor.clone()))
        .bind(("dr", doc.direccion_receptor.clone()))
        .bind(("cr", doc.comuna_receptor.clone()))
        .bind(("it", doc.ind_traslado))
        .bind((
            "refs",
            serde_json::to_value(&doc.referencias).map_err(|e| {
                tracing::error!(error = %e, "dte: referencias serialize");
                ApiError::internal("Error interno al procesar el DTE.")
            })?,
        ))
        .bind(("mn", dec_val(doc.monto_neto)))
        .bind(("iva", dec_val(doc.iva)))
        .bind(("mx", dec_val(doc.monto_exento)))
        .bind(("mt", dec_val(doc.monto_total)))
        .bind((
            "items",
            serde_json::to_value(&doc.items).map_err(|e| {
                tracing::error!(error = %e, "dte: items serialize");
                ApiError::internal("Error interno al procesar el DTE.")
            })?,
        ))
        .bind(("xml", signed_xml))
        .bind(("ted", ted))
        .bind(("ord", order_thing))
        .bind(("user", created_by))
        .await
        .map_err(db_err("create dte documento"))?;
    let row: Option<DteRow> = q.take(0).map_err(db_err("decode created documento"))?;
    let row = row.ok_or_else(|| {
        tracing::error!("dte: CREATE documento no retornó fila (folio {folio} quemado)");
        ApiError::internal("Error interno al persistir el DTE.")
    })?;

    Ok((StatusCode::CREATED, Json(DteDto::from(row))).into_response())
}

// --- POST /api/v1/dte/cert ---------------------------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct UploadCertRequest {
    /// Certificado digital PFX/PKCS#12 codificado en base64.
    pfx_base64: String,
    /// Passphrase del PFX (la misma que se teclea al emitir). No se persiste.
    cert_passphrase: String,
    /// RUT del propietario del cert (`12345678-9`).
    rut: String,
    /// Vigencia desde (YYYY-MM-DD).
    vigencia_desde: String,
    /// Vigencia hasta (YYYY-MM-DD).
    vigencia_hasta: String,
}

/// Sube el certificado digital (.pfx/.p12) del tenant encrypt-at-rest desde la
/// UI — mismo flujo que `pharma cert import`, pero el PFX viaja en base64. Se
/// valida que el material sea utilizable para firmar (parse PKCS#12 + clave RSA)
/// ANTES de cifrar y persistir, así una passphrase equivocada o un archivo que
/// no es cert se atrapa en el upload y no recién en la primera emisión. La
/// passphrase nunca se guarda ni se loguea (AES-256-GCM + Argon2id, ADR-0011
/// §cert). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/dte/cert", tag = "DTE",
    request_body = UploadCertRequest,
    responses(
        (status = 201, description = "Cert importado y cifrado", body = serde_json::Value),
        (status = 400, description = "base64/PFX/passphrase/vigencia inválidos", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn upload_cert(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<UploadCertRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    if req.cert_passphrase.is_empty() {
        return Err(ApiError::invalid("cert_passphrase requerida."));
    }
    if req.rut.trim().is_empty() {
        return Err(ApiError::invalid("rut del propietario requerido."));
    }
    let pfx = B64
        .decode(req.pfx_base64.trim().as_bytes())
        .map_err(|e| ApiError::invalid(format!("pfx_base64 inválido: {e}")))?;
    if pfx.is_empty() {
        return Err(ApiError::invalid("pfx_base64 vacío."));
    }
    let desde = parse_day(&req.vigencia_desde, "vigencia_desde")?;
    let hasta = parse_day(&req.vigencia_hasta, "vigencia_hasta")?;
    if hasta < desde {
        return Err(ApiError::invalid(format!(
            "vigencia invertida: {} > {}",
            req.vigencia_desde, req.vigencia_hasta
        )));
    }
    // Validar el material ANTES de cifrar/persistir (parse PKCS#12 o bundle PEM
    // + clave RSA). Un PFX con passphrase incorrecta falla acá.
    dte::KeyMaterial::from_keystore_bytes(&pfx, &req.cert_passphrase).map_err(|e| {
        ApiError::invalid(format!("el certificado no es utilizable para firmar: {e}"))
    })?;
    let encrypted = dte::cert::encrypt_pfx(&pfx, &req.cert_passphrase)
        .map_err(|e| ApiError::internal(format!("encrypt_pfx: {e}")))?;
    let tenant_id = pharma_core::tenant::TenantId::new(tenant.id.to_raw());
    let id = dte::cert::store_cert(
        db.as_ref(),
        tenant_id,
        &encrypted,
        (desde, hasta),
        req.rut.trim(),
    )
    .await
    .map_err(|e| ApiError::internal(format!("store_cert: {e}")))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id.to_string(),
            "rut": req.rut.trim(),
            "vigencia_desde": desde,
            "vigencia_hasta": hasta,
            "blob_bytes": encrypted.blob.len(),
        })),
    )
        .into_response())
}

// --- POST /api/v1/dte/caf ----------------------------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct UploadCafRequest {
    /// XML del CAF (Código de Autorización de Folios) entregado por el SII.
    xml: String,
}

/// Sube un CAF del SII (XML) desde la UI — mismo flujo que `pharma caf import`:
/// parsea + valida el rango de folios y lo persiste activo para el tenant. El
/// XML completo se conserva (el TED lo embebe inline). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/dte/caf", tag = "DTE",
    request_body = UploadCafRequest,
    responses(
        (status = 201, description = "CAF importado", body = serde_json::Value),
        (status = 400, description = "XML del CAF inválido", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn upload_caf(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<UploadCafRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    if req.xml.trim().is_empty() {
        return Err(ApiError::invalid("xml del CAF requerido."));
    }
    let caf = dte::caf::parse_xml(&req.xml)
        .map_err(|e| ApiError::invalid(format!("CAF inválido: {e}")))?;
    let mut q = db
        .query(
            "CREATE caf SET tenant = $tenant, tipo_dte = $tipo, \
             folio_desde = $desde, folio_hasta = $hasta, next_folio = $next, \
             fecha_autorizacion = $fa, rut_emisor = $rut, xml = $xml, \
             activo = $activo RETURN AFTER",
        )
        .bind(("tenant", tenant.clone()))
        .bind(("tipo", caf.tipo_dte.code()))
        .bind(("desde", caf.folio_desde))
        .bind(("hasta", caf.folio_hasta))
        .bind(("next", caf.next_folio))
        .bind(("fa", surrealdb::sql::Datetime::from(caf.fecha_autorizacion)))
        .bind(("rut", caf.rut_emisor.clone()))
        .bind(("xml", caf.xml.clone()))
        .bind(("activo", caf.activo))
        .await
        .map_err(db_err("create caf upload"))?;
    #[derive(Deserialize)]
    struct CafCreated {
        id: Thing,
        tipo_dte: i32,
        folio_desde: i64,
        folio_hasta: i64,
        next_folio: i64,
    }
    let row: Option<CafCreated> = q.take(0).map_err(db_err("decode caf upload"))?;
    let row = row.ok_or_else(|| ApiError::internal("CREATE caf no retornó fila."))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": row.id.to_string(),
            "tipo": row.tipo_dte,
            "folio_desde": row.folio_desde,
            "folio_hasta": row.folio_hasta,
            "next_folio": row.next_folio,
            "rut_emisor": caf.rut_emisor,
        })),
    )
        .into_response())
}

// --- GET /api/v1/dte ---------------------------------------------------------

fn default_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub(crate) struct DteListQuery {
    #[serde(default)]
    estado: Option<String>,
    #[serde(default)]
    tipo: Option<i32>,
    /// Inclusive, YYYY-MM-DD (UTC).
    #[serde(default)]
    from: Option<String>,
    /// Inclusive (día completo), YYYY-MM-DD (UTC).
    #[serde(default)]
    to: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

const ESTADOS: &[&str] = &[
    "draft",
    "signed",
    "sent",
    "accepted",
    "rejected",
    "cancelled",
];

/// Lista DTEs del tenant, filtrable por estado, tipo SII y rango de fechas.
#[utoipa::path(get, path = "/api/v1/dte", tag = "DTE",
    responses(
        (status = 200, description = "DTEs ordenados por fecha desc", body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn list_dtes(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(f): Query<DteListQuery>,
) -> Result<Json<Vec<DteDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    // WITH NOINDEX: el planner de SurrealDB 2.x a veces resuelve este filtro
    // compuesto contra el índice (tenant, estado, created_at) sin ver rows
    // recién escritos (race observado de forma reproducible en tests: la misma
    // row SÍ aparece vía el índice (tenant, order_id) del dedup). Table scan
    // determinista; volumen dte por tenant es pantalla-admin, no hot path.
    let mut sql = String::from("SELECT * FROM dte WITH NOINDEX WHERE tenant = $t");
    if f.estado.is_some() {
        sql.push_str(" AND estado = $estado");
    }
    if f.tipo.is_some() {
        sql.push_str(" AND tipo = $tipo");
    }
    if f.from.is_some() {
        sql.push_str(" AND fecha_emision >= $from");
    }
    if f.to.is_some() {
        sql.push_str(" AND fecha_emision < $to");
    }
    sql.push_str(" ORDER BY fecha_emision DESC LIMIT $limit");

    let mut q = db.query(sql).bind(("t", tenant));
    if let Some(e) = &f.estado {
        let estado = e.to_lowercase();
        if !ESTADOS.contains(&estado.as_str()) {
            return Err(ApiError::invalid(format!(
                "estado inválido '{estado}' (esperado: {})",
                ESTADOS.join("|")
            )));
        }
        q = q.bind(("estado", estado));
    }
    if let Some(t) = f.tipo {
        dte::DteTipo::from_code(t)?;
        q = q.bind(("tipo", t));
    }
    if let Some(s) = &f.from {
        q = q.bind((
            "from",
            surrealdb::sql::Datetime::from(parse_day(s, "from")?),
        ));
    }
    if let Some(s) = &f.to {
        // `to` inclusive día completo → bound exclusivo al día siguiente.
        let excl = parse_day(s, "to")? + chrono::Duration::days(1);
        q = q.bind(("to", surrealdb::sql::Datetime::from(excl)));
    }
    let limit = f.limit.clamp(1, 500) as i64;
    q = q.bind(("limit", limit));

    let mut res = q.await.map_err(db_err("list dte"))?;
    let rows: Vec<DteRow> = res.take(0).map_err(db_err("decode list"))?;
    Ok(Json(rows.into_iter().map(DteDto::from).collect()))
}

// --- GET /api/v1/dte/caf-status ----------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct CafStatusQuery {
    /// Tipo SII (default 39 boleta).
    #[serde(default)]
    tipo: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct CafRow {
    id: Thing,
    folio_desde: i64,
    folio_hasta: i64,
    next_folio: i64,
}

/// Folios disponibles por CAF activo del tipo dado. El cliente avisa "quedan
/// N folios" antes de que el mesón se quede sin boletas.
#[utoipa::path(get, path = "/api/v1/dte/caf-status", tag = "DTE",
    responses(
        (status = 200, description = "CAFs activos + folios restantes", body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn caf_status(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<CafStatusQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let tipo = q.tipo.unwrap_or(39);
    dte::DteTipo::from_code(tipo)?;

    let mut res = db
        .query(
            "SELECT id, folio_desde, folio_hasta, next_folio FROM caf \
             WHERE tenant = $t AND tipo_dte = $tipo AND activo = true \
             ORDER BY folio_desde ASC",
        )
        .bind(("t", tenant))
        .bind(("tipo", tipo))
        .await
        .map_err(db_err("caf status"))?;
    let rows: Vec<CafRow> = res.take(0).map_err(db_err("decode caf status"))?;

    let mut total: i64 = 0;
    let cafs: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let restantes = (r.folio_hasta - r.next_folio + 1).max(0);
            total += restantes;
            serde_json::json!({
                "id": r.id.to_string(),
                "folio_desde": r.folio_desde,
                "folio_hasta": r.folio_hasta,
                "next_folio": r.next_folio,
                "restantes": restantes,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "tipo": tipo,
        "folios_restantes": total,
        "cafs": cafs,
    })))
}

// --- GET /api/v1/dte/{id} + /xml ----------------------------------------------

/// Detalle de un DTE.
#[utoipa::path(get, path = "/api/v1/dte/{id}", tag = "DTE",
    params(("id" = String, Path, description = "Id `dte:<key>` o `<key>`")),
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn get_dte(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<DteDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let row = load_dte(db.as_ref(), &tenant, &dte_thing(&id)?).await?;
    Ok(Json(DteDto::from(row)))
}

/// XML firmado del DTE (`application/xml`). Free tier: esto es el export que
/// el contribuyente sube manual al portal SII (ADR-0005 sin lock-in).
#[utoipa::path(get, path = "/api/v1/dte/{id}/xml", tag = "DTE",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "XML DTE firmado", content_type = "application/xml"),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "DTE sin XML firmado", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn dte_xml(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let row = load_dte(db.as_ref(), &tenant, &dte_thing(&id)?).await?;
    let xml = row.xml_firmado.ok_or_else(|| {
        ApiError::conflict(format!("DTE en estado '{}' sin XML firmado.", row.estado))
    })?;
    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
        .into_response())
}

// --- GET /api/v1/dte/libro-ventas ------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct LibroVentasQuery {
    /// Período contable `YYYY-MM`.
    period: String,
}

/// Subset de la fila `dte` que el libro necesita (sin `items`/`xml_firmado`).
#[derive(Debug, Deserialize)]
struct LibroRow {
    tipo: i32,
    folio: i64,
    fecha_emision: DateTime<Utc>,
    rut_emisor: String,
    rut_receptor: String,
    razon_social_receptor: String,
    monto_neto: Decimal,
    iva: Decimal,
    monto_exento: Decimal,
    monto_total: Decimal,
}

/// Libro de Ventas mensual (subtask 9.1.g): XML `LibroCompraVenta` SII con
/// todos los DTEs `accepted` del período. Sin movimientos → libro vacío
/// (caratula + resumen vacío, SII lo acepta). El XML NO va firmado acá
/// (firma `EnvioLibro` es subtask aparte) — sirve para revisión contable y
/// carga manual.
#[utoipa::path(get, path = "/api/v1/dte/libro-ventas", tag = "DTE",
    params(("period" = String, Query, description = "Período contable YYYY-MM")),
    responses(
        (status = 200, description = "XML LibroCompraVenta del período", content_type = "application/xml"),
        (status = 400, description = "Período inválido o emisor sin configurar", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn libro_ventas(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<LibroVentasQuery>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let xml = build_libro_xml(db.as_ref(), &tenant, &q.period).await?;
    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
        .into_response())
}

/// Render compartido del `LibroCompraVenta` del período (sin firma). Usado
/// por el GET (revisión/carga manual) y el POST firmado.
async fn build_libro_xml(db: &db::Db, tenant: &Thing, period: &str) -> Result<String, ApiError> {
    let periodo =
        chrono::NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").map_err(|_| {
            ApiError::invalid(format!("'period' inválido '{period}' (esperado YYYY-MM)"))
        })?;
    let from = Utc.from_utc_datetime(&periodo.and_hms_opt(0, 0, 0).expect("00:00:00 válido"));
    let next = if periodo.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(periodo.year() + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(periodo.year(), periodo.month() + 1, 1)
    }
    .expect("primer día de mes válido");
    let to = Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0).expect("00:00:00 válido"));

    let emisor = load_emisor(db, tenant).await?;

    // WITH NOINDEX: mismo gotcha del planner que list_dtes (rows frescas
    // invisibles vía índice compuesto). Volumen mensual por tenant es chico.
    let mut res = db
        .query(
            "SELECT tipo, folio, fecha_emision, rut_emisor, rut_receptor, \
             razon_social_receptor, monto_neto, iva, monto_exento, monto_total \
             FROM dte WITH NOINDEX WHERE tenant = $t AND estado = 'accepted' \
             AND fecha_emision >= $from AND fecha_emision < $to \
             ORDER BY folio ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("from", surrealdb::sql::Datetime::from(from)))
        .bind(("to", surrealdb::sql::Datetime::from(to)))
        .await
        .map_err(db_err("libro ventas"))?;
    let rows: Vec<LibroRow> = res.take(0).map_err(db_err("decode libro ventas"))?;

    let dtes: Vec<dte::Dte> = rows
        .into_iter()
        .map(|r| {
            Ok(dte::Dte {
                id: uuid::Uuid::nil(),
                tipo: dte::DteTipo::from_code(r.tipo)?,
                folio: r.folio,
                fecha_emision: r.fecha_emision,
                rut_emisor: r.rut_emisor,
                rut_receptor: r.rut_receptor,
                razon_social_receptor: r.razon_social_receptor,
                giro_receptor: None,
                direccion_receptor: None,
                comuna_receptor: None,
                ind_traslado: None,
                referencias: vec![],
                descuentos_globales: vec![],
                monto_neto: r.monto_neto,
                iva: r.iva,
                monto_exento: r.monto_exento,
                monto_total: r.monto_total,
                items: Vec::new(),
                estado: dte::DteEstado::Accepted,
                xml_firmado: None,
                timbre: None,
                track_id: None,
                sii_glosa: None,
                metadata: None,
            })
        })
        .collect::<Result<_, ApiError>>()?;

    let tenant_id = pharma_core::tenant::TenantId::new(tenant.id.to_raw());
    Ok(dte::xml::libro::render_libro_ventas(
        &emisor, tenant_id, periodo, &dtes,
    )?)
}

// --- POST /api/v1/dte/libro-ventas/signed -----------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct LibroSignedReq {
    /// Período contable `YYYY-MM`.
    period: String,
    /// Passphrase del cert digital (la misma del PFX importado).
    cert_passphrase: String,
}

/// Libro de Ventas mensual FIRMADO (firma `EnvioLibro`): mismo XML del GET
/// más la firma XML-DSig enveloped sobre `<EnvioLibro>` con el cert digital
/// de la empresa — listo para carga al portal SII. POST (no GET) porque la
/// passphrase del cert no debe viajar en query string.
#[utoipa::path(post, path = "/api/v1/dte/libro-ventas/signed", tag = "DTE",
    request_body = LibroSignedReq,
    responses(
        (status = 200, description = "XML LibroCompraVenta firmado", content_type = "application/xml"),
        (status = 400, description = "Período inválido, emisor o cert sin configurar", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn libro_ventas_signed(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<LibroSignedReq>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let xml = build_libro_xml(db.as_ref(), &tenant, &req.period).await?;
    let key = load_keymaterial(db.as_ref(), &tenant, &req.cert_passphrase).await?;
    let signed = dte::sign_libro(&xml, &key)?;
    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        signed,
    )
        .into_response())
}

// --- POST /api/v1/dte/{id}/send ------------------------------------------------

/// Envía el DTE firmado al SII (multipart DTEUpload). **Tier-gated** (9.1.j):
/// boleta 39 requiere Pro+; factura/NC/ND/guía requieren Business+. Free =
/// local-only (402 FEATURE_REQUIRES_UPGRADE).
#[utoipa::path(post, path = "/api/v1/dte/{id}/send", tag = "DTE",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "Enviado: estado sent + track_id", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 402, description = "Tier insuficiente para envío automático", body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "Estado no enviable", body = crate::error::ErrorEnvelope),
        (status = 422, description = "SII rechazó el envío", body = crate::error::ErrorEnvelope),
        (status = 502, description = "Error de red contra SII", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn send_dte(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<DteDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let thing = dte_thing(&id)?;
    let row = load_dte(db.as_ref(), &tenant, &thing).await?;

    if row.estado != "signed" {
        return Err(ApiError::conflict(format!(
            "DTE en estado '{}' — sólo 'signed' se envía al SII.",
            row.estado
        )));
    }
    let xml = row.xml_firmado.clone().ok_or_else(|| {
        ApiError::conflict("DTE signed sin XML firmado (inconsistencia interna).")
    })?;
    let tipo = dte::DteTipo::from_code(row.tipo)?;

    // Gate de tier ANTES de tocar red (free → 402 sin side effects).
    let lic = state.license.load();
    dte::require_send_allowed(send_tier_of(lic.tier), tipo)?;

    let env = sii_env_of(db.as_ref(), &tenant).await?;
    let result = dte::sii::upload_dte(env, &xml, &[], "", &row.rut_emisor, &row.rut_emisor).await?;

    let mut q = db
        .query("UPDATE $id SET estado = 'sent', track_id = $tid RETURN AFTER")
        .bind(("id", thing))
        .bind(("tid", result.track_id))
        .await
        .map_err(db_err("update sent"))?;
    let updated: Option<DteRow> = q.take(0).map_err(db_err("decode sent"))?;
    let updated = updated.ok_or_else(ApiError::service_unavailable)?;
    Ok(Json(DteDto::from(updated)))
}

// --- POST /api/v1/dte/{id}/poll -------------------------------------------------

#[derive(Debug, Serialize)]
pub(crate) struct PollResponse {
    #[serde(flatten)]
    dte: DteDto,
    /// Estado SII normalizado de esta consulta:
    /// `aceptado|aceptado_con_reparos|rechazado|recibido|en_proceso|error`.
    sii_estado: &'static str,
}

/// Consulta en SII el veredicto de un DTE enviado (`QueryEstUp` por track_id)
/// y actualiza estado local: aceptado → `accepted`, rechazado → `rejected`,
/// en trámite → sigue `sent`. Idempotente sobre DTEs ya resueltos (no re-llama
/// al SII).
#[utoipa::path(post, path = "/api/v1/dte/{id}/poll", tag = "DTE",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "Estado refrescado", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "DTE sin envío SII", body = crate::error::ErrorEnvelope),
        (status = 502, description = "Error de red contra SII", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn poll_dte(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<PollResponse>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let thing = dte_thing(&id)?;
    let row = load_dte(db.as_ref(), &tenant, &thing).await?;

    // Ya resuelto → respuesta idempotente sin red.
    if row.estado == "accepted" || row.estado == "rejected" {
        let sii_estado = if row.estado == "accepted" {
            "aceptado"
        } else {
            "rechazado"
        };
        return Ok(Json(PollResponse {
            dte: DteDto::from(row),
            sii_estado,
        }));
    }
    if row.estado != "sent" {
        return Err(ApiError::conflict(format!(
            "DTE en estado '{}' sin envío SII que consultar.",
            row.estado
        )));
    }
    let track_id = row
        .track_id
        .ok_or_else(|| ApiError::conflict("DTE 'sent' sin track_id (inconsistencia interna)."))?;

    let env = sii_env_of(db.as_ref(), &tenant).await?;
    let poll = dte::sii::poll_status(env, track_id, &row.rut_emisor).await?;

    use dte::sii::SiiEstado as S;
    let (nuevo_estado, sii_estado) = match poll.estado {
        S::Aceptado => (Some("accepted"), "aceptado"),
        S::AceptadoConReparos => (Some("accepted"), "aceptado_con_reparos"),
        S::Rechazado => (Some("rejected"), "rechazado"),
        S::Recibido => (None, "recibido"),
        S::EnProceso => (None, "en_proceso"),
        S::Error => (None, "error"),
    };

    let mut q = db
        .query("UPDATE $id SET estado = $estado, sii_glosa = $glosa RETURN AFTER")
        .bind(("id", thing))
        .bind(("estado", nuevo_estado.unwrap_or("sent").to_string()))
        .bind(("glosa", poll.glosa.clone()))
        .await
        .map_err(db_err("update poll"))?;
    let updated: Option<DteRow> = q.take(0).map_err(db_err("decode poll"))?;
    let updated = updated.ok_or_else(ApiError::service_unavailable)?;
    Ok(Json(PollResponse {
        dte: DteDto::from(updated),
        sii_estado,
    }))
}

// --- POST /api/v1/dte/{id}/cancel ------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct CancelRequest {
    reason: String,
}

/// Anula un DTE pre-envío (`draft|signed → cancelled`). DTEs ya enviados al
/// SII no se anulan acá: eso es nota de crédito (61), subtask 9.1.f.
#[utoipa::path(post, path = "/api/v1/dte/{id}/cancel", tag = "DTE",
    params(("id" = String, Path)),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "DTE anulado", body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Requiere admin+", body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "Transición de estado no permitida", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn cancel_dte(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CancelRequest>,
) -> Result<Json<DteDto>, ApiError> {
    if req.reason.trim().is_empty() {
        return Err(ApiError::invalid("reason requerida para anular."));
    }
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let thing = dte_thing(&id)?;
    let row = load_dte(db.as_ref(), &tenant, &thing).await?;

    // Stub mínimo para que `cancel_dte` valide la transición y escriba el
    // trail en metadata (mismo patrón que la CLI `pharma dte cancel`).
    let mut stub = stub_for_transition(&row)?;
    dte::cancel_dte(&mut stub, req.reason.trim())?;

    let mut q = db
        .query("UPDATE $id SET estado = 'cancelled', metadata = $m RETURN AFTER")
        .bind(("id", thing))
        .bind(("m", stub.metadata))
        .await
        .map_err(db_err("update cancel"))?;
    let updated: Option<DteRow> = q.take(0).map_err(db_err("decode cancel"))?;
    let updated = updated.ok_or_else(ApiError::service_unavailable)?;
    Ok(Json(DteDto::from(updated)))
}

/// `dte::Dte` mínimo desde un row persistido — sólo los campos que las
/// transiciones de estado leen; el UPDATE posterior toca estado + metadata.
fn stub_for_transition(row: &DteRow) -> Result<dte::Dte, ApiError> {
    let estado = match row.estado.as_str() {
        "draft" => dte::DteEstado::Draft,
        "signed" => dte::DteEstado::Signed,
        "sent" => dte::DteEstado::Sent,
        "accepted" => dte::DteEstado::Accepted,
        "rejected" => dte::DteEstado::Rejected,
        "cancelled" => dte::DteEstado::Cancelled,
        other => {
            tracing::error!(estado = other, "dte: estado desconocido en DB");
            return Err(ApiError::internal("Estado DTE desconocido."));
        }
    };
    Ok(dte::Dte {
        id: uuid::Uuid::nil(),
        tipo: dte::DteTipo::from_code(row.tipo)?,
        folio: row.folio,
        fecha_emision: row.fecha_emision,
        rut_emisor: row.rut_emisor.clone(),
        rut_receptor: row.rut_receptor.clone(),
        razon_social_receptor: row.razon_social_receptor.clone(),
        giro_receptor: None,
        direccion_receptor: None,
        comuna_receptor: None,
        ind_traslado: None,
        referencias: vec![],
        descuentos_globales: vec![],
        monto_neto: Decimal::ZERO,
        iva: Decimal::ZERO,
        monto_exento: Decimal::ZERO,
        monto_total: row.monto_total,
        items: Vec::new(),
        estado,
        xml_firmado: None,
        timbre: None,
        track_id: row.track_id,
        sii_glosa: row.sii_glosa.clone(),
        metadata: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ted_returns_subtree() {
        let xml = "<DTE><Documento ID=\"x\"><TED version=\"1.0\"><DD/></TED><TmstFirma/></Documento></DTE>";
        let ted = extract_ted(xml).unwrap();
        assert!(ted.starts_with("<TED"));
        assert!(ted.ends_with("</TED>"));
        assert!(extract_ted("<DTE></DTE>").is_none());
    }

    #[test]
    fn dte_thing_accepts_both_forms() {
        assert_eq!(dte_thing("dte:abc").unwrap().tb, "dte");
        assert_eq!(dte_thing("abc").unwrap().tb, "dte");
        assert!(dte_thing("order:abc").is_err());
        assert!(dte_thing("").is_err());
    }
}
