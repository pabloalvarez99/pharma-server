//! Entrar con Google, de punta a punta y **sin red** (ADR-0022).
//!
//! El `id_token` se firma acá con un par RSA generado en memoria, y el JWKS se
//! sirve desde un servidor local que arranca este mismo test. Nada sale a
//! internet y nada depende de que existan credenciales en la consola de Google:
//! la verificación es una función de (token, llaves, `aud`), y así se prueba
//! entera. Cuando lleguen los valores reales sólo cambia la config.
//!
//! **Regla 3:** no hay ningún client id ni clave de verdad en este archivo. El
//! par RSA se genera en cada corrida — una clave privada "de test" commiteada
//! sigue siendo una clave privada commiteada.
//!
//! Este binario **sí** configura `PHARMA_GOOGLE_CLIENT_ID`. El caso contrario
//! —build sin client id— vive en `google_sin_client_id.rs`, que es otro binario
//! y por lo tanto otro proceso: dos tests que pelean por la misma variable de
//! entorno en el mismo proceso es una carrera, no una prueba.

mod e2e_common;

use std::sync::{Arc, OnceLock};

use axum::http::StatusCode;
use base64::Engine as _;
use e2e_common::{req_json, seed_tenant_admin, spawn_db, state_free};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use serde_json::json;

const AUD_NUESTRO: &str = "client-id-de-test.apps.googleusercontent.com";
const SUB_ROSA: &str = "1047290000000000000-estable";

// ---------------------------------------------------------------------------
// El Google de mentira
// ---------------------------------------------------------------------------

struct ParDeLlaves {
    pem: String,
    n: String,
    e: String,
}

fn llaves() -> &'static ParDeLlaves {
    static LLAVES: OnceLock<ParDeLlaves> = OnceLock::new();
    LLAVES.get_or_init(|| {
        let mut rng = rand::thread_rng();
        let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("generar rsa");
        let pub_key = priv_key.to_public_key();
        ParDeLlaves {
            pem: priv_key
                .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
                .expect("pem")
                .to_string(),
            n: b64url(&pub_key.n().to_bytes_be()),
            e: b64url(&pub_key.e().to_bytes_be()),
        }
    })
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn jwks_json() -> serde_json::Value {
    json!({
        "keys": [{
            "kid": "llave-1",
            "kty": "RSA",
            "alg": "RS256",
            "use": "sig",
            "n": llaves().n,
            "e": llaves().e,
        }]
    })
}

/// Levanta el JWKS en un puerto libre y apunta el server ahí.
///
/// Se hace una sola vez por proceso: el handler cachea las llaves en un
/// `OnceLock` global, así que dos servidores distintos en el mismo binario
/// serían un caché apuntando al primero y tests que dependen del orden.
///
/// ## Por qué tiene hilo y runtime propios
///
/// `#[tokio::test]` arma un runtime por test y lo **tira** al terminar. Un
/// `tokio::spawn` acá ataba el servidor al runtime del primer test que lo
/// arrancara: ese test terminaba, el runtime moría, y el servidor con él. Los
/// que venían después se encontraban la conexión rechazada.
///
/// No se veía siempre, que es lo peor: el caché de llaves del handler es
/// global y dura 24 h, así que cualquier test que alcanzara a bajarlas antes
/// de la muerte tapaba el problema para el resto. Sólo fallaban los que
/// pedían justo en la ventana entre el fin del dueño y el caché lleno — 503
/// intermitentes, distintos en cada corrida, y más probables cuanto más tests
/// tiene el binario. Un test que falla según quién terminó primero no prueba
/// nada.
///
/// `get_or_init` además arregla la carrera del arranque: la versión con
/// `get()` + `set()` dejaba que dos tests entraran a la vez y levantaran dos
/// servidores, cada uno pisándole la variable de entorno al otro.
fn google_de_mentira() {
    static ARRANCADO: OnceLock<()> = OnceLock::new();
    ARRANCADO.get_or_init(|| {
        let (avisar_puerto, esperar_puerto) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime del google de mentira");

            rt.block_on(async move {
                let app = axum::Router::new().route(
                    "/certs",
                    axum::routing::get(|| async {
                        (
                            // El mismo `Cache-Control` que manda Google de
                            // verdad: el handler lo parsea para decidir cuánto
                            // vive el caché.
                            [(
                                axum::http::header::CACHE_CONTROL,
                                "public, max-age=20489, must-revalidate, no-transform",
                            )],
                            axum::Json(jwks_json()),
                        )
                    }),
                );

                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("bind");
                let puerto = listener.local_addr().expect("addr").port();
                avisar_puerto.send(puerto).expect("avisar el puerto");

                axum::serve(listener, app).await.ok();
            });
        });

        // Bloquea hasta que el socket está escuchando. Devolver antes dejaría
        // al primer test pegándole a un puerto que todavía no existe.
        let puerto = esperar_puerto
            .recv()
            .expect("el google de mentira no llegó a escuchar");

        std::env::set_var("PHARMA_GOOGLE_CLIENT_ID", AUD_NUESTRO);
        std::env::set_var(
            "PHARMA_GOOGLE_JWKS_URL",
            format!("http://127.0.0.1:{puerto}/certs"),
        );
    });
}

