//! Respaldo cifrado del usuario (ADR-0022 / ADR-0023) — API de blob opaco.
//!
//! Distinto de [`super::backup`] (el tar.gz admin del directorio de Surreal).
//! Esta ruta recibe **sólo bytes ya cifrados** + metadatos. La frase de
//! recuperación no aparece nunca en el cable.
//!
//! ## La invariante
//!
//! **Ninguna ruta de este archivo puede producir plaintext.** El server recibe
//! un sobre `RB1` que salió cifrado del teléfono, lo guarda tal cual y lo
//! devuelve tal cual. No hay una llave acá, no hay una forma de pedirla, y no
//! hay un camino de código que descifre. Está fijado por
//! `crates/api/tests/user_backup_zero_knowledge.rs`, que es un test y no un
//! comentario porque un comentario no se pone rojo.
//!
//! ## Los tres caminos
//!
//! | ruta | quién | para qué |
//! |------|-------|----------|
//! | `POST /api/v1/user-backup` | JWT | subir el sobre del día |
//! | `GET /api/v1/user-backup` + `/{id}` | JWT | teléfono que todavía funciona |
//! | `POST /api/v1/user-backup/rescue` | **nadie** | teléfono nuevo, sin sesión |
//!
//! El tercero existe porque los dos primeros piden JWT, y alguien cuyo teléfono
//! se perdió no tiene forma de conseguir uno. Un respaldo que sólo se baja
//! desde el aparato que se rompió no es un respaldo. Ver [`rescue`].

pub mod repo_ops;
pub mod sigv4;
pub mod store;

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::user_backup as ub;

/// Lo que este nodo necesita para guardar respaldos: dónde ponerlos y con qué
/// límites.
///
/// Va por `Extension` y no adentro de `AppState` a propósito. `AppState` lo
/// construyen a mano dos docenas de tests con literal de struct, y un campo
/// nuevo los rompe a todos — incluidos archivos de otras ramas en vuelo. Como
/// además éste es un módulo opcional que **tiene** que degradar en vez de
/// tumbar el arranque (ADR-0005), el default correcto ya es
/// [`Runtime::deshabilitado`]: sin capa, la ruta contesta `accepted: false`,
/// que es exactamente lo que hacía antes de que existiera el bucket.
pub struct Runtime {
    pub store: Arc<dyn store::BlobStore>,
    pub cfg: pharma_core::config::UserBackupConfig,
}

impl Runtime {
    /// Resuelve el store desde la config. Una config a medias degrada a
    /// [`store::NoStore`]; no falla el arranque.
    pub fn from_config(cfg: &pharma_core::config::UserBackupConfig) -> Arc<Self> {
        Arc::new(Self {
            store: store::build(cfg),
            cfg: cfg.clone(),
        })
    }

    /// Valida y no guarda. El nodo que no configuró bucket, y todo test que no
    /// se ocupa del respaldo.
    pub fn deshabilitado() -> Arc<Self> {
        Arc::new(Self {
            store: Arc::new(store::NoStore),
            cfg: pharma_core::config::UserBackupConfig::default(),
        })
    }

    /// Store y cuotas explícitos. Lo usan los tests y cualquier nodo que arme
    /// el runtime a mano.
    pub fn nuevo(
        store: Arc<dyn store::BlobStore>,
        cfg: pharma_core::config::UserBackupConfig,
    ) -> Arc<Self> {
        Arc::new(Self { store, cfg })
    }

    /// Para tests: un store cualquiera con la config por default.
    pub fn con_store(store: Arc<dyn store::BlobStore>) -> Arc<Self> {
        Self::nuevo(store, pharma_core::config::UserBackupConfig::default())
    }

    fn quota(&self) -> ub::BackupQuota {
        ub::BackupQuota {
            max_envelope_bytes: self.cfg.max_envelope_bytes,
            max_versions_per_tenant: self.cfg.max_versions_per_tenant,
            min_seconds_between_uploads: self.cfg.min_seconds_between_uploads,
            max_uploads_per_day: self.cfg.max_uploads_per_day,
            retention_days: self.cfg.retention_days,
        }
    }
}

type Rt = Extension<Arc<Runtime>>;

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route(ub::USER_BACKUP_UPLOAD_PATH, post(upload).get(list))
        // El rescate va ANTES de `/{backup_id}` para que axum no le meta
        // "rescue" como id a la ruta de bajada.
        .route(ub::USER_BACKUP_RESCUE_PATH, post(rescue))
        .route(
            "/api/v1/user-backup/{backup_id}",
            get(download).delete(borrar),
        )
}

/// `tenant:abc` del JWT → `Thing`. Mismo patrón que `admin_web.rs`.
fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

// --- subir --------------------------------------------------------------------

