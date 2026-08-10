//! Google Sign-In exchange (ADR-0022).
//!
//! Canjea el `id_token` que devuelve el selector de cuentas de Android por
//! **nuestro** JWT de sesión — el mismo que emite el login con clave, por el
//! mismo camino, para que `SessionRepository` en Android no necesite una rama
//! nueva.
//!
//! **Nunca se loguea el `id_token`.** Ni entero, ni un prefijo, ni "los
//! primeros 8 caracteres para debuggear". Es una credencial de portador viva
//! hasta su `exp`: cualquiera que la lea en un log entra como esa persona.
//! Lo que sí se loguea es el `sub` ya verificado, que es un id opaco y no
//! sirve para entrar.
//!
//! ## El gate: sin client id esto sigue siendo un 501
//!
//! Si el proceso no trae client id, la ruta responde **exactamente lo mismo
//! que antes de este carril**. Es lo que permite mergear antes de que existan
//! las credenciales en la nube: un despliegue sin configurar no cambia de
//! comportamiento, y el botón tampoco aparece en Android (ver
//! `IdentidadGoogle.disponible()`).
//!
//! ## De dónde sale el client id
//!
//! De `PHARMA_GOOGLE_CLIENT_ID` en el entorno. **No** de `AppConfig`, y es a
//! propósito: agregarlo a `pharma_core::config` y a `AppState` obliga a tocar
//! `crates/core/src/config.rs`, `config/default.toml` y `crates/api/src/lib.rs`
//! — los tres del carril del arranque, que justo ahora está moviendo el guard
//! del secreto JWT en `lib.rs`. Leerlo del entorno deja este carril mergeable
//! solo. Cuando ese carril cierre, mudarlo a `AppConfig.google.client_id` son
//! cinco líneas y este comentario se borra.
//!
//! El client id de una app Android **no es secreto** (viaja en el APK y
//! cualquiera lo ve en el tráfico). Igual entra por entorno y no por el repo:
//! Regla 3 no se negocia por excepciones, y el día que al lado haya que poner
//! algo que sí es secreto, el camino ya está hecho.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use axum::{extract::State, routing::post, Json, Router};
use domain::google_verify::{
    verificar_id_token, ErrorVerificacionGoogle, GoogleJwks, IdentidadGoogleVerificada,
};

use crate::error::ApiError;
use crate::AppState;

/// Dónde publica Google sus llaves públicas. Sobrescribible por entorno para
/// que los tests apunten a un servidor local en vez de salir a internet.
const JWKS_URL_POR_DEFECTO: &str = "https://www.googleapis.com/oauth2/v3/certs";

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route(
        domain::google_identity::GOOGLE_SIGN_IN_PATH,
        post(exchange),
    )
}

