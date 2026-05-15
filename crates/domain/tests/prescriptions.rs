//! Prescriptions + pharmacist-shift integration tests on `kv-mem`.

use chrono::Utc;
use domain::prescriptions::{model::*, service};
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations apply");
    let mut r = db
        .query("CREATE tenant SET name = 'Farmacia Test', slug = 'test' RETURN id")
        .await
        .unwrap();
    let tenant: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    let mut r = db
        .query(
            "CREATE user SET tenant = $t, email = 'qf@test.cl', password = 'x', \
             roles = ['pharmacist'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let user: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    (db, tenant, user)
}

fn np(name: &str, rut: &str) -> NewPrescription {
    NewPrescription {
        product: None,
        customer: None,
        patient_name: name.into(),
        patient_rut: rut.into(),
        doctor_name: None,
        doctor_rut: None,
        controlled: false,
        folio: None,
        dispensed_at: None,
    }
}

#[tokio::test]
async fn create_and_read_prescription() {
    let (db, t, _u) = setup().await;
    let p = service::create_prescription(&db, &t, np("Ana", "1-1"))
        .await
        .unwrap();
    assert_eq!(p.patient_name, "Ana");
    let fetched = service::get_prescription(&db, &t, &p.id).await.unwrap();
    assert_eq!(fetched.id, p.id);
}

#[tokio::test]
async fn controlled_requires_doctor() {
    let (db, t, _u) = setup().await;
    let mut input = np("Ana", "1-1");
    input.controlled = true;
    let err = service::create_prescription(&db, &t, input)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    let mut ok = np("Ana", "1-1");
    ok.controlled = true;
    ok.doctor_name = Some("Dr. House".into());
    ok.doctor_rut = Some("9-9".into());
    let p = service::create_prescription(&db, &t, ok).await.unwrap();
    assert!(p.controlled);

    let list = service::list_controlled(&db, &t, PrescriptionFilters::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn tenant_isolation() {
    let (db, t1, _u) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    service::create_prescription(&db, &t1, np("Solo T1", "1-1"))
        .await
        .unwrap();
    let seen = service::list_prescriptions(&db, &t2, PrescriptionFilters::default())
        .await
        .unwrap();
    assert!(seen.is_empty());
}

#[tokio::test]
async fn shift_open_then_close() {
    let (db, t, u) = setup().await;
    let s = service::create_shift(
        &db,
        &t,
        NewPharmacistShift {
            user: u.to_string(),
            started_at: None,
            notes: None,
        },
    )
    .await
    .unwrap();
    assert!(s.ended_at.is_none());

    let closed = service::update_shift(
        &db,
        &t,
        &s.id,
        UpdatePharmacistShift {
            ended_at: Some(Utc::now()),
            notes: Some("turno normal".into()),
        },
    )
    .await
    .unwrap();
    assert!(closed.ended_at.is_some());

    let open_only = service::list_shifts(
        &db,
        &t,
        ShiftFilters {
            open: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(open_only.is_empty());
}

#[tokio::test]
async fn shift_close_twice_conflicts() {
    let (db, t, u) = setup().await;
    let s = service::create_shift(
        &db,
        &t,
        NewPharmacistShift {
            user: u.to_string(),
            started_at: None,
            notes: None,
        },
    )
    .await
    .unwrap();
    service::update_shift(
        &db,
        &t,
        &s.id,
        UpdatePharmacistShift {
            ended_at: Some(Utc::now()),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::update_shift(
        &db,
        &t,
        &s.id,
        UpdatePharmacistShift {
            ended_at: Some(Utc::now()),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}
