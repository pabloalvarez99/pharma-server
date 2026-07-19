//! Subtask 9.1.c — folio assignment atómico bajo concurrencia.
//!
//! Usa SurrealDB kv-mem para evitar locks de SurrealKV file-backed. Seed
//! CAF con rango [1..=20], spawn 20 tasks paralelas, todos deben recibir
//! folios distintos y consecutivos.

use dte::caf::assign_next;
use dte::DteTipo;
use std::collections::HashSet;
use std::sync::Arc;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

async fn setup_db() -> (Arc<Surreal<surrealdb::engine::local::Db>>, Thing) {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    // SCHEMALESS para simplificar; sólo necesitamos los campos que assign_next lee.
    db.query(
        "CREATE tenant:t1 SET name = 'farmacia test'; \
         CREATE caf:c1 SET \
            tenant = tenant:t1, \
            tipo_dte = 39, \
            folio_desde = 1, \
            folio_hasta = 20, \
            next_folio = 1, \
            fecha_autorizacion = time::now(), \
            rut_emisor = '76123456-7', \
            xml = '', \
            activo = true;",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    let tenant = Thing::from(("tenant", "t1"));
    (Arc::new(db), tenant)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn folios_unicos_bajo_concurrencia() {
    let (db, tenant) = setup_db().await;
    let mut handles = Vec::new();
    for _ in 0..20 {
        let db = db.clone();
        let tenant = tenant.clone();
        handles.push(tokio::spawn(async move {
            assign_next(&db, &tenant, DteTipo::BoletaElectronica)
                .await
                .map(|(_, folio)| folio)
        }));
    }
    let mut folios = Vec::new();
    for h in handles {
        folios.push(h.await.unwrap().expect("assign_next ok"));
    }
    let set: HashSet<_> = folios.iter().copied().collect();
    assert_eq!(set.len(), 20, "folios deben ser únicos: {folios:?}");
    let mut sorted = folios.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        (1..=20).collect::<Vec<_>>(),
        "folios contiguos 1..=20"
    );
}

#[tokio::test]
async fn caf_agotado_devuelve_folio_exhausted() {
    let (db, tenant) = setup_db().await;
    for _ in 0..20 {
        assign_next(&db, &tenant, DteTipo::BoletaElectronica)
            .await
            .unwrap();
    }
    let err = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
        .await
        .unwrap_err();
    assert!(
        matches!(err, dte::DteError::FolioExhausted { .. }),
        "esperado FolioExhausted, obtuve: {err:?}"
    );
}

#[tokio::test]
async fn sin_caf_activo_devuelve_folio_exhausted() {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    db.query("CREATE tenant:t1 SET name = 'x';")
        .await
        .unwrap()
        .check()
        .unwrap();
    let tenant = Thing::from(("tenant", "t1"));
    let err = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
        .await
        .unwrap_err();
    assert!(matches!(err, dte::DteError::FolioExhausted { .. }));
}