fn client_id() -> Option<String> {
    std::env::var("PHARMA_GOOGLE_CLIENT_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn jwks_url() -> String {
    std::env::var("PHARMA_GOOGLE_JWKS_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| JWKS_URL_POR_DEFECTO.to_string())
}

// ---------------------------------------------------------------------------
// Caché de llaves
// ---------------------------------------------------------------------------

/// Piso y techo de lo que se respeta del `Cache-Control` de Google.
///
/// Se respeta lo que Google dice, pero acotado: un `max-age` enorme por un bug
/// del otro lado nos dejaría con llaves muertas durante días, y uno de dos
/// segundos nos haría pegarle a Google en cada login.
const TTL_MINIMO: Duration = Duration::from_secs(60);
const TTL_MAXIMO: Duration = Duration::from_secs(24 * 60 * 60);
const TTL_SI_NO_DICE: Duration = Duration::from_secs(60 * 60);

/// Cada cuánto, como mucho, se acepta un refresco forzado por "kid descondido".
///
/// Sin este piso, mandar tokens con `kid` inventado es un amplificador: cada
/// pedido basura se convierte en un pedido nuestro a Google. Con él, una
/// avalancha de tokens falsos cuesta un fetch por minuto.
const MIN_ENTRE_REFRESCOS: Duration = Duration::from_secs(60);

struct Llaves {
    jwks: GoogleJwks,
    /// Cuándo dejan de servir según el `Cache-Control` de la respuesta.
    vence: Instant,
    /// Cuándo se bajaron, para el piso de refresco forzado.
    bajadas: Instant,
}

#[derive(Default)]
struct CacheDeLlaves {
    estado: RwLock<Option<Llaves>>,
}

static CACHE: OnceLock<Arc<CacheDeLlaves>> = OnceLock::new();

fn cache() -> Arc<CacheDeLlaves> {
    CACHE.get_or_init(|| Arc::new(CacheDeLlaves::default())).clone()
}

/// `max-age=N` del `Cache-Control`, acotado. Google manda algo como
/// `public, max-age=20489, must-revalidate, no-transform`.
fn ttl_del_header(cache_control: Option<&str>) -> Duration {
    let Some(cc) = cache_control else {
        return TTL_SI_NO_DICE;
    };
    let segundos = cc
        .split(',')
        .filter_map(|parte| {
            let parte = parte.trim();
            parte
                .strip_prefix("max-age")
                .and_then(|resto| resto.trim_start().strip_prefix('='))
                .and_then(|v| v.trim().parse::<u64>().ok())
        })
        .next();

    match segundos {
        Some(s) => Duration::from_secs(s).clamp(TTL_MINIMO, TTL_MAXIMO),
        None => TTL_SI_NO_DICE,
    }
}

impl CacheDeLlaves {
    /// Devuelve el JWKS, bajándolo si hace falta.
    ///
    /// `forzar` es el camino de rotación: Google firmó con una llave que
    /// todavía no habíamos visto. Se respeta [`MIN_ENTRE_REFRESCOS`] igual, así
    /// que forzar no es un pase libre para pegarle a Google.
    async fn obtener(&self, forzar: bool) -> Result<GoogleJwks, ApiError> {
        let ahora = Instant::now();

        {
            let guard = self.estado.read().unwrap_or_else(|e| e.into_inner());
            if let Some(l) = guard.as_ref() {
                let vigente = ahora < l.vence;
                let recien_bajadas = ahora.duration_since(l.bajadas) < MIN_ENTRE_REFRESCOS;
                if (vigente && !forzar) || (forzar && recien_bajadas) {
                    return Ok(l.jwks.clone());
                }
            }
        }

        let url = jwks_url();
        let resp = reqwest::Client::new()
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "google jwks: fetch failed");
                ApiError::service_unavailable()
            })?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "google jwks: non-2xx");
            return Err(ApiError::service_unavailable());
        }

        let ttl = ttl_del_header(
            resp.headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
        );

        let jwks: GoogleJwks = resp.json().await.map_err(|e| {
            tracing::warn!(error = %e, "google jwks: decode failed");
            ApiError::service_unavailable()
        })?;

        if jwks.vacio() {
            tracing::warn!("google jwks: sin llaves");
            return Err(ApiError::service_unavailable());
        }

        let ahora = Instant::now();
        let mut guard = self.estado.write().unwrap_or_else(|e| e.into_inner());
        *guard = Some(Llaves {
            jwks: jwks.clone(),
            vence: ahora + ttl,
            bajadas: ahora,
        });
        tracing::debug!(llaves = jwks.keys.len(), ttl_secs = ttl.as_secs(), "google jwks: cacheado");
        Ok(jwks)
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct UsuarioLigado {
    id: surrealdb::sql::Thing,
    tenant: surrealdb::sql::Thing,
    roles: Vec<String>,
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct VinculoRow {
    user: surrealdb::sql::Thing,
    tenant: surrealdb::sql::Thing,
}

#[derive(Debug, serde::Deserialize)]
struct TenantRow {
    id: surrealdb::sql::Thing,
}

async fn exchange(
    State(s): State<AppState>,
    Json(body): Json<domain::google_identity::GoogleSignInRequest>,
) -> Result<Json<domain::google_identity::GoogleSignInResponse>, ApiError> {
    // Sin client id configurado el carril entero no existe: 501, igual que
    // antes. No se toca la base ni se sale a la red.
    let Some(aud_esperado) = client_id() else {
        return Err(ApiError::not_implemented());
    };

    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;
    let cache = cache();
    let ahora = chrono::Utc::now().timestamp();

    // Verificar primero, preguntar después: nada de lo que viene en el cuerpo
    // vale hasta que la firma de Google cierre.
    let jwks = cache.obtener(false).await?;
    let identidad = match verificar_id_token(&body.id_token, &jwks, &aud_esperado, ahora) {
        Ok(id) => id,
        // Rotación de llaves: Google firmó con una que nuestro caché no tenía.
        // Se refresca una vez y se reintenta. Un caché eterno acá es lo que
        // deja a todo el mundo afuera un martes cualquiera.
        Err(ErrorVerificacionGoogle::LlaveDesconocida { kid }) => {
            tracing::info!(%kid, "google: kid desconocido, refrescando jwks");
            let frescas = cache.obtener(true).await?;
            verificar_id_token(&body.id_token, &frescas, &aud_esperado, ahora)
                .map_err(mapear_error_verificacion)?
        }
        Err(e) => return Err(mapear_error_verificacion(e)),
    };

    let tenant_pedido = body.tenant.as_deref().map(str::trim).filter(|t| !t.is_empty());

    // A partir de acá `body.id_token` no se vuelve a tocar. Lo que sigue usa
    // sólo claims ya verificados.
    let (usuario, es_nuevo) = resolver_usuario(db, &identidad, tenant_pedido).await?;

    let sub_usuario = usuario.id.to_string();
    let tenant_id = usuario.tenant.to_string();
    let token = auth::issue(&s.jwt, &sub_usuario, &tenant_id, usuario.roles.clone()).map_err(
        |e| {
            tracing::error!(error = %e, "google: issue jwt failed");
            ApiError::service_unavailable()
        },
    )?;

    // Misma secuencia que `routes::login`: re-verificar el propio token para
    // sacar el `exp` real y persistir la sesión.
    //
    // Está duplicado a propósito y no factorizado en un helper compartido:
    // `routes.rs` y el guard del secreto JWT en `lib.rs` los está moviendo el
    // carril del arranque ahora mismo, y un refactor de esos dos archivos desde
    // acá es un conflicto garantizado. Unificar es la primera tarea cuando ese
    // carril cierre.
    let claims = auth::verify(&s.jwt, &token).map_err(|e| {
        tracing::error!(error = %e, "google: re-verify own jwt failed");
        ApiError::service_unavailable()
    })?;
    let jti = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp, 0)
        .ok_or_else(ApiError::service_unavailable)?;

    if let Err(e) = db
        .query(
            "CREATE session SET user = $user, tenant = $tenant, jti = $jti, \
             expires_at = $expires_at",
        )
        .bind(("user", usuario.id.clone()))
        .bind(("tenant", usuario.tenant.clone()))
        .bind(("jti", jti))
        .bind(("expires_at", expires_at))
        .await
    {
        tracing::warn!(error = %e, "google: session persist failed (token still issued)");
    }

    if let Err(e) = db
        .query("UPDATE google_identity SET last_login_at = time::now() WHERE sub = $sub AND tenant = $tenant")
        .bind(("sub", identidad.sub.clone()))
        .bind(("tenant", usuario.tenant.clone()))
        .await
    {
        tracing::warn!(error = %e, "google: last_login_at update failed");
    }

    tracing::info!(sub = %identidad.sub, nuevo = es_nuevo, "google: sesión emitida");

    Ok(Json(domain::google_identity::GoogleSignInResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: s.jwt.ttl_seconds as i64,
        // Sólo para mostrar. La sesión ya está atada al `sub`, no a esto.
        email: identidad.email.clone().or(usuario.email.clone()),
        is_new_user: es_nuevo,
    }))
}

