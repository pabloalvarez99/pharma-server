//! Integration tests del wiring `/api/v1/dte/*` (Fase 9.1).
//!
//! Cubre el ciclo completo sin red: POS sale → emit boleta (folio + TED +
//! XML-DSig) → list/export → caf-status → tier gate de envío (402 Free) →
//! cancel → re-emisión. El envío SII real (red sandbox maullin) es la
//! subtask 9.1.l — acá sólo se verifica que el gate corre ANTES de tocar red
//! y que los guards de estado bloquean lo no-enviable.

mod e2e_common;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use e2e_common::*;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use serde_json::json;
use surrealdb::sql::Thing;

const EMISOR_RUT: &str = "76123456-7";
const CERT_PASS: &str = "pass-test-123";

/// Setting `dte.emisor` del tenant (JSON de `dte::EmisorConfig`).
async fn set_emisor(db: &db::Db, tenant: &Thing) {
    let emisor = json!({
        "rut": EMISOR_RUT,
        "razon_social": "FARMACIA TEST SPA",
        "giro": "FARMACIA",
        "direccion": "CALLE 123",
        "comuna": "COQUIMBO",
        "ciudad": "COQUIMBO",
        "acteco": 477310,
    });
    domain::sales::service::set_setting(db, tenant, "dte.emisor", &emisor.to_string())
        .await
        .expect("set emisor");
}

/// CAF sintético (RSA 1024 testing-only, espejo de crates/dte tests/common)
/// parseado + persistido como lo hace `pharma caf import`.
async fn import_caf(db: &db::Db, tenant: &Thing, desde: i64, hasta: i64) {
    let mut rng = rand::thread_rng();
    let priv_key = rsa::RsaPrivateKey::new(&mut rng, 1024).expect("rsa 1024");
    let pub_key = rsa::RsaPublicKey::from(&priv_key);
    let rsask = priv_key
        .to_pkcs1_pem(LineEnding::LF)
        .expect("rsask pem")
        .to_string();
    let rsapubk = pub_key
        .to_public_key_pem(LineEnding::LF)
        .expect("rsapubk pem");
    let xml = format!(
        r#"<AUTORIZACION>
<CAF version="1.0">
<DA>
<RE>{EMISOR_RUT}</RE>
<RS>FARMACIA TEST SPA</RS>
<TD>39</TD>
<RNG><D>{desde}</D><H>{hasta}</H></RNG>
<FA>2026-01-01</FA>
<RSAPK><M>placeholder</M><E>Aw==</E></RSAPK>
<IDK>100</IDK>
</DA>
<FRMA algoritmo="SHA1withRSA">PLACEHOLDER_SII_SIGNATURE</FRMA>
</CAF>
<RSASK>{rsask}</RSASK>
<RSAPUBK>{rsapubk}</RSAPUBK>
</AUTORIZACION>"#
    );
    let caf = dte::caf::parse_xml(&xml).expect("parse caf");
    db.query(
        "CREATE caf SET tenant = $t, tipo_dte = $tipo, folio_desde = $d, \
         folio_hasta = $h, next_folio = $n, fecha_autorizacion = $fa, \
         rut_emisor = $rut, xml = $xml, activo = true",
    )
    .bind(("t", tenant.clone()))
    .bind(("tipo", caf.tipo_dte.code()))
    .bind(("d", caf.folio_desde))
    .bind(("h", caf.folio_hasta))
    .bind(("n", caf.next_folio))
    .bind(("fa", surrealdb::sql::Datetime::from(caf.fecha_autorizacion)))
    .bind(("rut", caf.rut_emisor.clone()))
    .bind(("xml", caf.xml.clone()))
    .await
    .expect("create caf")
    .check()
    .expect("caf insert ok");
}

