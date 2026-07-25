//! Integration tests for the write-action framework (ADR-0016, Wave 3) over a
//! kv-mem SurrealDB. Covers the propose → confirm handshake, single-use /
//! expiry / tenant-binding of the confirm token, the two whitelisted actions
//! end-to-end against real domain services, and that every execution writes an
//! audit row. Same harness style as `tests/deterministic.rs`.

use std::str::FromStr;

use assist::{build, execute, parse_action, Action, ActionParse, ActionStore, BuildOutcome};
use domain::catalog::model::{NewProduct, ProductFilters};
use domain::catalog::service as catalog;
use domain::credit::repo as credit_repo;
use domain::customers::model::{CustomerFilters, NewCustomer};
use domain::customers::service as customers;
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

// ---- crear_cliente end-to-end -----------------------------------------------

#[tokio::test]
async fn crear_cliente_propose_no_write_then_confirm_executes_and_audits() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();

    let parsed = parse_action("crea un cliente Juan Pérez rut 12.345.678-9");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        _ => panic!("expected Ready"),
    };
    assert!(matches!(action, Action::CrearCliente { .. }));
    let proposal = store.propose(action, &tenant);

    let before = customers::list_customers(&db, &tenant, CustomerFilters::default())
        .await
        .unwrap();
    assert_eq!(before.len(), 0, "propose must not create a customer");
    assert_eq!(audit_action_count(&db, &tenant).await, 0);

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "crear_cliente");

    let after = customers::list_customers(&db, &tenant, CustomerFilters::default())
        .await
        .unwrap();
    assert_eq!(after.len(), 1, "confirm creates exactly one customer");
    assert_eq!(after[0].name, "Juan Perez");
    assert_eq!(after[0].rut.as_deref(), Some("123456789"));
    assert_eq!(audit_action_count(&db, &tenant).await, 1);
}

// ---- crear_producto_rapido end-to-end ---------------------------------------

#[tokio::test]
async fn crear_producto_propose_no_write_then_confirm_executes_and_audits() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();

    let parsed = parse_action("crea un producto Aspirina a $1000 stock 15");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        _ => panic!("expected Ready"),
    };
    assert!(matches!(action, Action::CrearProductoRapido { .. }));
    let proposal = store.propose(action, &tenant);

    let before = catalog::list_products(&db, &tenant, ProductFilters::default())
        .await
        .unwrap();
    assert_eq!(before.len(), 0, "propose must not create a product");

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "crear_producto_rapido");

    let after = catalog::list_products(&db, &tenant, ProductFilters::default())
        .await
        .unwrap();
    assert_eq!(after.len(), 1, "confirm creates exactly one product");
    assert_eq!(after[0].name, "Aspirina");
    assert_eq!(after[0].price, dec("1000"));
    assert_eq!(after[0].stock, 15);
    assert_eq!(audit_action_count(&db, &tenant).await, 1);
}

// ---- ajustar_precio end-to-end ----------------------------------------------

async fn seed_product(db: &Db, tenant: &Thing, name: &str, price: &str) {
    catalog::create_product(
        db,
        tenant,
        NewProduct {
            name: name.into(),
            slug: None,
            description: None,
            price: dec(price),
            cost_price: None,
            stock: 0,
            category: None,
            image_url: None,
            external_id: None,
            laboratory: None,
            therapeutic_action: None,
            active_ingredient: None,
            prescription_type: None,
            presentation: None,
            discount_percent: None,
            attrs: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn ajustar_precio_resolves_product_and_updates_price() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    seed_product(&db, &tenant, "Paracetamol", "500").await;

    let parsed = parse_action("cambia el precio de paracetamol a $1500");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        _ => panic!("expected Ready"),
    };
    match &action {
        Action::AjustarPrecio {
            product_name,
            old_price,
            new_price,
            ..
        } => {
            assert_eq!(product_name, "Paracetamol");
            assert_eq!(*old_price, dec("500"));
            assert_eq!(*new_price, dec("1500"));
        }
        _ => panic!("expected AjustarPrecio"),
    }
    let proposal = store.propose(action, &tenant);

    // Propose must not change the price.
    let before = catalog::list_products(&db, &tenant, ProductFilters::default())
        .await
        .unwrap();
    assert_eq!(before[0].price, dec("500"), "propose must not reprice");

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "ajustar_precio");

    let after = catalog::list_products(&db, &tenant, ProductFilters::default())
        .await
        .unwrap();
    assert_eq!(after[0].price, dec("1500"), "confirm applies the new price");
    assert_eq!(audit_action_count(&db, &tenant).await, 1);
}

