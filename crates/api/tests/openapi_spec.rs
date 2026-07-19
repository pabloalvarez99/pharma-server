//! Smoke tests for the generated OpenAPI document.
//!
//! Cheap unit-style checks that don't need a DB: build the spec via
//! `ApiDoc::openapi()` and assert the surface area we promised in
//! `docs/api/README.md`. If a `#[utoipa::path]` annotation is lost or a
//! route disappears, these fail.

use api::openapi::ApiDoc;
use utoipa::OpenApi;

fn paths() -> Vec<String> {
    ApiDoc::openapi().paths.paths.into_keys().collect()
}

#[test]
fn spec_builds_without_panic() {
    let doc = ApiDoc::openapi();
    assert_eq!(doc.info.title, "pharma-server API");
    assert!(!doc.paths.paths.is_empty(), "spec must include paths");
}

#[test]
fn spec_includes_pos_sale_endpoint() {
    assert!(paths().iter().any(|p| p == "/api/v1/pos/sale"));
}

#[test]
fn spec_includes_inventory_routes() {
    let p = paths();
    for expected in [
        "/api/v1/stock-movements",
        "/api/v1/batches",
        "/api/v1/inventory",
        "/api/v1/inventory/abc",
    ] {
        assert!(p.iter().any(|x| x == expected), "missing path {expected}");
    }
}

#[test]
fn spec_includes_catalog_routes() {
    let p = paths();
    for expected in [
        "/api/v1/products",
        "/api/v1/products/{id}",
        "/api/v1/categories",
        "/api/v1/products/bulk-price",
    ] {
        assert!(p.iter().any(|x| x == expected), "missing path {expected}");
    }
}

#[test]
fn spec_includes_prescriptions_routes() {
    let p = paths();
    for expected in [
        "/api/v1/prescriptions",
        "/api/v1/libro-recetas",
        "/api/v1/turnos-farmaceutico",
    ] {
        assert!(p.iter().any(|x| x == expected), "missing path {expected}");
    }
}

#[test]
fn spec_includes_cash_register_routes() {
    let p = paths();
    for expected in [
        "/api/v1/cash-sessions",
        "/api/v1/cash-sessions/{id}",
        "/api/v1/cash-sessions/{id}/arqueo",
        "/api/v1/cash-sessions/{id}/close",
    ] {
        assert!(p.iter().any(|x| x == expected), "missing path {expected}");
    }
}

#[test]
fn spec_includes_customers_routes() {
    let p = paths();
    for expected in [
        "/api/v1/clientes",
        "/api/v1/clientes/{id}",
        "/api/v1/loyalty",
    ] {
        assert!(p.iter().any(|x| x == expected), "missing path {expected}");
    }
}

#[test]
fn spec_defines_bearer_jwt_security_scheme() {
    let doc = ApiDoc::openapi();
    let comps = doc.components.expect("components present");
    assert!(
        comps.security_schemes.contains_key("bearer_jwt"),
        "bearer_jwt scheme must be registered"
    );
}

#[test]
fn spec_serialises_to_json() {
    let json = serde_json::to_string(&ApiDoc::openapi()).expect("json serialise");
    assert!(json.contains("\"openapi\""));
    assert!(json.contains("\"/api/v1/pos/sale\""));
    assert!(json.contains("bearer_jwt"));
}