/// Firma un `id_token` como lo haría Google.
fn firmar(kid: &str, claims: serde_json::Value) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(kid.to_string());
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(llaves().pem.as_bytes()).expect("key");
    jsonwebtoken::encode(&header, &claims, &key).expect("firmar")
}

fn claims(email: &str, verificado: bool) -> serde_json::Value {
    json!({
        "sub": SUB_ROSA,
        "iss": "https://accounts.google.com",
        "aud": AUD_NUESTRO,
        "exp": chrono::Utc::now().timestamp() + 3600,
        "iat": chrono::Utc::now().timestamp(),
        "email": email,
        "email_verified": verificado,
        "name": "Rosa del puesto",
    })
}

fn app(db: &Arc<db::Db>) -> axum::Router {
    api::build_router(state_free(db.clone()))
}

const RUTA: &str = "/api/v1/auth/google";

// ---------------------------------------------------------------------------
// El camino feliz
// ---------------------------------------------------------------------------

/// La primera vez: no hay vínculo, se liga por el email verificado contra el
/// usuario que ya existe en el negocio, y sale **nuestro** JWT — el mismo que
/// emite el login con clave, para que Android no necesite otra rama.
#[tokio::test]
async fn primera_vez_liga_la_cuenta_y_devuelve_nuestro_jwt() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let token = firmar("llave-1", claims("rosa@gmail.com", true));
    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": token, "tenant": "puesto-rosa" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_new_user"], json!(true));
    assert_eq!(body["token_type"], json!("Bearer"));
    assert_eq!(body["email"], json!("rosa@gmail.com"));

    // El JWT que sale es nuestro y lo verifica nuestra propia clave: es lo que
    // hace que `SessionRepository` reuse la ruta de activación sin ramas.
    let nuestro = body["token"].as_str().expect("token");
    let claims = auth::verify(&e2e_common::jwt_cfg(), nuestro).expect("nuestro jwt verifica");
    assert!(claims.roles.contains(&"admin".to_string()));

    // Y ese token abre el resto de la API, que es la prueba de que sirve.
    let (status, _) = req_json(&app(&tdb.db), "GET", "/api/v1/me", Some(nuestro), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
}

/// La segunda vez ya no liga nada: entra por el `sub`, que es el punto.
#[tokio::test]
async fn la_segunda_vez_ya_no_es_usuario_nuevo() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let cuerpo = json!({
        "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
        "tenant": "puesto-rosa",
    });
    let (s1, _) = req_json(&app(&tdb.db), "POST", RUTA, None, Some(cuerpo.clone()), &[]).await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, b2) = req_json(&app(&tdb.db), "POST", RUTA, None, Some(cuerpo), &[]).await;
    assert_eq!(s2, StatusCode::OK, "{b2}");
    assert_eq!(b2["is_new_user"], json!(false), "el vínculo ya existía");
}

