//! La invariante del respaldo: **ninguna ruta del server puede producir
//! plaintext** (ADR-0023).
//!
//! Esto no se verifica leyendo el código y confiando. Se verifica así:
//!
//! 1. El test cifra de verdad —AES-256-GCM— un texto que contiene un marcador
//!    imposible de confundir (`MARCADOR_EN_CLARO`).
//! 2. Sube el sobre por la API, igual que lo haría el teléfono.
//! 3. Recorre **todo** lo que el server puede emitir o guardar: la respuesta de
//!    subida, el listado, la bajada con sesión, el rescate sin sesión, los
//!    bytes en reposo en el store, y las filas de la tabla índice.
//! 4. Afirma que el marcador no aparece en ninguno.
//!
//! Si mañana alguien agrega un endpoint que descifra "para debuggear", o mete
//! el plaintext en un log estructurado que vuelve por la respuesta, este
//! archivo se pone rojo. Un comentario que dijera lo mismo, no.
//!
//! El paso 1 usa una llave derivada con SHA-256 y no con PBKDF2-210k: al test
//! no le importa **cuál** es el KDF —eso lo fija `CifrarSobre.kt` del lado del
//! teléfono— sino que la llave, sea cual sea, nunca cruza el cable.

mod e2e_common;

use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use e2e_common as h;
use sha2::{Digest, Sha256};

use api::user_backup::store::MemoryStore;
use api::user_backup::Runtime;

/// Aparece en el plaintext y en ningún otro lado. Si sale por una respuesta o
/// queda escrito en el store, el respaldo dejó de ser de la dueña.
const MARCADOR_EN_CLARO: &str = "MARCADOR-PLAINTEXT-QUE-NO-DEBE-SALIR-JAMAS-7f3a9c";

/// El negocio de verdad que va adentro del sobre: nombres de clientes, deudas,
/// precios. Es exactamente lo que no puede leer nadie más que la dueña.
fn negocio_en_claro() -> String {
    format!(
        r#"{{"marcador":"{MARCADOR_EN_CLARO}",
           "fiados":[{{"nombre":"Rosa Contreras","deuda":18500}},
                     {{"nombre":"Juan Pérez","deuda":7200}}],
           "boletas":[{{"total":12300}},{{"total":8900}}]}}"#
    )
}

/// Un sobre `RB1` real: `RB1\n<header JSON>\n<ciphertext||tag>`.
///
/// Devuelve `(bytes_del_sobre, llave)`. La llave **se queda en el test**, que
/// es exactamente el punto: en producción se queda en el teléfono.
fn cifrar_sobre(plaintext: &str) -> (Vec<u8>, [u8; 32]) {
    let llave: [u8; 32] = Sha256::digest(b"tarjeta-de-rescate-del-test").into();
    let salt = [0x11u8; 16];
    let nonce_bytes = [0x22u8; 12];

    let header = serde_json::json!({
        "v": 1,
        "kdf": "pbkdf2-hmac-sha256",
        "iter": 210_000,
        "salt": hex::encode(salt),
        "aead": "aes-256-gcm",
        "nonce": hex::encode(nonce_bytes),
    })
    .to_string();

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&llave));
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext.as_bytes(),
                aad: header.as_bytes(),
            },
        )
        .expect("cifrar");

    let mut sobre = Vec::new();
    sobre.extend_from_slice(b"RB1\n");
    sobre.extend_from_slice(header.as_bytes());
    sobre.push(b'\n');
    sobre.extend_from_slice(&ct);
    (sobre, llave)
}

fn cuerpo_de_subida(
    tenant_id: &str,
    sobre: &[u8],
    retrieval_hash_hex: Option<&str>,
) -> serde_json::Value {
    let sha = hex::encode(Sha256::digest(sobre));
    serde_json::json!({
        "meta": {
            "tenant_id": tenant_id,
            "format_version": 1,
            "ciphertext_sha256_hex": sha,
            "size_bytes": sobre.len(),
            "uploaded_at_unix": chrono::Utc::now().timestamp(),
            "label": "sábado de feria",
        },
        "ciphertext_base64": base64::engine::general_purpose::STANDARD.encode(sobre),
        "retrieval_hash_hex": retrieval_hash_hex,
    })
}