/// Un token que no cierra es un 401 y punto: no se le dice al cliente **cuál**
/// de los chequeos falló, porque eso es un oráculo gratis para quien esté
/// probando tokens. Los problemas nuestros (sin llaves, red caída) son 503 —
/// esos sí conviene distinguirlos, porque el cliente debería reintentar.
fn mapear_error_verificacion(e: ErrorVerificacionGoogle) -> ApiError {
    match e {
        ErrorVerificacionGoogle::SinLlaves => {
            tracing::warn!("google: jwks vacío al verificar");
            ApiError::service_unavailable()
        }
        otro => {
            // El motivo va al log del server, no a la respuesta.
            tracing::info!(motivo = %otro, "google: id_token rechazado");
            ApiError::bad_credentials()
        }
    }
}

/// De `sub` verificado a usuario nuestro.
///
/// Devuelve `(usuario, es_nuevo)`. `es_nuevo` es true sólo cuando este pedido
/// creó el vínculo, o sea la primera vez que esta cuenta de Google entra a este
/// negocio.
async fn resolver_usuario(
    db: &db::Db,
    identidad: &IdentidadGoogleVerificada,
    tenant_pedido: Option<&str>,
) -> Result<(UsuarioLigado, bool), ApiError> {
    // Si viene negocio explícito, se resuelve primero: acota la búsqueda y es
    // el único caso donde se puede crear un vínculo nuevo.
    let tenant_thing = match tenant_pedido {
        Some(slug) => {
            let mut q = db
                .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
                .bind(("slug", slug.to_string()))
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "google: tenant lookup failed");
                    ApiError::service_unavailable()
                })?;
            let fila: Option<TenantRow> = q.take(0).map_err(|e| {
                tracing::warn!(error = %e, "google: tenant decode failed");
                ApiError::service_unavailable()
            })?;
            // Negocio que no existe se contesta como credencial mala, no como
            // 404: enumerar slugs válidos desde afuera no aporta nada bueno.
            Some(fila.ok_or_else(ApiError::bad_credentials)?.id)
        }
        None => None,
    };

    let vinculos: Vec<VinculoRow> = match &tenant_thing {
        Some(t) => {
            let mut q = db
                .query("SELECT user, tenant FROM google_identity WHERE sub = $sub AND tenant = $tenant LIMIT 1")
                .bind(("sub", identidad.sub.clone()))
                .bind(("tenant", t.clone()))
                .await
                .map_err(fallo_de_lectura)?;
            q.take(0).map_err(fallo_de_lectura)?
        }
        None => {
            let mut q = db
                .query("SELECT user, tenant FROM google_identity WHERE sub = $sub")
                .bind(("sub", identidad.sub.clone()))
                .await
                .map_err(fallo_de_lectura)?;
            q.take(0).map_err(fallo_de_lectura)?
        }
    };

    match vinculos.len() {
        1 => {
            let v = &vinculos[0];
            let usuario = cargar_usuario(db, &v.user).await?;
            if usuario.active == Some(false) {
                return Err(ApiError::bad_credentials());
            }
            // Defensa en profundidad: el índice `gi_sub_tenant` ya lo garantiza,
            // pero si alguna vez no lo hiciera, emitir un JWT para un tenant que
            // no es el del usuario sería el peor bug posible de este archivo.
            if usuario.tenant != v.tenant {
                tracing::error!("google: vínculo con tenant inconsistente");
                return Err(ApiError::service_unavailable());
            }
            Ok((usuario, false))
        }

        // Pertenece a varios negocios y no dijo a cuál entra. No se elige por
        // él: entrar al negocio equivocado con la caja del otro a la vista es
        // peor que pedirle que toque una opción más.
        n if n > 1 => Err(ApiError::conflict(
            "Esta cuenta de Google está en más de un negocio. Elegí a cuál entrar.",
        )
        .with_details(serde_json::json!({ "negocios": vinculos.len() }))),

        // Todavía no hay vínculo: es la primera vez.
        _ => primer_vinculo(db, identidad, tenant_thing).await,
    }
}