/// `POST /api/v1/user-backup` — mete el sobre en el bucket.
///
/// Orden de los chequeos, y no es casual: primero forma (barato, sin I/O),
/// después cuota de tamaño (corta antes de tocar la base), después cuota de
/// frecuencia (una consulta), y recién al final el `PUT` al bucket, que es lo
/// caro y lo único que cuesta plata.
async fn upload(
    State(s): State<AppState>,
    Extension(rt): Rt,
    AuthUser(claims): AuthUser,
    Json(body): Json<ub::UploadEncryptedBackupRequest>,
) -> Result<axum::response::Response, ApiError> {
    let quota = rt.quota();

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(body.ciphertext_base64.as_bytes())
        .map_err(|_| ApiError::invalid("ciphertext_base64 no es base64 válido"))?;

    let sha_hex = hex::encode(Sha256::digest(&decoded));

    if let Err(e) = ub::validate_upload(&body.meta, &decoded, &sha_hex) {
        return Err(ApiError::invalid(e.message()));
    }

    // El tenant del JWT manda; el del body es informativo. Un cliente no puede
    // escribir en la carpeta de otro negocio diciendo que es de otro.
    let tenant_id = claims.tenant_id.clone();
    if tenant_id.is_empty() {
        return Err(ApiError::invalid("tenant vacío en la sesión"));
    }

    // Cuota de tamaño: antes de la base y antes del bucket.
    let ahora = now_unix();
    if let Err(q) = ub::check_quota(
        &quota,
        &ub::UploadAttempt::solo_tamano(decoded.len() as u64, ahora),
    ) {
        return Err(quota_error(q));
    }

    // Sin dónde guardar, se contesta honesto y no se inventa una nube.
    if !rt.store.guarda() {
        return Ok(Json(ub::UploadEncryptedBackupResponse {
            accepted: false,
            reason: Some(no_store_reason(&rt)),
            backup_id: None,
        })
        .into_response());
    }

    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;

    // Cuotas de frecuencia — control de costo antes que de abuso: los PUT son
    // el 96% de la factura del bucket, no los bytes. Son dos y hacen cosas
    // distintas: el piso frena la ráfaga, el tope diario acota el gasto.
    let dia = domain::user_backup_repo::dia_utc(chrono::Utc::now());
    let last = domain::user_backup_repo::last_upload(&db, &tenant)
        .await
        .map_err(ApiError::from)?;
    let hoy = domain::user_backup_repo::uploads_hoy(&db, &tenant, &dia)
        .await
        .map_err(ApiError::from)?;
    if let Err(q) = ub::check_quota(
        &quota,
        &ub::UploadAttempt {
            envelope_len: decoded.len() as u64,
            now_unix: ahora,
            last_upload_unix: last.map(|d| d.timestamp()),
            uploads_last_24h: hoy,
        },
    ) {
        return Err(quota_error(q));
    }

    if let Some(h) = body.retrieval_hash_hex.as_deref() {
        if !is_sha256_hex(h) {
            return Err(ApiError::invalid(
                "retrieval_hash_hex debe ser 64 hex chars",
            ));
        }
    }

    let backup_id = nuevo_backup_id(&sha_hex);
    let key = store::object_key(&rt.cfg.prefix, &tenant_id, &backup_id)
        .map_err(|_| ApiError::internal("clave de objeto inválida"))?;

    let size = decoded.len() as u64;
    // El objeto va PRIMERO y el índice después. Si se cae en el medio queda un
    // objeto huérfano —unos centavos hasta la barrida de retención— en vez de
    // una fila que promete un respaldo que el bucket no tiene.
    rt.store.put(&key, decoded).await.map_err(|e| {
        tracing::error!(error = %e, "user-backup: el bucket rechazó el PUT");
        ApiError::internal("no se pudo guardar el respaldo")
    })?;

    let row = domain::user_backup_repo::insert(
        &db,
        &tenant,
        &domain::user_backup_repo::NewBackup {
            backup_id: &backup_id,
            object_key: &key,
            format_version: body.meta.format_version,
            ciphertext_sha256: &sha_hex,
            size_bytes: size,
            label: body.meta.label.as_deref(),
            retrieval_hash: body.retrieval_hash_hex.as_deref(),
        },
    )
    .await
    .map_err(ApiError::from)?;

    // El contador se suma acá y no antes: un PUT que falló no cuesta Class A y
    // no tiene por qué gastarle el cupo del día a la dueña.
    if let Err(e) = domain::user_backup_repo::contar_subida(&db, &tenant, &dia).await {
        // Que no se pueda contar no es motivo para rechazar un respaldo que ya
        // está en el bucket. Se registra y sigue: el peor caso es una subida
        // extra en el día, no una venta sin respaldar.
        tracing::warn!(error = %e, "user-backup: no se pudo contar la subida del día");
    }

    // Rotación: la nueva entró, la más vieja sale. En R2 `DeleteObject` es
    // gratis, así que esto no suma factura; no rotar, sí.
    repo_ops::rotate(&rt, &db, &tenant).await;

    Ok(Json(ub::UploadEncryptedBackupResponse {
        accepted: true,
        reason: None,
        backup_id: Some(row.backup_id),
    })
    .into_response())
}