/// El marcador, en cualquier forma en que un descuido lo podría dejar salir:
/// crudo, en base64, o en hex.
fn contiene_el_marcador(bytes: &[u8]) -> Option<&'static str> {
    let texto = String::from_utf8_lossy(bytes);
    if texto.contains(MARCADOR_EN_CLARO) {
        return Some("en claro");
    }
    if texto.contains(&base64::engine::general_purpose::STANDARD.encode(MARCADOR_EN_CLARO)) {
        return Some("en base64");
    }
    if texto.contains(&hex::encode(MARCADOR_EN_CLARO)) {
        return Some("en hex");
    }
    None
}

fn revisar(que: &str, bytes: &[u8]) {
    if let Some(forma) = contiene_el_marcador(bytes) {
        panic!(
            "FUGA DE PLAINTEXT: el marcador apareció {forma} en «{que}». \
             El server no puede producir plaintext (ADR-0023)."
        );
    }
}

/// La prueba de retiro que mandaría el teléfono, y el hash que guarda el server.
fn prueba_y_hash(slug: &str) -> (String, String) {
    // El test no replica PBKDF2-210k: eso lo fija el lado Kotlin. Acá alcanza
    // con un secreto de 32 bytes estable — lo que se está fijando es el
    // contrato del server, que sólo ve `SHA-256(prueba)`.
    let prueba: [u8; 32] = Sha256::digest(format!("prueba-de-retiro:{slug}")).into();
    let hash = domain::user_backup::retrieval_hash_hex(&prueba);
    (hex::encode(prueba), hash)
}

struct Escenario {
    app: axum::Router,
    store: Arc<MemoryStore>,
    db: Arc<db::Db>,
    _tmp: tempfile::TempDir,
    token: String,
    tenant_id: String,
    slug: String,
}

async fn montar(slug: &str) -> Escenario {
    let tdb = h::spawn_db().await;
    let db = tdb.db.clone();
    let (tenant_id, user_id, roles) = h::seed_tenant_admin(&db, slug, "duena@feria.cl").await;
    let token = h::token_for(&user_id, &tenant_id, roles);

    let store = Arc::new(MemoryStore::new());
    let rt = Runtime::nuevo(
        store.clone(),
        pharma_core::config::UserBackupConfig {
            // Sin freno de frecuencia: varios tests suben dos veces seguidas y
            // lo que se está fijando acá es la confidencialidad, no la cuota.
            min_seconds_between_uploads: 0,
            ..Default::default()
        },
    );
    let app = api::build_router_with(h::state_free(db.clone()), rt);

    Escenario {
        app,
        store,
        db,
        _tmp: tdb._dir,
        token,
        tenant_id,
        slug: slug.to_string(),
    }
}

