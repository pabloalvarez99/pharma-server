//! Integration tests for the SII config-upload endpoints (config center):
//! `POST /api/v1/dte/cert` (certificado digital) + `POST /api/v1/dte/caf`
//! (Código de Autorización de Folios). Both mirror the CLI (`pharma cert
//! import` / `pharma caf import`) but accept the artifact from the UI: the cert
//! as base64, the CAF as XML.
//!
//! Strongest assertion: upload cert + CAF via the endpoints, then emit a real
//! boleta with them — proving the uploaded material round-trips end-to-end
//! (encrypt-at-rest → decrypt → sign).

mod e2e_common;

use axum::http::StatusCode;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use e2e_common::*;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::json;
use surrealdb::sql::Thing;

const EMISOR_RUT: &str = "76123456-7";
const CERT_PASS: &str = "pass-test-123";

/// A PEM cert bundle (PKCS#8 RSA key + dummy X.509 block), base64-encoded — the
/// shape `KeyMaterial::from_keystore_bytes` accepts on the PEM path. Mirrors the
/// fixture in `dte_endpoints.rs` but returns the RAW bundle b64 for the upload
/// endpoint to encrypt + store itself.
fn cert_bundle_b64() -> String {
    let mut rng = rand::thread_rng();
    let key = rsa::RsaPrivateKey::new(&mut rng, 1024).expect("rsa 1024");
    let key_pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("pkcs8 pem")
        .to_string();
    let cert_pem =
        "-----BEGIN CERTIFICATE-----\nZHVtbXktY2VydC1kZXItYnl0ZXM=\n-----END CERTIFICATE-----\n";
    let bundle = format!("{key_pem}\n{cert_pem}");
    B64.encode(bundle.as_bytes())
}

/// A synthetic CAF XML (RSA 1024 testing-only) — same shape the SII delivers,
/// parseable by `dte::caf::parse_xml`.
fn caf_xml(tipo: i32, desde: i64, hasta: i64) -> String {
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::pkcs8::EncodePublicKey;
    let mut rng = rand::thread_rng();
    let priv_key = rsa::RsaPrivateKey::new(&mut rng, 1024).expect("rsa 1024");
    let pub_key = rsa::RsaPublicKey::from(&priv_key);
    let rsask = priv_key
        .to_pkcs1_pem(LineEnding::LF)
        .expect("rsask")
        .to_string();
    let rsapubk = pub_key.to_public_key_pem(LineEnding::LF).expect("rsapubk");
    format!(
        r#"<AUTORIZACION>
<CAF version="1.0">
<DA>
<RE>{EMISOR_RUT}</RE>
<RS>FARMACIA TEST SPA</RS>
<TD>{tipo}</TD>
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
    )
}

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

#[tokio::test]
async fn upload_cert_and_caf_then_emit_boleta() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (tid, uid, roles) = seed_tenant_admin(tdb.db.as_ref(), "up1", "a@up1.cl").await;
    let tenant = tid_thing(&tid);
    let token = token_for(&uid, &tid, roles);

    // Upload the cert via the endpoint (it encrypts + stores).
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&token),
        Some(json!({
            "pfx_base64": cert_bundle_b64(),
            "cert_passphrase": CERT_PASS,
            "rut": EMISOR_RUT,
            "vigencia_desde": "2026-01-01",
            "vigencia_hasta": "2027-01-01",
        })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "upload cert: {body}");
    assert!(body["id"].as_str().unwrap().starts_with("cert_digital:"));
    assert_eq!(body["rut"], EMISOR_RUT);
    assert!(body["blob_bytes"].as_u64().unwrap() > 0);

    // Upload a CAF for boletas (tipo 39) via the endpoint.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/caf",
        Some(&token),
        Some(json!({ "xml": caf_xml(39, 1, 5) })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "upload caf: {body}");
    assert_eq!(body["tipo"], 39);
    assert_eq!(body["folio_desde"], 1);
    assert_eq!(body["folio_hasta"], 5);
    assert_eq!(body["next_folio"], 1);

    // caf-status now reflects the uploaded folios.
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
    assert_eq!(body["folios_restantes"], 5, "{body}");

    // End-to-end proof: emit a boleta with the uploaded cert + CAF.
    set_emisor(tdb.db.as_ref(), &tenant).await;
    let pid = seed_product(
        tdb.db.as_ref(),
        &tenant,
        "Paracetamol",
        "1000",
        "600",
        10,
        None,
    )
    .await;
    let sale = seed_sale(
        tdb.db.as_ref(),
        &tenant,
        None,
        "pos_cash",
        Some("2000"),
        None,
        None,
        &[SaleLine {
            product: &pid,
            name: "Paracetamol",
            qty: 2,
            unit_price: "1000",
        }],
    )
    .await;
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/dte/boletas",
        Some(&token),
        Some(json!({ "order_id": sale.order.id, "cert_passphrase": CERT_PASS })),
        &[],
    )
    .await;
    assert_eq!(
        st,
        StatusCode::CREATED,
        "emit with uploaded cert+caf: {body}"
    );
    assert_eq!(body["estado"], "signed");
    assert_eq!(body["folio"], 1);
    assert_eq!(body["has_xml"], true);
}