/// El primer vínculo entre una cuenta de Google y un usuario del negocio.
///
/// Es el único momento en que el **email** decide algo, y por eso está cerrado
/// con llave por los cuatro lados:
///
/// 1. El negocio tiene que venir explícito. Sin eso habría que buscar el email
///    en todos los negocios del server, y el primer homónimo entra donde no va.
/// 2. `email_verified` tiene que ser true. Google emite tokens con emails sin
///    verificar; ligar por uno de esos es ligar por algo que el usuario escribió.
/// 3. Tiene que existir un usuario **activo** con ese email en ese negocio. No
///    se crea nadie: dar de alta un usuario porque alguien tocó un botón de
///    Google es abrir el negocio a cualquiera con una cuenta de Gmail.
/// 4. Ese usuario no puede tener ya otra cuenta de Google ligada (índice
///    `gi_user UNIQUE`).
///
/// Después de esta vez, el email no vuelve a decidir nada: se entra por `sub`.
async fn primer_vinculo(
    db: &db::Db,
    identidad: &IdentidadGoogleVerificada,
    tenant_thing: Option<surrealdb::sql::Thing>,
) -> Result<(UsuarioLigado, bool), ApiError> {
    let Some(tenant) = tenant_thing else {
        return Err(ApiError::conflict(
            "Decinos el nombre corto del negocio para entrar con Google la primera vez.",
        ));
    };

    if !identidad.email_verificado {
        tracing::info!(sub = %identidad.sub, "google: primer vínculo sin email verificado");
        return Err(ApiError::forbidden());
    }
    let Some(email) = identidad.email.as_deref() else {
        return Err(ApiError::forbidden());
    };

    let mut q = db
        .query(
            "SELECT id, tenant, roles, active, email FROM user \
             WHERE tenant = $tenant AND email = $email LIMIT 1",
        )
        .bind(("tenant", tenant.clone()))
        .bind(("email", email.to_string()))
        .await
        .map_err(fallo_de_lectura)?;
    let usuario: Option<UsuarioLigado> = q.take(0).map_err(fallo_de_lectura)?;

    // Nadie con ese correo en este negocio. No se crea: el alta del negocio y
    // de sus usuarios es otro camino (`/api/v1/setup`), con su propia
    // autorización. Acá sólo se liga a alguien que ya existe.
    let usuario = usuario.ok_or_else(|| {
        tracing::info!(sub = %identidad.sub, "google: sin usuario con ese email en el negocio");
        ApiError::forbidden()
    })?;

    if usuario.active == Some(false) {
        return Err(ApiError::bad_credentials());
    }

    // `.check()` no es decorativo: en SurrealDB el `.await` de un `query()`
    // devuelve `Ok` aunque la sentencia haya fallado — el error viaja **dentro**
    // de la `Response`. Sin `.check()`, el rechazo del índice único pasa
    // desapercibido y el handler sigue como si hubiera ligado la cuenta,
    // emitiendo un JWT por un vínculo que no existe.
    let creado = db
        .query(
            "CREATE google_identity SET sub = $sub, user = $user, tenant = $tenant, \
             email_al_ligar = $email, last_login_at = time::now()",
        )
        .bind(("sub", identidad.sub.clone()))
        .bind(("user", usuario.id.clone()))
        .bind(("tenant", tenant.clone()))
        .bind(("email", email.to_string()))
        .await
        // El error se aplana a texto acá mismo: `surrealdb::Error` pesa 144 bytes
        // y arrastrarlo dentro de un `Result` que sólo se mira para decidir
        // 409-o-no hace que clippy marque `result_large_err`, con razón.
        .map_err(|e| e.to_string())
        .and_then(|r| r.check().map_err(|e| e.to_string()));

    if let Err(e) = creado {
        // El índice único es la última palabra: si ese usuario ya tenía otra
        // cuenta de Google, o esta cuenta ya estaba en el negocio, la base lo
        // rechaza y acá se traduce a un 409 en vez de a un 500.
        tracing::warn!(error = %e, "google: no se pudo crear el vínculo");
        return Err(ApiError::conflict(
            "Esta cuenta ya está ligada a otro usuario de este negocio.",
        ));
    }

    tracing::info!(sub = %identidad.sub, "google: primer vínculo creado");
    Ok((usuario, true))
}