#[tokio::test]
async fn ninguna_ruta_del_server_puede_producir_plaintext() {
    let e = montar("feria-zk").await;
    let (sobre, llave) = cifrar_sobre(&negocio_en_claro());
    let (prueba_hex, hash) = prueba_y_hash(&e.slug);

    // --- subir ---
    let (st, subida) = h::req_json(
        &e.app,
        "POST",
        "/api/v1/user-backup",
        Some(&e.token),
        Some(cuerpo_de_subida(&e.tenant_id, &sobre, Some(&hash))),
        &[],
    )
    .await;
    assert_eq!(st, 200, "subida: {subida}");
    assert_eq!(subida["accepted"], true, "el bucket de memoria sí guarda");
    let backup_id = subida["backup_id"].as_str().expect("backup_id").to_string();
    revisar("respuesta de POST /user-backup", subida.to_string().as_bytes());

    // --- listar ---
    let (st, listado) = h::req_json(
        &e.app,
        "GET",
        "/api/v1/user-backup",
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 200);
    assert_eq!(listado.as_array().map(Vec::len), Some(1));
    revisar("respuesta de GET /user-backup", listado.to_string().as_bytes());
    // El listado tampoco puede devolver el hash de retiro: sería regalarle al
    // que lo lea el material para atacar la prueba offline.
    assert!(
        !listado.to_string().contains(&hash),
        "el listado filtró retrieval_hash"
    );

    // --- bajar con sesión ---
    let (st, bajada) = h::req_json(
        &e.app,
        "GET",
        &format!("/api/v1/user-backup/{backup_id}"),
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 200, "bajada: {bajada}");
    revisar("respuesta de GET /user-backup/{id}", bajada.to_string().as_bytes());

    // Y lo que devuelve tiene que ser el sobre, byte por byte: si el server lo
    // tocara —descomprimir, re-serializar, "normalizar"— el GCM no abriría.
    let vuelto = base64::engine::general_purpose::STANDARD
        .decode(bajada["ciphertext_base64"].as_str().expect("ciphertext"))
        .expect("base64");
    assert_eq!(vuelto, sobre, "el sobre volvió distinto de como entró");
    revisar("bytes del sobre devuelto", &vuelto);

    // --- rescate sin sesión ---
    let (st, rescate) = h::req_json(
        &e.app,
        "POST",
        "/api/v1/user-backup/rescue",
        None,
        Some(serde_json::json!({
            "tenant_slug": e.slug,
            "retrieval_proof_hex": prueba_hex,
        })),
        &[],
    )
    .await;
    assert_eq!(st, 200, "rescate: {rescate}");
    revisar("respuesta de POST /user-backup/rescue", rescate.to_string().as_bytes());

    // --- los bytes en reposo ---
    let guardado = e.store.contenido();
    assert_eq!(guardado.len(), 1, "un sobre subido, un objeto guardado");
    for (clave, bytes) in &guardado {
        revisar(&format!("objeto en reposo «{clave}»"), bytes);
        assert_eq!(bytes, &sobre, "el store guardó algo distinto del sobre");
    }

    // --- la tabla índice ---
    // El índice guarda punteros y metadatos. Si algún día alguien agrega ahí un
    // campo "para debug" con contenido del respaldo, salta acá.
    // El `*` es deliberado: un struct tipado sólo miraría los campos que hoy
    // conozco, y lo que hay que atrapar es el campo que alguien agregue mañana.
    // Los que no son JSON planos (`Thing`, `Datetime`) se castean a texto en la
    // consulta y se sacan del `*`, para que serde pueda leer el resto.
    let mut r = e
        .db
        .query(
            "SELECT <string>id AS id_txt, <string>tenant AS tenant_txt, \
             <string>uploaded_at AS fecha_txt, * \
             OMIT id, tenant, uploaded_at FROM user_backup",
        )
        .await
        .expect("consulta");
    let filas: Vec<serde_json::Value> = r.take(0).expect("filas");
    assert_eq!(filas.len(), 1);
    revisar(
        "filas de la tabla user_backup",
        filas[0].to_string().as_bytes(),
    );

    // Cierre: la llave sigue siendo del test (= del teléfono). El server nunca
    // la vio, y sin ella lo que guardó no significa nada.
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&llave));
    let header_fin = sobre.iter().skip(4).position(|b| *b == b'\n').unwrap() + 4;
    let header = &sobre[4..header_fin];
    let ct = &sobre[header_fin + 1..];
    let abierto = cipher
        .decrypt(
            Nonce::from_slice(&[0x22u8; 12]),
            Payload {
                msg: ct,
                aad: header,
            },
        )
        .expect("con la llave del teléfono, el sobre abre");
    assert!(
        String::from_utf8_lossy(&abierto).contains(MARCADOR_EN_CLARO),
        "el sobre que el server guardó sí contiene el negocio — cifrado"
    );
}