#[tokio::test]
async fn ajustar_precio_unknown_product_is_rejected_without_token() {
    let (db, tenant, _user) = setup().await;
    let parsed = parse_action("cambia el precio de inexistente a $1500");
    match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Reject(msg) => assert!(msg.to_lowercase().contains("producto")),
        _ => panic!("unknown product must be rejected, no token issued"),
    }
}

// ---- registrar_abono end-to-end ---------------------------------------------

/// Create a customer and (optionally) fiar them `debt` pesos via a ledger
/// `cargo`, the same row the POS writes for a `pos_fiado` sale. Returns the
/// customer record id.
async fn seed_customer_with_debt(db: &Db, tenant: &Thing, name: &str, debt: &str) -> Thing {
    let c = customers::create_customer(
        db,
        tenant,
        NewCustomer {
            name: name.into(),
            rut: None,
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();
    let cid = surrealdb::sql::thing(&c.id).unwrap();
    if !debt.is_empty() {
        // Unique `order` per seed: `post_cargo` is idempotent by order id.
        let order = Thing::from(("order", name));
        credit_repo::post_cargo(db, tenant, &cid, &order, dec(debt), None)
            .await
            .unwrap();
    }
    cid
}

#[tokio::test]
async fn abono_resolves_customer_and_records_payment() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    let cid = seed_customer_with_debt(&db, &tenant, "Ana Perez", "12000").await;

    let parsed = parse_action("abónale 5000 a doña Ana Perez");
    let action = match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Ready(a) => a,
        other => panic!(
            "expected Ready, got {}",
            match other {
                BuildOutcome::Reject(m) => m,
                _ => "NotAnAction".into(),
            }
        ),
    };
    match &action {
        Action::RegistrarAbono {
            customer_name,
            amount,
            debt_before,
            ..
        } => {
            assert_eq!(customer_name, "Ana Perez");
            assert_eq!(*amount, dec("5000"));
            assert_eq!(*debt_before, dec("12000"));
        }
        _ => panic!("expected RegistrarAbono"),
    }
    let proposal = store.propose(action, &tenant);

    // Propose must not touch the ledger.
    assert_eq!(
        credit_repo::balance(&db, &tenant, &cid).await.unwrap(),
        dec("12000"),
        "propose must not record the abono"
    );
    assert_eq!(audit_action_count(&db, &tenant).await, 0);

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "registrar_abono");

    assert_eq!(
        credit_repo::balance(&db, &tenant, &cid).await.unwrap(),
        dec("7000"),
        "confirm applies the abono"
    );
    assert_eq!(audit_action_count(&db, &tenant).await, 1);
}

#[tokio::test]
async fn abono_without_debt_is_rejected_without_token() {
    let (db, tenant, _user) = setup().await;
    seed_customer_with_debt(&db, &tenant, "Ana Perez", "").await;
    let parsed = parse_action("abónale 5000 a Ana Perez");
    match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Reject(msg) => assert!(
            msg.to_lowercase().contains("deuda"),
            "expected a no-debt reject, got {msg}"
        ),
        _ => panic!("a customer with no debt must be rejected, no token issued"),
    }
}

#[tokio::test]
async fn abono_over_debt_is_rejected_without_token() {
    let (db, tenant, _user) = setup().await;
    seed_customer_with_debt(&db, &tenant, "Ana Perez", "3000").await;
    let parsed = parse_action("abónale 5000 a Ana Perez");
    match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Reject(msg) => assert!(
            msg.to_lowercase().contains("supera"),
            "expected an over-debt reject, got {msg}"
        ),
        _ => panic!("an abono over the debt must be rejected, no token issued"),
    }
}

#[tokio::test]
async fn abono_unknown_customer_is_rejected_without_token() {
    let (db, tenant, _user) = setup().await;
    let parsed = parse_action("abónale 5000 a inexistente");
    match build(&db, &tenant, parsed).await.unwrap() {
        BuildOutcome::Reject(msg) => assert!(msg.to_lowercase().contains("cliente")),
        _ => panic!("unknown customer must be rejected, no token issued"),
    }
}