/// Cert empresa como bundle PEM cifrado at-rest (el path soportado hasta que
/// 9.1.b.3 traiga parse PFX nativo). `KeyMaterial::from_pem` no parsea X.509
/// (base64-decode del bloque) → cert dummy basta para firmar en tests.
async fn import_cert_pem(db: &db::Db, tenant: &Thing, passphrase: &str) {
    let mut rng = rand::thread_rng();
    let key = rsa::RsaPrivateKey::new(&mut rng, 1024).expect("rsa 1024");
    let key_pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("pkcs8 pem")
        .to_string();
    let cert_pem =
        "-----BEGIN CERTIFICATE-----\nZHVtbXktY2VydC1kZXItYnl0ZXM=\n-----END CERTIFICATE-----\n";
    let bundle = format!("{key_pem}\n{cert_pem}");
    let enc = dte::cert::encrypt_pfx(bundle.as_bytes(), passphrase).expect("encrypt pem bundle");
    let tid = pharma_core::tenant::TenantId::new(tenant.id.to_raw());
    dte::cert::store_cert(
        db,
        tid,
        &enc,
        (
            Utc::now() - Duration::days(1),
            Utc::now() + Duration::days(365),
        ),
        EMISOR_RUT,
    )
    .await
    .expect("store cert");
}

/// Tenant listo para emitir: producto + venta POS + emisor + CAF + cert.
/// Retorna (app router se arma fuera), token, tenant Thing y order id.
async fn seed_emission_fixture(db: &std::sync::Arc<db::Db>, slug: &str) -> (String, Thing, String) {
    let (tid, uid, roles) = seed_tenant_admin(db.as_ref(), slug, &format!("a@{slug}.cl")).await;
    let tenant = tid_thing(&tid);
    let token = token_for(&uid, &tid, roles);
    let pid = seed_product(
        db.as_ref(),
        &tenant,
        "Paracetamol 500mg",
        "1000",
        "600",
        10,
        None,
    )
    .await;
    let sale = seed_sale(
        db.as_ref(),
        &tenant,
        None,
        "pos_cash",
        Some("2000"),
        None,
        None,
        &[SaleLine {
            product: &pid,
            name: "Paracetamol 500mg",
            qty: 2,
            unit_price: "1000",
        }],
    )
    .await;
    set_emisor(db.as_ref(), &tenant).await;
    import_caf(db.as_ref(), &tenant, 1, 5).await;
    import_cert_pem(db.as_ref(), &tenant, CERT_PASS).await;
    (token, tenant, sale.order.id)
}