/// Id opaco del sobre: fecha + trozo del sha. La fecha adelante deja el listado
/// del bucket ordenado si algún día hay que mirarlo a mano.
fn nuevo_backup_id(sha_hex: &str) -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        &sha_hex[..12.min(sha_hex.len())]
    )
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// La cuota rechazó la subida → error HTTP, **conservando el texto**.
///
/// `ApiError::rate_limited` trae un mensaje genérico ("demasiadas solicitudes")
/// y acá eso no sirve: lo que la dueña necesita leer es que su venta no se
/// perdió, que sigue en el teléfono, y en cuántos minutos reintentar. Por eso
/// se arma el 429 a mano con el mensaje de [`ub::QuotaRejection::message`] en
/// vez de llamar al helper.
fn quota_error(q: ub::QuotaRejection) -> ApiError {
    match q.retry_after_seconds() {
        Some(secs) => ApiError::new(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", q.message())
            .with_details(serde_json::json!({
                "scope": "user-backup",
                "retry_after_secs": secs,
            })),
        None => ApiError::invalid(q.message()),
    }
}

fn no_store_reason(rt: &Runtime) -> String {
    let kind = rt.store.kind();
    rt.cfg.why_not_storing().unwrap_or_else(|| {
        format!(
            "el respaldo se recibió y se validó, pero este nodo usa un store \
             {kind} que no es durable. En este teléfono el cifrado ya está listo."
        )
    })
}

// --- listar y bajar (con sesión) ----------------------------------------------

/// `GET /api/v1/user-backup` — metadatos de los sobres de este negocio.
async fn list(
    State(s): State<AppState>,
    Extension(rt): Rt,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<ub::EncryptedBackupMeta>>, ApiError> {
    let Some(db) = s.db.clone() else {
        return Ok(Json(vec![]));
    };
    if !rt.store.guarda() {
        return Ok(Json(vec![]));
    }
    let tenant = tenant_of(&claims)?;
    let rows = domain::user_backup_repo::list_newest_first(&db, &tenant)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        rows.iter().map(|r| r.to_meta(&claims.tenant_id)).collect(),
    ))
}

/// `GET /api/v1/user-backup/{backup_id}` — el sobre, para un teléfono que
/// todavía tiene sesión. El de otro tenant contesta 404 igual que uno que no
/// existe: distinguirlos sería un oráculo de enumeración.
async fn download(
    State(s): State<AppState>,
    Extension(rt): Rt,
    AuthUser(claims): AuthUser,
    Path(backup_id): Path<String>,
) -> Result<Json<ub::DownloadEncryptedBackupResponse>, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;
    let row = domain::user_backup_repo::find_for_tenant(&db, &tenant, &backup_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(ApiError::not_found)?;

    let bytes = rt
        .store
        .get(&row.object_key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "user-backup: el bucket falló al bajar");
            ApiError::internal("no se pudo traer el respaldo")
        })?
        .ok_or_else(ApiError::not_found)?;

    Ok(Json(ub::DownloadEncryptedBackupResponse {
        meta: row.to_meta(&claims.tenant_id),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        backup_id: row.backup_id,
    }))
}

