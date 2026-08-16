//! Chips feria Android (S3): «Vendí 2 kg de tomates a 2000» y
//! «Anota 2 kg de tomates a 2000 fiado a Don Juan».

use std::str::FromStr;

use assist::{build, parse_action, Action, ActionParse, BuildOutcome};
use domain::cash_register::model::OpenSessionInput;
use domain::cash_register::service as caja;
use domain::provisioning::SETTING_VERTICAL;
use domain::sales::service as sales;
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

async fn setup() -> (Db, Thing, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations).await.expect("migr");
    let tenant: Thing = db
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let user: Thing = db
        .query("CREATE user SET tenant=$t, email='a@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    (db, tenant, user)
}

async fn set_feria(db: &Db, tenant: &Thing) {
    sales::set_setting(db, tenant, SETTING_VERTICAL, "feria")
        .await
        .unwrap();
}

/// Chip cash: «Vendí 2 kg de tomates a 2000» — Vender Ready en feria vacía.
#[tokio::test]
async fn chip_vendi_2kg_tomates_a_2000_parse_y_ready_feria() {
    let (db, tenant, _user) = setup().await;
    set_feria(&db, &tenant).await;

    let q = "Vendí 2 kg de tomates a 2000";
    let parsed = parse_action(q);
    match &parsed {
        ActionParse::Venta {
            lines,
            fiado,
            customer_name,
        } => {
            assert_eq!(lines.len(), 1);
            assert!(
                lines[0].product_name.eq_ignore_ascii_case("tomates"),
                "product={}",
                lines[0].product_name
            );
            assert_eq!(lines[0].quantity, 2);
            assert_eq!(lines[0].unit_price, Some(dec("2000")));
            assert!(!fiado);
            assert!(customer_name.is_none());
        }
        other => panic!("expected Venta for chip cash, got {other:?}"),
    }

    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("feria+precio debe Ready (ensure), got Reject: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready"),
    };
    match action {
        Action::Vender { lines, total, .. } => {
            assert_eq!(lines.len(), 1);
            assert!(
                lines[0].product_name.eq_ignore_ascii_case("tomates"),
                "{}",
                lines[0].product_name
            );
            assert_eq!(lines[0].quantity, 2);
            assert_eq!(lines[0].unit_price, dec("2000"));
            assert_eq!(total, dec("4000"));
        }
        other => panic!("expected Vender, got {other:?}"),
    }
}

/// Chip fiado: «Anota 2 kg de tomates a 2000 fiado a Don Juan».
#[test]
fn chip_anota_2kg_tomates_fiado_don_juan_parse() {
    let q = "Anota 2 kg de tomates a 2000 fiado a Don Juan";
    match parse_action(q) {
        ActionParse::Venta {
            lines,
            fiado,
            customer_name,
        } => {
            assert!(fiado, "chip fiado debe ser fiado");
            assert_eq!(lines.len(), 1);
            assert!(
                lines[0].product_name.eq_ignore_ascii_case("tomates"),
                "product={}",
                lines[0].product_name
            );
            assert_eq!(lines[0].quantity, 2);
            assert_eq!(lines[0].unit_price, Some(dec("2000")));
            let name = customer_name.expect("cliente del fiado");
            // clean_abono_name quita «don»; queda Juan o Don Juan.
            assert!(
                name.eq_ignore_ascii_case("juan")
                    || name.eq_ignore_ascii_case("don juan"),
                "cliente={name}"
            );
        }
        other => panic!("expected Venta fiado for chip, got {other:?}"),
    }
}

/// Farmacia (sin vertical feria) + producto inexistente: sigue «créalo primero».
#[tokio::test]
async fn chip_cash_farmacia_sin_producto_pide_crearlo() {
    let (db, tenant, user) = setup().await;
    // Caja abierta: si no, el reject es de caja y no de catálogo.
    caja::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let msg = match build(&db, &tenant, parse_action("Vendí 2 kg de tomates a 2000"))
        .await
        .unwrap()
    {
        BuildOutcome::Reject(m) => m,
        BuildOutcome::Ready(_) => panic!("farmacia no crea SKU al vuelo"),
        BuildOutcome::NotAnAction => panic!("farmacia no crea SKU al vuelo"),
    };
    assert!(
        msg.contains("créalo") || msg.contains("crealo"),
        "{msg}"
    );
}