/// **La razón de que la tabla exista.** Rosa liga su cuenta; después su correo
/// cambia de dueño —dejó la empresa, el dominio lo reasignó— y Google empieza a
/// mandar el mismo `sub` con otro email. Tiene que seguir entrando a su negocio,
/// porque la identidad es el `sub`.
///
/// El espejo de esto es el bug que la tabla evita: si se entrara por email, el
/// **nuevo** dueño de esa casilla entraría al negocio de Rosa.
#[tokio::test]
async fn el_sub_manda_aunque_cambie_el_email() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (s1, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
            "tenant": "puesto-rosa",
        })),
        &[],
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    // Mismo `sub`, correo nuevo, y ni siquiera dice a qué negocio entra.
    let (s2, b2) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("otro-correo@gmail.com", true)) })),
        &[],
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "{b2}");
    assert_eq!(b2["is_new_user"], json!(false));
}

// ---------------------------------------------------------------------------
// Lo que no entra
// ---------------------------------------------------------------------------

/// El chequeo que la gente olvida. El token está impecablemente firmado por
/// Google — sólo que para otra aplicación.
#[tokio::test]
async fn un_token_para_otra_app_no_entra() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let mut c = claims("rosa@gmail.com", true);
    c["aud"] = json!("la-app-del-atacante.apps.googleusercontent.com");

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", c), "tenant": "puesto-rosa" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn un_token_firmado_por_cualquier_otro_no_entra() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    // Otra llave RSA, mismo `kid` que el JWKS publica.
    let otra = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).expect("rsa");
    let pem = otra
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .expect("pem")
        .to_string();
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("llave-1".into());
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).expect("key");
    let falso = jsonwebtoken::encode(&header, &claims("rosa@gmail.com", true), &key).expect("f");

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": falso, "tenant": "puesto-rosa" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn un_token_vencido_no_entra() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let mut c = claims("rosa@gmail.com", true);
    c["exp"] = json!(chrono::Utc::now().timestamp() - 7200);

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", c), "tenant": "puesto-rosa" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Google emite tokens con emails sin verificar. Ligar por uno de esos es ligar
/// por un dato que escribió el usuario, no por uno que Google comprobó.
#[tokio::test]
async fn sin_email_verificado_no_se_liga() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("rosa@gmail.com", false)),
            "tenant": "puesto-rosa",
        })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Una cuenta de Google que no corresponde a ningún usuario del negocio no crea
/// uno. Si lo creara, cualquiera con un Gmail entraría a cualquier negocio con
/// sólo saber su nombre corto.
#[tokio::test]
async fn una_cuenta_ajena_no_se_da_de_alta_sola() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("cualquiera@gmail.com", true)),
            "tenant": "puesto-rosa",
        })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Y no quedó nadie nuevo en la base.
    let mut q = tdb
        .db
        .query("SELECT count() FROM user GROUP ALL")
        .await
        .expect("contar");
    #[derive(serde::Deserialize)]
    struct Conteo {
        count: i64,
    }
    let fila: Option<Conteo> = q.take(0).expect("decode");
    assert_eq!(fila.expect("hay fila").count, 1, "no se creó ningún usuario");
}

/// Un negocio que no existe se contesta como credencial mala, no como 404:
/// enumerar slugs válidos desde afuera no le sirve a nadie que tenga derecho a
/// entrar.
#[tokio::test]
async fn negocio_inexistente_no_confirma_que_no_existe() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
            "tenant": "no-existe-este-negocio",
        })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Primera vez sin decir a qué negocio entra: no se adivina. El 409 es la señal
/// para que la app pregunte.
#[tokio::test]
async fn la_primera_vez_hay_que_decir_el_negocio() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)) })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

/// La pregunta de producto, con la respuesta que da hoy el server: la misma
/// persona en dos negocios y sin decir a cuál entra **no** se resuelve sola.
/// Entrar al negocio equivocado con la caja del otro a la vista es peor que
/// tocar una opción más.
#[tokio::test]
async fn dos_negocios_sin_elegir_pide_elegir() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;
    seed_tenant_admin(&tdb.db, "almacen-juan", "rosa@gmail.com").await;

    for negocio in ["puesto-rosa", "almacen-juan"] {
        let (s, b) = req_json(
            &app(&tdb.db),
            "POST",
            RUTA,
            None,
            Some(json!({
                "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
                "tenant": negocio,
            })),
            &[],
        )
        .await;
        assert_eq!(s, StatusCode::OK, "ligar {negocio}: {b}");
    }

    // Ahora, sin decir cuál.
    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)) })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // Y diciendo cuál, entra a ese y no al otro.
    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
            "tenant": "almacen-juan",
        })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let claims_nuestros =
        auth::verify(&e2e_common::jwt_cfg(), body["token"].as_str().expect("token")).expect("jwt");

    let mut q = tdb
        .db
        .query("SELECT id FROM tenant WHERE slug = 'almacen-juan' LIMIT 1")
        .await
        .expect("q");
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let juan: Option<Row> = q.take(0).expect("decode");
    assert_eq!(
        claims_nuestros.tenant_id,
        juan.expect("tenant").id.to_string(),
        "la sesión tiene que ser del negocio que pidió",
    );
}

