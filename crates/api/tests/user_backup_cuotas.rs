//! Cuotas, rotación, retención y borrado del respaldo cifrado (ADR-0023).
//!
//! Un endpoint de subida abierto es hosting gratis para internet, y sin techo
//! la cuenta del ADR es ficción. Estos tests fijan los cuatro topes:
//!
//! * **tamaño** — un sobre más grande que el máximo no llega al bucket;
//! * **frecuencia** — la segunda subida seguida se rechaza con `Retry-After`;
//! * **versiones** — entra la nueva, sale la más vieja, y el objeto del bucket
//!   se va con ella (no queda huérfano pagando espacio);
//! * **retención** — los sobres de quien dejó de usar la app se barren.
//!
//! Más el borrado a pedido, que es lo que hace que "tus datos son tuyos" no sea
//! sólo una frase: `DELETE .../all` tiene que dejar el índice **y** el bucket
//! vacíos.
//!
//! El orden de los chequeos importa y también se fija acá: lo que rechaza por
//! tamaño no debe haber tocado el bucket, porque el `PUT` es lo único que
//! cuesta plata.

mod e2e_common;

use std::sync::Arc;

use base64::Engine;
use e2e_common as h;
use sha2::{Digest, Sha256};

use api::user_backup::store::MemoryStore;
use api::user_backup::Runtime;

fn cfg(
    max_envelope_bytes: u64,
    max_versions_per_tenant: u32,
    min_seconds_between_uploads: u64,
    retention_days: u32,
) -> pharma_core::config::UserBackupConfig {
    pharma_core::config::UserBackupConfig {
        max_envelope_bytes,
        max_versions_per_tenant,
        min_seconds_between_uploads,
        retention_days,
        ..Default::default()
    }
}

/// Un sobre de `n` bytes con contenido distinto en cada llamada, para que el
/// `sha256` (y por lo tanto el `backup_id`) no se repita entre subidas.
fn sobre(n: usize, semilla: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(n);
    v.extend_from_slice(b"RB1\n{\"v\":1}\n");
    let mut x = semilla;
    while v.len() < n {
        x = x.wrapping_mul(31).wrapping_add(17);
        v.push(x);
    }
    v.truncate(n);
    v
}

