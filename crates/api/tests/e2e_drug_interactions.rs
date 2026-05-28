//! E2E scenario 7 — drug-interaction warnings on the POS surface.
//!
//! The interaction ruleset lives in `domain::sales::interactions` (Beers +
//! Vademécum CL). A real interacting pair: ANTICOAGULANTE × AINE = `Critica`
//! (e.g. WARFARINA + IBUPROFENO), and WARFARINA + PARACETAMOL = `Moderada`, so
//! a 3-drug cart yields a severity-sorted list with `Critica` first.
//!
//! Two surfaces exercised:
//! * `POST /api/v1/interactions/check` — the live cart preview (bearer-only
//!   READ route, NOT role-gated, so it works over HTTP today);
//! * the POS sale response `interaction_warnings` — seeded via
//!   `domain::sales::service::post_sale` (the `/pos/sale` route is role-gated
//!   and 500s — BUG-001). The sale must COMPLETE (warnings never block).

mod e2e_common;

use std::sync::Arc;

use axum::http::StatusCode;
use e2e_common::*;
use surrealdb::sql::Thing;

async fn setup() -> (
    axum::Router,
    String,
    Arc<db::Db>,
    Thing,
    Vec<String>,
    TestDb,
) {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "farmacia-rx", "admin@rx.cl").await;
    let tenant = tid_thing(&tid);
    let token = token_for(&uid, &tid, roles);
    let db = tdb.db.clone();
    let app = api::build_router(state_free(db.clone()));

    // A real interacting pair + a third drug for severity ordering.
    let warfarina = seed_product(
        &db,
        &tenant,
        "Warfarina 5mg",
        "3500",
        "1800",
        100,
        Some("Warfarina 5 mg"),
    )
    .await;
    let ibuprofeno = seed_product(
        &db,
        &tenant,
        "Ibuprofeno 400",
        "1200",
        "600",
        100,
        Some("Ibuprofeno 400 mg"),
    )
    .await;
    let paracetamol = seed_product(
        &db,
        &tenant,
        "Paracetamol 500",
        "990",
        "450",
        100,
        Some("Paracetamol 500 mg"),
    )
    .await;
    (
        app,
        token,
        db,
        tenant,
        vec![warfarina, ibuprofeno, paracetamol],
        tdb,
    )
}

#[tokio::test]
async fn interactions_check_endpoint_flags_warfarina_ibuprofeno_critica() {
    let (app, token, _db, _tenant, products, _tdb) = setup().await;

    let body = serde_json::json!({
        "products": [products[0], products[1]], // warfarina + ibuprofeno
    });
    let (st, json) = req_json(
        &app,
        "POST",
        "/api/v1/interactions/check",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "interactions/check must be 200: {json}");

    let warnings = json["interaction_warnings"]
        .as_array()
        .expect("warnings array");
    assert_eq!(warnings.len(), 1, "exactly one pair interacts");
    let w = &warnings[0];
    assert_eq!(w["severity"], "critica", "ANTICOAGULANTE × AINE is Critica");
    let drugs = w["drugs"].as_array().unwrap();
    let names: Vec<&str> = drugs.iter().map(|d| d.as_str().unwrap()).collect();
    assert!(
        names.contains(&"WARFARINA") && names.contains(&"IBUPROFENO"),
        "the flagged pair must be WARFARINA + IBUPROFENO, got {names:?}"
    );
    assert!(
        !w["effect"].as_str().unwrap().is_empty(),
        "effect text present"
    );
    assert!(
        !w["recommendation"].as_str().unwrap().is_empty(),
        "recommendation text present"
    );
}

#[tokio::test]
async fn interactions_check_sorts_by_severity_descending() {
    let (app, token, _db, _tenant, products, _tdb) = setup().await;

    // warfarina + ibuprofeno (Critica) + paracetamol (Moderada with warfarina).
    let body = serde_json::json!({ "products": products });
    let (st, json) = req_json(
        &app,
        "POST",
        "/api/v1/interactions/check",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{json}");

    let warnings = json["interaction_warnings"].as_array().unwrap();
    assert!(warnings.len() >= 2, "at least Critica + Moderada pairs");
    assert_eq!(
        warnings[0]["severity"], "critica",
        "highest severity (Critica) sorted first"
    );
    // Severity is non-increasing across the list.
    let rank = |s: &str| match s {
        "critica" => 3,
        "mayor" => 2,
        "moderada" => 1,
        _ => 0,
    };
    let ranks: Vec<i32> = warnings
        .iter()
        .map(|w| rank(w["severity"].as_str().unwrap()))
        .collect();
    for win in ranks.windows(2) {
        assert!(
            win[0] >= win[1],
            "severities must be sorted descending: {ranks:?}"
        );
    }
}

#[tokio::test]
async fn pos_sale_surfaces_warnings_but_does_not_block() {
    let (_app, _token, db, tenant, products, _tdb) = setup().await;

    // Sell warfarina + ibuprofeno in one ticket.
    let lines = [
        SaleLine {
            product: &products[0],
            name: "Warfarina 5mg",
            qty: 1,
            unit_price: "3500",
        },
        SaleLine {
            product: &products[1],
            name: "Ibuprofeno 400",
            qty: 1,
            unit_price: "1200",
        },
    ];
    let resp = seed_sale(
        &db,
        &tenant,
        None,
        "pos_cash",
        Some("4700"),
        None,
        None,
        &lines,
    )
    .await;

    // The sale COMPLETED (order persisted, paid) despite the critical interaction.
    assert_eq!(
        resp.order.status, "paid",
        "interaction warnings never block the sale"
    );
    assert_eq!(resp.items.len(), 2, "both lines sold");

    // ...and the warning rode along on the response.
    assert_eq!(
        resp.interaction_warnings.len(),
        1,
        "the Critica pair surfaced"
    );
    let w = &resp.interaction_warnings[0];
    assert_eq!(
        w.severity,
        domain::sales::interactions::Severity::Critica,
        "WARFARINA × IBUPROFENO is Critica"
    );

    // Stock still moved (sale is real).
    #[derive(serde::Deserialize)]
    struct P {
        stock: i64,
    }
    let w0 = surrealdb::sql::thing(&products[0]).unwrap();
    let mut q = db
        .query("SELECT stock FROM product WHERE id = $p LIMIT 1")
        .bind(("p", w0))
        .await
        .unwrap();
    let p: Option<P> = q.take(0).unwrap();
    assert_eq!(p.unwrap().stock, 99, "warfarina stock 100-1=99");
}
