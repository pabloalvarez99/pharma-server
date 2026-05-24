//! OpenAPI document + Swagger UI mount.
//!
//! Today no handler carries `#[utoipa::path(...)]`, so `paths()` is empty (the
//! interactive "try it out" surface is therefore minimal) — see the GAP note
//! below. The domain DTOs *do* derive [`utoipa::ToSchema`], so we register a
//! representative one under `components.schemas` to give the document real
//! content and a stable hook for future per-handler `paths(...)` registration.
//!
//! The doc is served at `/api-docs/openapi.json` and the Swagger UI at
//! `/swagger-ui`, but only when [`AppState::docs_enabled`] is `true` (config
//! flag `docs.enabled`, env `PHARMA__DOCS__ENABLED`). On a hardened on-prem box
//! the operator flips it off to keep the API surface off the wire.
//!
//! GAP: paths are not yet aggregated. Handlers in `v1/`, `routes.rs`,
//! `health.rs` lack `#[utoipa::path]`. Registering them is intentionally out of
//! scope for this slice (another agent owns `v1/`); the spec serves what
//! exists. As `#[utoipa::path]` annotations land, add them to `paths(...)`.

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::AppState;

/// Path the OpenAPI JSON is served at (and the URL Swagger UI fetches).
pub const OPENAPI_JSON_PATH: &str = "/api-docs/openapi.json";
/// Mount point for the interactive Swagger UI.
pub const SWAGGER_UI_PATH: &str = "/swagger-ui";

/// Aggregated OpenAPI document for the pharma-server HTTP API.
///
/// `paths` is currently empty (no handler is annotated yet); `components`
/// registers domain schemas so the document is non-trivial and downstream
/// tooling/codegen has types to bind against.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "pharma-server API",
        description = "On-prem pharmacy ERP server. Versioned HTTP/JSON API under /api/v1.",
        license(name = "Proprietary")
    ),
    paths(),
    components(schemas(domain::inventory::StockMovementDto))
)]
pub struct ApiDoc;

/// Build the Swagger UI + OpenAPI JSON sub-router, or an empty router when docs
/// are disabled. Merged into the top-level router in [`crate::build_router`].
///
/// When enabled this serves:
/// * `GET /swagger-ui` → 303 redirect to `/swagger-ui/` (then the UI),
/// * `GET /api-docs/openapi.json` → the [`ApiDoc`] document.
///
/// When disabled it returns `Router::new()`, so both paths 404.
pub fn router(state: &AppState) -> Router<AppState> {
    if state.docs_enabled {
        SwaggerUi::new(SWAGGER_UI_PATH)
            .url(OPENAPI_JSON_PATH, ApiDoc::openapi())
            .into()
    } else {
        Router::new()
    }
}