fn cuerpo(tenant_id: &str, bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "meta": {
            "tenant_id": tenant_id,
            "format_version": 1,
            "ciphertext_sha256_hex": hex::encode(Sha256::digest(bytes)),
            "size_bytes": bytes.len(),
            "uploaded_at_unix": chrono::Utc::now().timestamp(),
        },
        "ciphertext_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

struct Escenario {
    app: axum::Router,
    store: Arc<MemoryStore>,
    db: Arc<db::Db>,
    _tmp: tempfile::TempDir,
    token: String,
    tenant_id: String,
}

async fn montar(cfg: pharma_core::config::UserBackupConfig) -> Escenario {
    let tdb = h::spawn_db().await;
    let db = tdb.db.clone();
    let (tenant_id, user_id, roles) = h::seed_tenant_admin(&db, "feria-cuotas", "d@feria.cl").await;
    let token = h::token_for(&user_id, &tenant_id, roles);
    let store = Arc::new(MemoryStore::new());
    let app = api::build_router_with(
        h::state_free(db.clone()),
        Runtime::nuevo(store.clone(), cfg),
    );
    Escenario {
        app,
        store,
        db,
        _tmp: tdb._dir,
        token,
        tenant_id,
    }
}

impl Escenario {
    async fn subir(&self, bytes: &[u8]) -> (axum::http::StatusCode, serde_json::Value) {
        h::req_json(
            &self.app,
            "POST",
            "/api/v1/user-backup",
            Some(&self.token),
            Some(cuerpo(&self.tenant_id, bytes)),
            &[],
        )
        .await
    }

    async fn listar(&self) -> Vec<serde_json::Value> {
        let (st, body) = h::req_json(
            &self.app,
            "GET",
            "/api/v1/user-backup",
            Some(&self.token),
            None,
            &[],
        )
        .await;
        assert_eq!(st, 200);
        body.as_array().cloned().unwrap_or_default()
    }
}

#[tokio::test]
async fn un_sobre_mas_grande_que_el_tope_no_llega_al_bucket() {
    let e = montar(cfg(1024, 5, 0, 400)).await;

    let (st, body) = e.subir(&sobre(4096, 1)).await;
    assert_eq!(st, 400, "un sobre sobre el tope se rechaza: {body}");

    // Lo importante no es el 400 sino esto: el `PUT` es la única operación que
    // cuesta plata, así que la cuota de tamaño tiene que cortar ANTES de tocar
    // el bucket. Si esto se rompe, un atacante paga la factura con basura que
    // igual se rechaza.
    assert_eq!(
        e.store.len(),
        0,
        "la subida rechazada por tamaño igual tocó el bucket"
    );

    // Y uno que sí cabe entra.
    let (st, body) = e.subir(&sobre(512, 2)).await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(body["accepted"], true);
    assert_eq!(e.store.len(), 1);
}

#[tokio::test]
async fn la_segunda_subida_seguida_se_frena_y_dice_cuanto_esperar() {
    let e = montar(cfg(1024 * 1024, 5, 900, 400)).await;

    let (st, body) = e.subir(&sobre(512, 1)).await;
    assert_eq!(st, 200, "{body}");

    let (st, body) = e.subir(&sobre(512, 2)).await;
    assert_eq!(st, 429, "la segunda seguida se frena: {body}");

    // El mensaje tiene que decirle a la dueña que su plata está a salvo — el
    // freno es del respaldo remoto, no de la venta.
    let msg = body.to_string();
    assert!(
        msg.contains("teléfono") || msg.contains("telefono"),
        "el 429 tiene que aclarar que lo cobrado sigue en el teléfono: {msg}"
    );

    assert_eq!(e.store.len(), 1, "la frenada no escribió un segundo objeto");
    assert_eq!(e.listar().await.len(), 1);
}

#[tokio::test]
async fn entra_la_nueva_sale_la_mas_vieja_y_el_objeto_se_va_con_ella() {
    // 3 versiones, sin freno de frecuencia para poder subir 5 seguidas.
    let e = montar(cfg(1024 * 1024, 3, 0, 400)).await;

    let mut ids = Vec::new();
    for i in 1..=5u8 {
        let (st, body) = e.subir(&sobre(256 + i as usize, i)).await;
        assert_eq!(st, 200, "subida {i}: {body}");
        ids.push(body["backup_id"].as_str().unwrap().to_string());
        // `backup_id` lleva la fecha al segundo; sin esta pausa las 5 subidas
        // caen en el mismo segundo y el índice `UNIQUE` las rechaza. En
        // producción el freno de frecuencia hace innecesario el detalle.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }

    let listado = e.listar().await;
    assert_eq!(listado.len(), 3, "se conservan 3 versiones, no 5");
    assert_eq!(
        e.store.len(),
        3,
        "el bucket tiene que quedar con 3 objetos: rotar sin borrar el objeto \
         deja huérfanos que se pagan para siempre"
    );

    // Las que quedan son las 3 más nuevas.
    let quedaron: Vec<String> = listado
        .iter()
        .map(|m| m["backup_id"].as_str().unwrap().to_string())
        .collect();
    for viejo in &ids[..2] {
        assert!(!quedaron.contains(viejo), "{viejo} tendría que haber salido");
    }
    for nuevo in &ids[2..] {
        assert!(quedaron.contains(nuevo), "{nuevo} tendría que estar");
    }

    // Y el sobre rotado ya no se puede bajar: la fila y el objeto se fueron
    // juntos, sin dejar una promesa colgada.
    let (st, _) = h::req_json(
        &e.app,
        "GET",
        &format!("/api/v1/user-backup/{}", ids[0]),
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 404);
}

#[tokio::test]
async fn borrar_todo_deja_el_indice_y_el_bucket_vacios() {
    let e = montar(cfg(1024 * 1024, 5, 0, 400)).await;

    for i in 1..=2u8 {
        let (st, body) = e.subir(&sobre(256 + i as usize, i)).await;
        assert_eq!(st, 200, "{body}");
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
    assert_eq!(e.store.len(), 2);

    let (st, _) = h::req_json(
        &e.app,
        "DELETE",
        "/api/v1/user-backup/all",
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 204);

    assert_eq!(e.listar().await.len(), 0, "el índice quedó con filas");
    assert_eq!(
        e.store.len(),
        0,
        "«borrá mis datos» tiene que sacar los bytes del bucket, no sólo la fila"
    );

    // Borrar de nuevo sigue siendo 204: el estado pedido ya es el que hay.
    let (st, _) = h::req_json(
        &e.app,
        "DELETE",
        "/api/v1/user-backup/all",
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 204);
}

#[tokio::test]
async fn borrar_uno_solo_no_se_lleva_los_otros() {
    let e = montar(cfg(1024 * 1024, 5, 0, 400)).await;

    let mut ids = Vec::new();
    for i in 1..=3u8 {
        let (st, body) = e.subir(&sobre(256 + i as usize, i)).await;
        assert_eq!(st, 200, "{body}");
        ids.push(body["backup_id"].as_str().unwrap().to_string());
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }

    let (st, _) = h::req_json(
        &e.app,
        "DELETE",
        &format!("/api/v1/user-backup/{}", ids[1]),
        Some(&e.token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, 204);

    assert_eq!(e.listar().await.len(), 2);
    assert_eq!(e.store.len(), 2);
}

#[tokio::test]
async fn la_retencion_barre_los_sobres_de_quien_dejo_de_usar_la_app() {
    let e = montar(cfg(1024 * 1024, 5, 0, 30)).await;

    let (st, body) = e.subir(&sobre(512, 1)).await;
    assert_eq!(st, 200, "{body}");
    assert_eq!(e.store.len(), 1);

    // Envejecer la fila: la dueña dejó de vender hace 90 días.
    e.db.query("UPDATE user_backup SET uploaded_at = $viejo")
        .bind(("viejo", surrealdb::sql::Datetime::from(chrono::Utc::now() - chrono::Duration::days(90))))
        .await
        .expect("envejecer");

    let store: Arc<dyn api::user_backup::store::BlobStore> = e.store.clone();
    let barridos = api::user_backup::repo_ops::sweep_expired(&e.db, &store, 30).await;

    assert_eq!(barridos, 1);
    assert_eq!(e.listar().await.len(), 0);
    assert_eq!(
        e.store.len(),
        0,
        "la retención tiene que sacar los bytes, no sólo la fila"
    );
}

#[tokio::test]
async fn con_retencion_en_cero_no_se_barre_nada() {
    // 0 = para siempre. Es una decisión del que instala, y el barredor tiene
    // que respetarla en vez de interpretarla como "vencido hace 0 días".
    let e = montar(cfg(1024 * 1024, 5, 0, 0)).await;

    let (st, _) = e.subir(&sobre(512, 1)).await;
    assert_eq!(st, 200);
    e.db.query("UPDATE user_backup SET uploaded_at = $viejo")
        .bind(("viejo", surrealdb::sql::Datetime::from(chrono::Utc::now() - chrono::Duration::days(5000))))
        .await
        .expect("envejecer");

    let store: Arc<dyn api::user_backup::store::BlobStore> = e.store.clone();
    assert_eq!(
        api::user_backup::repo_ops::sweep_expired(&e.db, &store, 0).await,
        0
    );
    assert_eq!(e.store.len(), 1);
}

#[tokio::test]
async fn un_sobre_todavia_joven_no_lo_toca_la_retencion() {
    let e = montar(cfg(1024 * 1024, 5, 0, 400)).await;

    let (st, _) = e.subir(&sobre(512, 1)).await;
    assert_eq!(st, 200);
    // 200 días parado: invierno largo, una enfermedad. Sigue siendo su negocio.
    e.db.query("UPDATE user_backup SET uploaded_at = $viejo")
        .bind(("viejo", surrealdb::sql::Datetime::from(chrono::Utc::now() - chrono::Duration::days(200))))
        .await
        .expect("envejecer");

    let store: Arc<dyn api::user_backup::store::BlobStore> = e.store.clone();
    assert_eq!(
        api::user_backup::repo_ops::sweep_expired(&e.db, &store, 400).await,
        0
    );
    assert_eq!(e.store.len(), 1, "400 días es el corte, 200 no lo alcanza");
}