async fn cargar_usuario(
    db: &db::Db,
    user: &surrealdb::sql::Thing,
) -> Result<UsuarioLigado, ApiError> {
    let mut q = db
        .query("SELECT id, tenant, roles, active, email FROM user WHERE id = $id LIMIT 1")
        .bind(("id", user.clone()))
        .await
        .map_err(fallo_de_lectura)?;
    let fila: Option<UsuarioLigado> = q.take(0).map_err(fallo_de_lectura)?;
    // Vínculo apuntando a un usuario borrado: no es culpa de quien entra, pero
    // tampoco hay a quién dejar entrar.
    fila.ok_or_else(ApiError::bad_credentials)
}

fn fallo_de_lectura(e: impl std::fmt::Display) -> ApiError {
    tracing::warn!(error = %e, "google: lectura falló");
    ApiError::service_unavailable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respeta_el_max_age_de_google() {
        let ttl = ttl_del_header(Some("public, max-age=20489, must-revalidate, no-transform"));
        assert_eq!(ttl, Duration::from_secs(20489));
    }

    #[test]
    fn sin_cache_control_hay_un_ttl_por_defecto() {
        assert_eq!(ttl_del_header(None), TTL_SI_NO_DICE);
        assert_eq!(ttl_del_header(Some("public")), TTL_SI_NO_DICE);
    }

    /// Un `max-age` absurdo no nos deja con llaves muertas por días, y uno
    /// diminuto no nos hace pegarle a Google en cada login.
    #[test]
    fn el_ttl_queda_acotado_por_arriba_y_por_abajo() {
        assert_eq!(ttl_del_header(Some("max-age=999999999")), TTL_MAXIMO);
        assert_eq!(ttl_del_header(Some("max-age=1")), TTL_MINIMO);
    }

    #[test]
    fn max_age_basura_no_rompe() {
        assert_eq!(ttl_del_header(Some("max-age=abc")), TTL_SI_NO_DICE);
        assert_eq!(ttl_del_header(Some("max-age=")), TTL_SI_NO_DICE);
    }
}