#[tokio::test]
async fn upload_cert_validation_and_role_gate() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (tid, uid, roles) = seed_tenant_admin(tdb.db.as_ref(), "up2", "a@up2.cl").await;
    let token = token_for(&uid, &tid, roles);
    let tenant = tid_thing(&tid);

    let base = |pfx: &str, from: &str, to: &str| {
        json!({
            "pfx_base64": pfx,
            "cert_passphrase": CERT_PASS,
            "rut": EMISOR_RUT,
            "vigencia_desde": from,
            "vigencia_hasta": to,
        })
    };

    // Invalid base64 → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&token),
        Some(base("!!!not-base64!!!", "2026-01-01", "2027-01-01")),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "bad base64");

    // Valid base64 but not a cert (neither PKCS#12 nor PEM) → 400.
    let junk = B64.encode(b"this is not a certificate");
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&token),
        Some(base(&junk, "2026-01-01", "2027-01-01")),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "non-cert bytes");

    // Inverted vigencia → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&token),
        Some(base(&cert_bundle_b64(), "2027-01-01", "2026-01-01")),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "inverted vigencia");

    // Empty passphrase → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&token),
        Some(json!({
            "pfx_base64": cert_bundle_b64(),
            "cert_passphrase": "",
            "rut": EMISOR_RUT,
            "vigencia_desde": "2026-01-01",
            "vigencia_hasta": "2027-01-01",
        })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "empty passphrase");

    // Cashier (no admin) → 403.
    let cashier = token_for("user:caja1", &tenant.to_string(), vec!["cashier".into()]);
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        Some(&cashier),
        Some(base(&cert_bundle_b64(), "2026-01-01", "2027-01-01")),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "cashier cannot upload cert");

    // No token → 401.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/cert",
        None,
        Some(base(&cert_bundle_b64(), "2026-01-01", "2027-01-01")),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn upload_caf_validation_and_role_gate() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (tid, uid, roles) = seed_tenant_admin(tdb.db.as_ref(), "up3", "a@up3.cl").await;
    let token = token_for(&uid, &tid, roles);
    let tenant = tid_thing(&tid);

    // Garbage XML → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/caf",
        Some(&token),
        Some(json!({ "xml": "<not><a>caf</a></not>" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "garbage caf xml");

    // Empty xml → 400.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/caf",
        Some(&token),
        Some(json!({ "xml": "   " })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "empty caf xml");

    // Cashier (no admin) → 403.
    let cashier = token_for("user:caja1", &tenant.to_string(), vec!["cashier".into()]);
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/caf",
        Some(&cashier),
        Some(json!({ "xml": caf_xml(39, 1, 5) })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "cashier cannot upload caf");

    // No token → 401.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/dte/caf",
        None,
        Some(json!({ "xml": caf_xml(39, 1, 5) })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}