#[tokio::test]
async fn el_sobre_de_otro_negocio_no_se_baja_ni_se_enumera() {
    let e = montar("feria-a").await;
    let (sobre, _) = cifrar_sobre(&negocio_en_claro());
    let (_, hash) = prueba_y_hash(&e.slug);

    let (st, subida) = h::req_json(
        &e.app,
        "POST",
        "/api/v1/user-backup",
        Some(&e.token),
        Some(cuerpo_de_subida(&e.tenant_id, &sobre, Some(&hash))),
        &[],
    )
    .await;
    assert_eq!(st, 200, "{subida}");
    let backup_id = subida["backup_id"].as_str().unwrap().to_string();

    // Otro negocio en la misma base, con su propia sesión.
    let (tid_b, uid_b, roles_b) = h::seed_tenant_admin(&e.db, "feria-b", "otra@feria.cl").await;
    let token_b = h::token_for(&uid_b, &tid_b, roles_b);

    // Pide el id ajeno con un token válido propio.
    let (st, _) = h::req_json(
        &e.app,
        "GET",
        &format!("/api/v1/user-backup/{backup_id}"),
        Some(&token_b),
        None,
        &[],
    )
    .await;
    assert_eq!(
        st, 404,
        "el respaldo de otro negocio contesta 404, igual que uno que no existe"
    );

    // Y su listado está vacío: el índice es por tenant, no global.
    let (st, listado) = h::req_json(
        &e.app,
        "GET",
        "/api/v1/user-backup",
        Some(&token_b),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 200);
    assert_eq!(listado.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn el_rescate_contesta_404_uniforme_para_todo_lo_que_falla() {
    let e = montar("feria-404").await;
    let (sobre, _) = cifrar_sobre(&negocio_en_claro());
    let (prueba_hex, hash) = prueba_y_hash(&e.slug);

    h::req_json(
        &e.app,
        "POST",
        "/api/v1/user-backup",
        Some(&e.token),
        Some(cuerpo_de_subida(&e.tenant_id, &sobre, Some(&hash))),
        &[],
    )
    .await;

    // Tres formas distintas de fallar. Las tres tienen que ser indistinguibles:
    // si el slug inexistente contestara algo distinto del slug real con prueba
    // mala, el que prueba slugs ya sabría cuáles existen.
    let casos: [(&str, &str, &str); 3] = [
        ("slug que no existe", "no-existe-este", &prueba_hex),
        ("slug real, prueba mala", &e.slug, &hex::encode([0u8; 32])),
        ("prueba con forma inválida", &e.slug, "no-es-hex"),
    ];
    let mut respuestas = Vec::new();
    for (nombre, slug, prueba) in casos {
        let (st, body) = h::req_json(
            &e.app,
            "POST",
            "/api/v1/user-backup/rescue",
            None,
            Some(serde_json::json!({
                "tenant_slug": slug,
                "retrieval_proof_hex": prueba,
            })),
            &[],
        )
        .await;
        assert_eq!(st, 404, "«{nombre}» tiene que contestar 404, contestó {body}");
        respuestas.push(body.to_string());
    }
    assert!(
        respuestas.windows(2).all(|p| p[0] == p[1]),
        "los tres fallos tienen que devolver el mismo cuerpo, devolvieron {respuestas:?}"
    );

    // Y el que sí sabe, entra.
    let (st, ok) = h::req_json(
        &e.app,
        "POST",
        "/api/v1/user-backup/rescue",
        None,
        Some(serde_json::json!({
            "tenant_slug": e.slug,
            "retrieval_proof_hex": prueba_hex,
        })),
        &[],
    )
    .await;
    assert_eq!(st, 200, "{ok}");
}

#[tokio::test]
async fn sin_bucket_configurado_no_se_inventa_que_guardo() {
    // El default de un on-prem: valida, contesta honesto, no manda nada a la
    // nube de nadie. Es el comportamiento que había antes del bucket, y tiene
    // que seguir siendo el default.
    let tdb = h::spawn_db().await;
    let (tenant_id, user_id, roles) =
        h::seed_tenant_admin(&tdb.db, "feria-onprem", "duena@feria.cl").await;
    let token = h::token_for(&user_id, &tenant_id, roles);
    let app = api::build_router_with(h::state_free(tdb.db.clone()), Runtime::deshabilitado());

    let (sobre, _) = cifrar_sobre(&negocio_en_claro());
    let (st, body) = h::req_json(
        &app,
        "POST",
        "/api/v1/user-backup",
        Some(&token),
        Some(cuerpo_de_subida(&tenant_id, &sobre, None)),
        &[],
    )
    .await;

    assert_eq!(st, 200);
    assert_eq!(body["accepted"], false, "no guardó, y lo dice");
    assert!(body["backup_id"].is_null());
    assert!(
        body["reason"].as_str().is_some_and(|r| !r.is_empty()),
        "la razón tiene que estar escrita, no vacía: {body}"
    );

    // Y el rescate no existe cuando no hay dónde guardar: ofrecer una puerta de
    // rescate sobre un bucket que no está sería la misma mentira de antes.
    let (st, _) = h::req_json(
        &app,
        "POST",
        "/api/v1/user-backup/rescue",
        None,
        Some(serde_json::json!({
            "tenant_slug": "feria-onprem",
            "retrieval_proof_hex": hex::encode([7u8; 32]),
        })),
        &[],
    )
    .await;
    assert_eq!(st, 404);
}
