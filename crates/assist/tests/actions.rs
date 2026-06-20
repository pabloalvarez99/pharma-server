//! Integration tests for the write-action framework (ADR-0016, Wave 3) over a
//! kv-mem SurrealDB. Covers the propose → confirm handshake, single-use /
//! expiry / tenant-binding of the confirm token, the two whitelisted actions
//! end-to-end against real domain services, and that every execution writes an
//! audit row. Same harness style as `tests/deterministic.rs`.

use std::str::FromStr;

use assist::{build, execute, parse_action, Action, ActionParse, ActionStore, BuildOutcome};
use domain::expenses::model::ExpenseFilters;
use domain::expenses::service as expenses;
use domain::purchasing::model::{NewSupplier, PurchaseOrderFilters};
use domain::purchasing::service as purchasing;
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

async fn audit_action_count(db: &Db, tenant: &Thing) -> usize {
    let n: Option<i64> = db
        .query(
            "SELECT count() AS n FROM audit_log WHERE tenant = $t AND method = 'ACTION' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take((0, "n"))
        .unwrap();
    n.unwrap_or(0) as usize
}

// ---- registrar_gasto end-to-end ---------------------------------------------

#[tokio::test]
async fn propose_does_not_write_then_confirm_executes_and_audits() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();

    // PROPOSE: parse + build + store a token. Nothing must be written yet.
    let parsed = parse_action("registra un gasto de 5000 en arriendo");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        _ => panic!("expected Ready, got a reject/none"),
    };
    let proposal = store.propose(action, &tenant);

    let before = expenses::list_expenses(&db, &tenant, ExpenseFilters::default())
        .await
        .unwrap();
    assert_eq!(before.len(), 0, "propose must not write any expense");
    assert_eq!(audit_action_count(&db, &tenant).await, 0);

    // CONFIRM: consume the token, execute.
    let action = store
        .consume(&proposal.confirm_token, &tenant)
        .expect("valid token");
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "registrar_gasto");

    let after = expenses::list_expenses(&db, &tenant, ExpenseFilters::default())
        .await
        .unwrap();
    assert_eq!(after.len(), 1, "confirm must create exactly one expense");
    assert_eq!(after[0].amount, dec("5000"));
    assert_eq!(after[0].category, "arriendo");
    assert_eq!(
        audit_action_count(&db, &tenant).await,
        1,
        "execution audited"
    );

    // Replay the same token: rejected, no second expense.
    assert!(store.consume(&proposal.confirm_token, &tenant).is_err());
    let again = expenses::list_expenses(&db, &tenant, ExpenseFilters::default())
        .await
        .unwrap();
    assert_eq!(again.len(), 1, "single-use token cannot double-write");
}

// ---- crear_orden_compra_draft end-to-end ------------------------------------

#[tokio::test]
async fn oc_draft_resolves_supplier_and_creates_draft() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    purchasing::create_supplier(
        &db,
        &tenant,
        NewSupplier {
            name: "Farmaltda".into(),
            rut: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            default_invoice_format: None,
        },
    )
    .await
    .unwrap();

    let parsed = parse_action("crea una orden de compra de 10 paracetamol a Farmaltda a $500");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        _ => panic!("expected Ready"),
    };
    assert!(matches!(action, Action::CrearOrdenCompraDraft { .. }));
    let proposal = store.propose(action, &tenant);

    // Nothing created on propose.
    let before = purchasing::list_purchase_orders(&db, &tenant, PurchaseOrderFilters::default())
        .await
        .unwrap();
    assert_eq!(before.len(), 0);

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "crear_orden_compra_draft");

    let pos = purchasing::list_purchase_orders(&db, &tenant, PurchaseOrderFilters::default())
        .await
        .unwrap();
    assert_eq!(pos.len(), 1);
    assert_eq!(pos[0].status, "draft", "OC must land in draft, not sent");
    assert_eq!(pos[0].total, dec("5000")); // 10 * 500
    assert_eq!(audit_action_count(&db, &tenant).await, 1);
}

#[tokio::test]
async fn oc_unknown_supplier_is_rejected_without_token() {
    let (db, tenant, _user) = setup().await;
    let parsed = parse_action("crea una orden de compra de 10 paracetamol a Inexistente a $500");
    match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Reject(msg) => assert!(msg.to_lowercase().contains("proveedor")),
        _ => panic!("unknown supplier must be rejected, no token issued"),
    }
}

// ---- token safety against a real execute path -------------------------------

#[tokio::test]
async fn expired_token_cannot_execute() {
    let (db, tenant, _user) = setup().await;
    let store = ActionStore::new();
    let action = Action::RegistrarGasto {
        category: "luz".into(),
        description: "Luz".into(),
        amount: dec("1000"),
        payment_method: "cash".into(),
    };
    let proposal = store.propose_with_ttl(action, &tenant, -1);
    assert!(store.consume(&proposal.confirm_token, &tenant).is_err());

    let expenses = expenses::list_expenses(&db, &tenant, ExpenseFilters::default())
        .await
        .unwrap();
    assert_eq!(expenses.len(), 0, "expired token never reaches execute");
}

#[tokio::test]
async fn token_from_one_tenant_cannot_execute_for_another() {
    let (db, tenant_a, _user) = setup().await;
    let tenant_b: Thing = db
        .query("CREATE tenant SET name='B', slug='b' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let store = ActionStore::new();
    let action = Action::RegistrarGasto {
        category: "luz".into(),
        description: "Luz".into(),
        amount: dec("1000"),
        payment_method: "cash".into(),
    };
    let proposal = store.propose(action, &tenant_a);
    // Tenant B presents A's token: rejected.
    assert!(store.consume(&proposal.confirm_token, &tenant_b).is_err());
    // A can still use it.
    assert!(store.consume(&proposal.confirm_token, &tenant_a).is_ok());
}

#[tokio::test]
async fn read_questions_are_not_actions() {
    assert_eq!(parse_action("cuánto vendí hoy"), ActionParse::NotAnAction);
    assert_eq!(parse_action("gastos del mes"), ActionParse::NotAnAction);
    assert_eq!(
        parse_action("stock de paracetamol"),
        ActionParse::NotAnAction
    );
}