/// Un usuario no puede terminar con dos cuentas de Google ligadas.
///
/// Google no emite dos `sub` para el mismo correo verificado, así que en la
/// vida real esto no llega — pero el test lo puede forjar, porque firma sus
/// propios tokens. Y es exactamente para lo que está el índice `gi_user UNIQUE`:
/// dos cuentas ligadas al mismo usuario significan que quien controle
/// **cualquiera** de las dos entra, o sea el doble de superficie para robar la
/// cuenta sin que nadie lo haya decidido.
///
/// Prueba además que el rechazo del índice se **ve**: sin `.check()`, SurrealDB
/// devuelve `Ok` con el error adentro de la respuesta y el handler seguiría de
/// largo emitiendo un JWT por un vínculo que no se creó.
#[tokio::test]
async fn un_usuario_no_puede_tener_dos_cuentas_de_google() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    let (s1, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
            "tenant": "puesto-rosa",
        })),
        &[],
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    // Otro `sub`, mismo correo verificado.
    let mut otro = claims("rosa@gmail.com", true);
    otro["sub"] = json!("otro-sub-de-google-000000");

    let (s2, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({
            "id_token": firmar("llave-1", otro),
            "tenant": "puesto-rosa",
        })),
        &[],
    )
    .await;
    assert_eq!(s2, StatusCode::CONFLICT, "el índice único tiene que frenarlo");

    #[derive(serde::Deserialize)]
    struct Conteo {
        count: i64,
    }
    let mut q = tdb
        .db
        .query("SELECT count() FROM google_identity GROUP ALL")
        .await
        .expect("contar");
    let fila: Option<Conteo> = q.take(0).expect("decode");
    assert_eq!(fila.expect("hay fila").count, 1, "quedó un solo vínculo");
}

/// Basura no es token, y no revienta el server.
#[tokio::test]
async fn basura_no_entra() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;

    for basura in ["", "no-soy-un-jwt", "a.b.c"] {
        let (status, _) = req_json(
            &app(&tdb.db),
            "POST",
            RUTA,
            None,
            Some(json!({ "id_token": basura, "tenant": "puesto-rosa" })),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "basura: {basura:?}");
    }
}

// ---------------------------------------------------------------------------
// Crear un negocio nuevo — la otra puerta
// ---------------------------------------------------------------------------

const RUTA_ALTA: &str = "/api/v1/auth/google/negocio";

#[derive(Debug, serde::Deserialize)]
struct Conteo {
    count: i64,
}

async fn contar(db: &Arc<db::Db>, tabla: &str) -> i64 {
    let mut q = db
        .query(format!("SELECT count() FROM {tabla} GROUP ALL"))
        .await
        .expect("contar");
    let fila: Option<Conteo> = q.take(0).expect("decode");
    fila.map(|f| f.count).unwrap_or(0)
}

/// El camino que hoy no existe: alguien con una cuenta de Google y ningún
/// negocio se crea el suyo. Tenant, usuario y vínculo, y queda adentro.
///
/// Es la contracara del 403 de `/auth/google`: entrar a un negocio ajeno está
/// cerrado, arrancar uno vacío no le saca nada a nadie.
#[tokio::test]
async fn con_google_se_crea_un_negocio_nuevo_y_queda_adentro() {
    google_de_mentira();
    let tdb = spawn_db().await;

    let token = firmar("llave-1", claims("rosa@gmail.com", true));
    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({
            "id_token": token,
            "business_name": "Verdulería Rosa",
            "vertical": "feria",
        })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["token_type"], json!("Bearer"));
    assert_eq!(body["email"], json!("rosa@gmail.com"));
    // El slug lo decide el server; el cliente no lo puede adivinar y por eso
    // vuelve en la respuesta.
    assert_eq!(body["tenant_slug"], json!("verduleria-rosa"));

    let nuestro = body["token"].as_str().expect("token");
    let (status, _) = req_json(&app(&tdb.db), "GET", "/api/v1/me", Some(nuestro), None, &[]).await;
    assert_eq!(status, StatusCode::OK, "el token del alta no abre la API");

    assert_eq!(contar(&tdb.db, "tenant").await, 1);
    assert_eq!(contar(&tdb.db, "user").await, 1);
    assert_eq!(contar(&tdb.db, "google_identity").await, 1);
}