#[tokio::test]
async fn dte_emit_export_cancel_caf_flow() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (token, _tenant, order_id) = seed_emission_fixture(&tdb.db, "dte1").await;

    // Emit: folio 1, signed, total de la orden.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "emit: {body}");
    assert_eq!(body["tipo"], 39);
    assert_eq!(body["folio"], 1);
    assert_eq!(body["estado"], "signed");
    assert_eq!(body["monto_total"], "2000");
    assert_eq!(body["rut_emisor"], EMISOR_RUT);
    assert_eq!(body["rut_receptor"], "66666666-6");
    assert_eq!(body["has_xml"], true);
    let dte_id = body["id"].as_str().expect("id").to_string();

    // Doble emisión de la misma orden → 409 con el id existente.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "dup emit: {body}");
    assert_eq!(body["error"]["details"]["dte_id"], dte_id);

    // List con filtros.
    let (st, body) = req_json(
        &app,
        "GET",
        "/api/v1/dte?estado=signed&tipo=39",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.as_array().expect("array").len(), 1, "{body}");

    // Export XML firmado: DTE + TED + Signature presentes.
    let (st, body) = req_json(
        &app,
        "GET",
        &format!("/api/v1/dte/{dte_id}/xml"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let xml = body.as_str().expect("xml body");
    assert!(xml.contains("<DTE"), "sin <DTE>: {xml}");
    assert!(xml.contains("<TED"), "sin TED");
    assert!(xml.contains("<Signature"), "sin XML-DSig");
    assert!(xml.contains("<MntTotal>2000</MntTotal>"), "total en XML");

    // CAF status: 1 de 5 usado.
    let (st, body) = req_json(
        &app,
        "GET",
        "/api/v1/dte/caf-status?tipo=39",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["folios_restantes"], 4, "{body}");
    assert_eq!(body["cafs"][0]["next_folio"], 2);

    // Send con licencia Free → 402 ANTES de tocar red (gate 9.1.j).
    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/send"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::PAYMENT_REQUIRED, "send free: {body}");
    assert_eq!(body["error"]["code"], "FEATURE_REQUIRES_UPGRADE");
    assert_eq!(body["error"]["details"]["feature"], "dte.sii_send");
    assert_eq!(body["error"]["details"]["tier_required"], "pro");

    // Poll sin envío → 409.
    let (st, _b) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/poll"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // Cancel (signed → cancelled) con razón.
    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/cancel"),
        Some(&token),
        Some(json!({ "reason": "folio de prueba" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "cancel: {body}");
    assert_eq!(body["estado"], "cancelled");

    // Cancel doble → 409 (cancelled → cancelled no es transición válida).
    let (st, _b) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/cancel"),
        Some(&token),
        Some(json!({ "reason": "otra vez" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // Anulada la primera, la orden puede re-emitir → folio 2.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "re-emit: {body}");
    assert_eq!(body["folio"], 2);
}

#[tokio::test]
async fn dte_emit_guards() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (tid, uid, roles) = seed_tenant_admin(tdb.db.as_ref(), "dte2", "a@dte2.cl").await;
    let tenant = tid_thing(&tid);
    let token = token_for(&uid, &tid, roles);
    let pid = seed_product(
        tdb.db.as_ref(),
        &tenant,
        "Ibuprofeno",
        "500",
        "300",
        10,
        None,
    )
    .await;
    let sale = seed_sale(
        tdb.db.as_ref(),
        &tenant,
        None,
        "pos_cash",
        Some("500"),
        None,
        None,
        &[SaleLine {
            product: &pid,
            name: "Ibuprofeno",
            qty: 1,
            unit_price: "500",
        }],
    )
    .await;
    let order_id = sale.order.id;
    let emit = |body: serde_json::Value| {
        req_json(
            &app,
            "POST",
            "/api/v1/dte/boletas",
            Some(&token),
            Some(body),
            &[],
        )
    };

    // Orden inexistente → 404.
    let (st, _b) = emit(json!({ "order_id": "order:nope", "cert_passphrase": "x" })).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // Sin emisor configurado → 400 con instrucción.
    let (st, body) = emit(json!({ "order_id": order_id, "cert_passphrase": "x" })).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dte.emisor"),
        "{body}"
    );

    // Emisor OK, sin cert → 409.
    set_emisor(tdb.db.as_ref(), &tenant).await;
    let (st, body) = emit(json!({ "order_id": order_id, "cert_passphrase": "x" })).await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");

    // Cert OK, passphrase mala → 400 (GCM tag mismatch).
    import_cert_pem(tdb.db.as_ref(), &tenant, CERT_PASS).await;
    let (st, body) = emit(json!({ "order_id": order_id, "cert_passphrase": "WRONG" })).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{body}");

    // Cert + passphrase OK, sin CAF → 409 FOLIO_EXHAUSTED.
    let (st, body) = emit(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })).await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "FOLIO_EXHAUSTED");

    // Sin token → 401.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        None,
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn dte_role_estado_and_tenant_isolation() {
    let tdb = spawn_db().await;
    // Pro tier: el gate de tier pasa para boleta — los guards de estado deben
    // bloquear ANTES de tocar red (ningún DTE queda 'signed' al llamar send).
    let app = api::build_router(state_pro(tdb.db.clone(), &[]));
    let (token, tenant, order_id) = seed_emission_fixture(&tdb.db, "dte3").await;

    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{body}");
    let dte_id = body["id"].as_str().unwrap().to_string();

    // Cashier puro NO puede cancelar (admin+) → 403. (El role gate lee los
    // claims del JWT, no la DB — un user sintético del mismo tenant basta.)
    let cashier_token = token_for("user:cajero1", &tenant.to_string(), vec!["cashier".into()]);
    let (st, _b) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/cancel"),
        Some(&cashier_token),
        Some(json!({ "reason": "no debería" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Cancelar con admin y luego send (Pro) → 409 por estado, sin red.
    let (st, _b) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/cancel"),
        Some(&token),
        Some(json!({ "reason": "test estado" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/dte/{dte_id}/send"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "send cancelled: {body}");

    // Aislamiento multi-tenant: otro tenant no ve el DTE → 404.
    let (tid2, uid2, roles2) = seed_tenant_admin(tdb.db.as_ref(), "dte4", "a@dte4.cl").await;
    let token2 = token_for(&uid2, &tid2, roles2);
    let (st, _b) = req_json(
        &app,
        "GET",
        &format!("/api/v1/dte/{dte_id}"),
        Some(&token2),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn libro_ventas_monthly_xml() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (token, _tenant, order_id) = seed_emission_fixture(&tdb.db, "libro1").await;
    let period = Utc::now().format("%Y-%m").to_string();

    // Emitir boleta (queda signed).
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "emit: {body}");
    let dte_id = body["id"].as_str().expect("id").to_string();

    // Libro del mes con el DTE sólo `signed` → libro vacío (sólo accepted).
    let (st, body) = req_json(
        &app,
        "GET",
        &format!("/api/v1/dte/libro-ventas?period={period}"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "libro vacío: {body}");
    let xml = body.as_str().expect("xml");
    assert!(xml.contains("LibroCompraVenta"), "raíz libro: {xml}");
    assert!(xml.contains(&period), "período tributario: {xml}");
    assert!(!xml.contains("<NroDoc>"), "signed no debe entrar: {xml}");

    // Marcar accepted (lo que haría el poll SII) y re-pedir el libro.
    let thing = surrealdb::sql::thing(&dte_id).expect("dte thing");
    tdb.db
        .query("UPDATE $id SET estado = 'accepted'")
        .bind(("id", thing))
        .await
        .expect("update accepted")
        .check()
        .expect("update ok");
    let (st, body) = req_json(
        &app,
        "GET",
        &format!("/api/v1/dte/libro-ventas?period={period}"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "libro: {body}");
    let xml = body.as_str().expect("xml");
    assert!(xml.contains("<TpoDoc>39</TpoDoc>"), "tipo doc: {xml}");
    assert!(xml.contains("<NroDoc>1</NroDoc>"), "folio: {xml}");
    assert!(xml.contains("<MntTotal>2000</MntTotal>"), "total: {xml}");
    assert!(xml.contains("<TotDoc>1</TotDoc>"), "resumen count: {xml}");

    // Período inválido → 400.
    let (st, _b) = req_json(
        &app,
        "GET",
        "/api/v1/dte/libro-ventas?period=2026-13",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Mes sin movimientos → 200 libro vacío.
    let (st, body) = req_json(
        &app,
        "GET",
        "/api/v1/dte/libro-ventas?period=2020-01",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert!(!body.as_str().expect("xml").contains("<NroDoc>"));

    // Sin token → 401.
    let (st, _b) = req_json(
        &app,
        "GET",
        &format!("/api/v1/dte/libro-ventas?period={period}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn libro_ventas_signed_xml() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (token, _tenant, order_id) = seed_emission_fixture(&tdb.db, "librosig").await;
    let period = Utc::now().format("%Y-%m").to_string();

    // Emitir + marcar accepted para que el libro tenga movimiento.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": order_id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "emit: {body}");
    let dte_id = body["id"].as_str().expect("id").to_string();
    let thing = surrealdb::sql::thing(&dte_id).expect("dte thing");
    tdb.db
        .query("UPDATE $id SET estado = 'accepted'")
        .bind(("id", thing))
        .await
        .expect("update accepted")
        .check()
        .expect("update ok");

    // Libro firmado: Signature enveloped sobre EnvioLibro, verificable.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/libro-ventas/signed",
        Some(&token),
        Some(json!({ "period": period, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "libro firmado: {body}");
    let xml = body.as_str().expect("xml");
    assert!(xml.contains("<NroDoc>1</NroDoc>"), "movimiento: {xml}");
    assert!(
        xml.contains("<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\">"),
        "firma presente: {xml}"
    );
    // <Signature> tras </EnvioLibro>, antes de </LibroCompraVenta>.
    let pos_envio = xml.find("</EnvioLibro>").expect("cierre EnvioLibro");
    let pos_sig = xml.find("<Signature").expect("Signature");
    let pos_root = xml.find("</LibroCompraVenta>").expect("cierre raíz");
    assert!(pos_envio < pos_sig && pos_sig < pos_root, "posición firma");
    dte::verify_libro_signature(xml).expect("firma libro verifica");

    // Passphrase mala → error (cert no descifra), sin XML.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/libro-ventas/signed",
        Some(&token),
        Some(json!({ "period": period, "cert_passphrase": "wrong-pass" })),
        &[],
    )
    .await;
    assert_ne!(st, StatusCode::OK, "passphrase mala no firma");

    // Período inválido → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/libro-ventas/signed",
        Some(&token),
        Some(json!({ "period": "2026-99", "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Sin token → 401.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/libro-ventas/signed",
        None,
        Some(json!({ "period": period, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}
