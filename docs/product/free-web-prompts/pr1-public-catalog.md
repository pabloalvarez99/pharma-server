# PR1 — Public catalog (RutBusiness Free Web)

You are a principal engineer on **RutBusiness** (repo `pharma-server`): Rust ERP,
offline-first. This session ships the **public read-only catalog API**: a storefront
can pull a safe catalog when the tenant published their web. Execute fully: code →
tests → gates → push. No storefront UI. No orders. No API keys (that's PR2/PR3).

## Setup (PowerShell)

```powershell
cd "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server"
git fetch origin
git worktree add "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web" -b feature/free-web-shopify-parity origin/feature/erp-parity  # skip if exists; then just cd + git pull
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"
$env:CARGO_TARGET_DIR = "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server\target"  # reuse compile cache (sessions are sequential)
```

## Laws (fail = wrong product)

1. `web.published != "true"` ⇒ ALL public routes return plain 404 (darkness, no hint).
2. Never serialize `cost_price` or any cost/margin field on public DTOs.
3. Prices = decimal **strings** (`rust_decimal::serde::str`).
4. New tables/fields: migrations **append-only** (never edit applied `NNNN_*.surql`); tenant-scoped tables carry `tenant: record<tenant>` + composite index leading with tenant.
5. Error responses use existing envelope `{"error":{"code","message"(es),"details"}}` via `ApiError`.
6. Gate before push: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`.

## Verified repo facts (2026-07-20 — trust, don't re-derive)

- Stack: Rust ed2021, axum 0.8, SurrealDB 2.1 embedded (kv-surrealkv), rust_decimal, utoipa 5.
- Migrations run through `0017_dte.surql` → **new file is `migrations/0018_web_storefront.surql`** (confirm with `ls migrations | sort | tail -3`; bump if taken).
- No `public*` routes exist anywhere.
- `product` table (mig 0003): fields incl. `name, slug, description, price (decimal), cost_price, stock (int), category (option<record<category>>), image_url, active (bool), prescription_type (string, default 'direct'), presentation, discount_percent`. Index pattern: `DEFINE INDEX product_tenant_slug ON product FIELDS tenant, slug UNIQUE;`
- `category` table: `tenant, name, slug, active`.
- `admin_setting` table (mig 0007): tenant-scoped k/v `{tenant, key, value(string), updated_at}`, unique index (tenant,key). Existing endpoints `GET|PUT /api/v1/settings/{key}` (JWT) already read/write it — reuse for web settings, **no new admin endpoints needed**.
- Domain error type: `domain::DomainError` (`crates/domain/src/errors.rs`) → `NotFound` maps to 404. Service fns return `DomainResult<T>`.
- API error: `crates/api/src/error.rs` `ApiError` (`ApiError::not_found()` etc.).
- Router wiring: `crates/api/src/v1/mod.rs` — `pub mod x;` + `.merge(x::router(state.clone()))` chain inside `pub fn router(state: AppState)`. Copy `crates/api/src/v1/catalog.rs` handler style (`State(state)`, `db_of`, `Json`).
- **Public routes have NO JWT** — existing handlers use `AuthUser(claims)` extractor; public handlers must NOT. Check how routes get auth: if a global auth layer exists on `/api/v1`, mount the public router OUTSIDE it (inspect `crates/api/src/routes.rs` / `lib.rs` build_router to find the right merge point — public router must be reachable unauthenticated).
- Test harness to copy: `crates/api/tests/integration_db.rs` → helpers `spawn_test_db()` (tempdir + `db::run_migrations(&handle, "../../migrations")`), `seed_tenant_and_user`, `state_with_db(db) -> api::AppState`, then `api::build_router(state).oneshot(Request…)` with `tower::ServiceExt`. Reuse the same pattern in the new test file (copy helpers in; they're not exported).

## Tenant resolution (decided)

Public URL carries the tenant slug: `/api/v1/public/{slug}/…`. Resolve `tenant` record
by `tenant.slug` (tenant table from mig 0001; test seed creates `slug`). Then check
`admin_setting` `web.published == "true"` for that tenant; otherwise `DomainError::NotFound`.

## Files

- Create: `migrations/0018_web_storefront.surql`
- Modify: `crates/domain/src/catalog/model.rs` (public DTOs), `service.rs`, `repo.rs`
- Create: `crates/api/src/v1/public_web.rs`
- Modify: `crates/api/src/v1/mod.rs` (+ outer router file if public must mount outside auth layer)
- Create: `crates/api/tests/public_web_catalog.rs`

## Migration 0018 (exact)

```surql
-- pharma-server 0018: web storefront — product online projection fields (Free Web PR1)
DEFINE FIELD online_visible     ON product TYPE bool DEFAULT false;
DEFINE FIELD online_title       ON product TYPE option<string>;
DEFINE FIELD online_description ON product TYPE option<string>;
DEFINE FIELD online_sort        ON product TYPE int DEFAULT 0;
DEFINE FIELD online_price       ON product TYPE option<decimal>;
DEFINE INDEX product_tenant_online ON product FIELDS tenant, online_visible, active, online_sort;
```

Existing product rows: SCHEMAFULL + DEFAULT covers new reads; if repo SELECTs decode
`Option`-less bools, make DTO mapping tolerant (`#[serde(default)]` on raw row structs).

## Settings keys (admin_setting, written via existing PUT /settings/{key})

`web.published` ("true"/"false") · `web.store_name` · `web.whatsapp_e164` ·
`web.hours_label` · `web.address_line` · `web.pickup_instructions`

## Domain interface (Produces — PR3/PR5 depend on these exact names)

In `crates/domain/src/catalog/model.rs`:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicStoreDto {
    pub name: String,
    pub slug: String,            // tenant slug
    pub currency: String,        // "CLP"
    pub whatsapp_e164: Option<String>,
    pub address_line: Option<String>,
    pub hours_label: Option<String>,
    pub pickup_enabled: bool,    // true in PR1
    pub pickup_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicProductDto {
    pub id: String,              // "product:xyz"
    pub slug: String,
    pub name: String,            // online_title ?? name
    pub description_short: Option<String>, // online_description ?? description
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub price_clp: Decimal,      // online_price ?? price
    pub image_url: Option<String>,
    pub category_slug: Option<String>,
    pub availability: PublicAvailability, // from stock (PR3 subtracts stock_reserved)
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicAvailability { InStock, Low, OutOfStock }

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct PublicCatalogFilters {
    pub q: Option<String>,
    pub category: Option<String>, // category slug
    pub limit: Option<u32>,       // default 50, max 100
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicCatalogPage {
    pub store: PublicStoreDto,
    pub items: Vec<PublicProductDto>,
    pub next_offset: Option<u32>, // Some when a full page returned
}
```

In `service.rs` (signatures — PR3 reuses the first two):

```rust
pub async fn resolve_published_tenant(db: &db::Db, slug: &str) -> DomainResult<surrealdb::sql::Thing>; // NotFound unless tenant exists AND web.published=="true"
pub async fn public_store(db: &db::Db, tenant: &Thing, slug: &str) -> DomainResult<PublicStoreDto>;
pub async fn list_public_catalog(db: &db::Db, tenant: &Thing, slug: &str, f: PublicCatalogFilters) -> DomainResult<PublicCatalogPage>;
pub async fn get_public_product(db: &db::Db, tenant: &Thing, product_slug: &str) -> DomainResult<PublicProductDto>; // NotFound if !active || !online_visible
```

Availability thresholds: `stock <= 0 → OutOfStock`, `<= 5 → Low`, else `InStock`
(low threshold: read `admin_setting low_stock_threshold` if trivially available, else
constant 5 — `catalog::model::LOW_STOCK_DEFAULT` exists).

Pharmacy safety: run `rg -n "prescription_type" crates/domain/src --type rust | head -20`
to enumerate values; public catalog includes ONLY products whose `prescription_type`
is the over-the-counter/'direct' value — exclude everything else in the repo query.

## Routes (new file `crates/api/src/v1/public_web.rs`)

```
GET /api/v1/public/{slug}/store            → PublicStoreDto
GET /api/v1/public/{slug}/catalog?q&category&limit&offset → PublicCatalogPage
GET /api/v1/public/{slug}/catalog/{product_slug} → PublicProductDto
```

No auth extractor. Handlers: resolve_published_tenant first; `DomainError::NotFound`
already maps to 404 envelope. `pub fn router(state: AppState) -> Router<AppState>`
mounted so it bypasses any JWT layer (see facts above).

## Tests (`crates/api/tests/public_web_catalog.rs`) — copy harness from integration_db.rs

Seed: tenant "demo" + admin user; 3 products via direct db queries (or existing service):
A active+online_visible, B active only (not visible), C online_visible but `active=false`.
Set `admin_setting web.published/web.store_name` via db insert or the PUT endpoint with JWT.

1. `unpublished_returns_404_on_all_three_routes`
2. `published_catalog_lists_only_active_and_visible` (A only; B, C absent)
3. `public_json_has_no_cost_fields` — serialize page to `serde_json::Value`, assert no key contains `"cost"` anywhere (recursive walk).
4. `price_clp_is_string` (`items[0]["price_clp"].is_string()`)
5. `tenant_isolation` — second tenant published; its catalog never shows tenant A products.
6. `product_detail_by_slug_and_404_when_hidden` (A ok; B → 404)
7. If pharmacy exclusion applies: product with non-direct `prescription_type` absent.

## Gate + ship

```powershell
cargo fmt --all
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
git add -A
git commit -m "feat(web): PR1 public catalog — mig 0018, public DTOs, /api/v1/public/{slug} read routes"
git push -u origin feature/free-web-shopify-parity
gh pr create --base feature/erp-parity --title "feat(web): Free Web PR1 — public catalog seam" --body "Public read-only catalog behind web.published 404-darkness. Mig 0018. Next: PR2 API keys, PR3 pickup orders. Plan: docs/product/free-web-prompts/README.md"
```

(If a PR for this branch already exists, just push.) Red gate = fix before push; never weaken asserts.
Done → print `✅ PR1 LISTO — PR abierto · next: pr2-api-keys.md in fresh session`.