/// Lo que hace que el alta sirva: la segunda vez entra por la puerta de
/// siempre, sin decir el negocio, porque el vínculo quedó hecho.
#[tokio::test]
async fn despues_del_alta_entra_por_la_puerta_de_siempre() {
    google_de_mentira();
    let tdb = spawn_db().await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
                     "business_name": "Verdulería Rosa" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)) })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_new_user"], json!(false), "el vínculo ya existía");
}

/// La cuenta nace **sin** clave usable. No es clave vacía ni una fija: se
/// hashea material aleatorio que no se guarda, así que ni nosotros podemos
/// entrar por ahí.
#[tokio::test]
async fn la_cuenta_creada_con_google_no_entra_por_clave() {
    google_de_mentira();
    let tdb = spawn_db().await;

    let (status, body) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
                     "business_name": "Verdulería Rosa" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let slug = body["tenant_slug"].as_str().expect("slug").to_string();

    for intento in ["", "password", "rosa@gmail.com", "verduleria-rosa"] {
        let (status, _) = req_json(
            &app(&tdb.db),
            "POST",
            "/api/v1/login",
            None,
            Some(json!({ "tenant": slug, "email": "rosa@gmail.com", "password": intento })),
            &[],
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "entró por clave con {intento:?}"
        );
    }
}

/// Nombre corto repetido: 409 con texto, no un 500 del índice único ni —peor—
/// meterla en el negocio que ya existe.
#[tokio::test]
async fn el_nombre_corto_repetido_no_toca_el_negocio_que_ya_esta() {
    google_de_mentira();
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "verduleria-rosa", "otra@gmail.com").await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
                     "business_name": "Verdulería Rosa" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(contar(&tdb.db, "tenant").await, 1, "se creó un tenant de más");
    assert_eq!(contar(&tdb.db, "user").await, 1, "se metió en el ajeno");
    assert_eq!(contar(&tdb.db, "google_identity").await, 0);
}

/// Sin nombre no hay negocio. El server no lo deriva del correo: "gmail.com"
/// no es el nombre de nadie.
#[tokio::test]
async fn sin_nombre_no_se_crea_negocio() {
    google_de_mentira();
    let tdb = spawn_db().await;

    for nombre in ["", "   "] {
        let (status, _) = req_json(
            &app(&tdb.db),
            "POST",
            RUTA_ALTA,
            None,
            Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", true)),
                         "business_name": nombre })),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "nombre {nombre:?}");
    }
    assert_eq!(contar(&tdb.db, "tenant").await, 0);
}

/// Email sin verificar: Google deja escribir cualquier cosa ahí en cuentas que
/// nunca confirmaron la casilla. No alcanza para fundar un negocio.
#[tokio::test]
async fn sin_email_verificado_no_se_crea_negocio() {
    google_de_mentira();
    let tdb = spawn_db().await;

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", claims("rosa@gmail.com", false)),
                     "business_name": "Verdulería Rosa" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(contar(&tdb.db, "tenant").await, 0);
}

/// La puerta nueva se verifica igual que la vieja: un token emitido para otra
/// app de Google no funda nada.
#[tokio::test]
async fn un_token_para_otra_app_no_crea_negocio() {
    google_de_mentira();
    let tdb = spawn_db().await;

    let mut ajenos = claims("rosa@gmail.com", true);
    ajenos["aud"] = json!("otra-app.apps.googleusercontent.com");

    let (status, _) = req_json(
        &app(&tdb.db),
        "POST",
        RUTA_ALTA,
        None,
        Some(json!({ "id_token": firmar("llave-1", ajenos),
                     "business_name": "Verdulería Rosa" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(contar(&tdb.db, "tenant").await, 0);
}