/// `DELETE /api/v1/user-backup/{backup_id}` — "borrá esto".
///
/// Con `{backup_id} = all` borra **todos** los respaldos del negocio. Es el
/// derecho a que se vayan los bytes, y tiene que existir en la misma API que
/// los subió: mandar a alguien a escribir un mail para que le borren sus datos
/// es no tener borrado.
async fn borrar(
    State(s): State<AppState>,
    Extension(rt): Rt,
    AuthUser(claims): AuthUser,
    Path(backup_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;

    let keys = if backup_id == "all" {
        domain::user_backup_repo::delete_all_for_tenant(&db, &tenant)
            .await
            .map_err(ApiError::from)?
    } else {
        domain::user_backup_repo::delete_rows(
            &db,
            &tenant,
            std::slice::from_ref(&backup_id),
        )
        .await
        .map_err(ApiError::from)?
    };

    // Índice primero (ya está), bucket después. Un objeto huérfano lo barre la
    // retención; una fila viva apuntando a un objeto borrado sería prometer un
    // respaldo que no está.
    for k in keys {
        if let Err(e) = rt.store.delete(&k).await {
            tracing::warn!(error = %e, key = %k, "user-backup: quedó objeto huérfano en el bucket");
        }
    }
    // 204 aunque no hubiera nada: el estado pedido ya es el que hay.
    Ok(StatusCode::NO_CONTENT)
}

// --- rescate sin sesión --------------------------------------------------------

/// `POST /api/v1/user-backup/rescue` — teléfono nuevo, app recién instalada.
///
/// ## Por qué esta ruta no pide JWT
///
/// Porque no puede. La persona que la necesita perdió el teléfono: no tiene
/// sesión, no tiene el token guardado, y si su negocio se creó con Google
/// tampoco tiene garantizado el acceso a esa cuenta desde un aparato nuevo. Si
/// la única puerta pidiera JWT, el respaldo no existiría — sería exactamente el
/// mismo agujero que había cuando el bucket no estaba cableado, con más código.
///
/// ## Qué la reemplaza
///
/// La tarjeta del cuaderno. El cliente deriva una **prueba de retiro** del
/// mismo material (slug + palabras/bloques) por una rama distinta a la de la
/// llave de cifrado, y manda la prueba. El server guardó `SHA-256(prueba)` al
/// subir y compara en tiempo constante.
///
/// Tres cosas que esto **no** rompe:
///
/// 1. El server sigue sin poder descifrar: de la prueba no se llega a la llave.
/// 2. Si se filtra la base, llegar a la semilla exige el mismo PBKDF2 de 210k
///    que protege al sobre (~2^101).
/// 3. Aunque alguien acertara la prueba, se lleva ciphertext que no puede
///    abrir. La confidencialidad nunca dependió de esta puerta.
///
/// Todo lo que falla contesta **404 sin cuerpo**: slug que no existe, prueba
/// que no calza, negocio sin respaldos. Distinguirlos le regalaría al que
/// prueba slugs la mitad del trabajo.
async fn rescue(
    State(s): State<AppState>,
    Extension(rt): Rt,
    headers: HeaderMap,
    Json(body): Json<ub::RescueBackupRequest>,
) -> Result<Json<ub::RescueBackupResponse>, ApiError> {
    if !rt.cfg.rescue_enabled || !rt.store.guarda() {
        return Err(ApiError::not_found());
    }
    let db = db_of(&s)?;

    let proof = ub::validate_rescue_request(&body).map_err(|_| ApiError::not_found())?;
    let slug = ub::normalize_slug(&body.tenant_slug);

    // El limitador global por IP ya cubre esta ruta (va antes de auth). Acá se
    // suma un freno propio y mucho más duro por slug: adivinar la prueba es
    // 2^84, pero un atacante que sabe el slug de un negocio conocido no tiene
    // por qué poder intentarlo miles de veces por minuto.
    if let Some(rl) = s.rate_limit.as_ref() {
        if rl.rescue_throttled(&slug) {
            return Err(ApiError::rate_limited("user-backup-rescue", 60));
        }
    }

    let hash = ub::retrieval_hash_hex(&proof);
    let found = domain::user_backup_repo::find_newest_by_retrieval(&db, &slug, &hash)
        .await
        .map_err(ApiError::from)?;

    let Some((tenant, row)) = found else {
        // Log sin el slug completo: un log de rescates fallidos con slugs es un
        // mapa de negocios para el que lea los logs.
        tracing::info!(
            ip = %primer_hop(&headers),
            "user-backup: rescate sin coincidencia"
        );
        return Err(ApiError::not_found());
    };

    // Nota sobre timing: la comparación de la prueba la hace el índice de la
    // base (`ub_retrieval`), no un `==` en este archivo, así que no hay dónde
    // meter una comparación en tiempo constante aunque se quisiera. Y no hace
    // falta: lo que se compara es el SHA-256 de un secreto de 84 bits, o sea
    // que el atacante tendría que sacar ~2^84 intentos por un canal de tiempo
    // sobre la red, contra un endpoint que además está limitado por slug y por
    // IP. El eslabón que protege esto es la entropía, no el `memcmp`.
    let bytes = rt
        .store
        .get(&row.object_key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "user-backup: el bucket falló en el rescate");
            ApiError::internal("no se pudo traer el respaldo")
        })?
        .ok_or_else(ApiError::not_found)?;

    tracing::info!(backup_id = %row.backup_id, "user-backup: rescate entregado");

    Ok(Json(ub::RescueBackupResponse {
        meta: row.to_meta(&tenant.to_string()),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        backup_id: row.backup_id,
    }))
}

fn primer_hop(headers: &HeaderMap) -> String {
    headers
        .get(header::HeaderName::from_static("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or("-")
        .trim()
        .to_string()
}
