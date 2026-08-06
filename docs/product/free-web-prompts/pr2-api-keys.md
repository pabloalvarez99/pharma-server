# PR2 — API keys + public auth middleware (RutBusiness Free Web)

You are a principal engineer on **RutBusiness** (repo `pharma-server`): Rust ERP,
offline-first. PR1 (public catalog, merged/pushed on this lane) exposed keyless
read routes. This session ships the **iron door**: `web_api_key` table, admin CRUD,
and bearer-key middleware that PR3 (web orders) will mount on write routes.
Execute fully: code → tests → gates → push.

## Setup (PowerShell)

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"   # created in PR0/PR1
git pull
$env:CARGO_TARGET_DIR = "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server\target"
```

(If worktree missing: `cd ...\01-product\rutbusiness\pharma-server; git fetch origin; git worktree add "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web" feature/free-web-shopify-parity`.)

## Laws

1. Plaintext key + HMAC secret returned **exactly once** at create/rotate; DB stores only SHA-256 hash of the key (key has 256-bit entropy → plain SHA-256 is correct here, NOT argon2 — verification runs per-request).
2. Keys never appear in logs or error messages.
3. Migrations append-only; tenant field + tenant-leading index on new tables.
4. Errors via existing `ApiError` envelope, Spanish messages, SCREAMING_SNAKE codes.
5. Gate: `cargo fmt --all -- --check` + `clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`.

## Verified repo facts (2026-07-20 + PR1 outputs)

- Next migration number: check `ls migrations | sort | tail -3` — expect **0019** (`0019_web_api_key.surql`).
- Admin-authed handler pattern: copy `crates/api/src/v1/sales.rs` — `AuthUser(claims)`, `tenant_of(&claims)`, role layer `crate::role::layer(state, &["admin","owner"])`.
- Middleware dir: `crates/api/src/middleware/` (has `auth.rs`, `role.rs`, `audit.rs`; registered in `middleware/mod.rs`).
- Router wiring: `crates/api/src/v1/mod.rs` merge chain.
- PR1 produced: `domain::catalog::service::resolve_published_tenant(db, slug) -> DomainResult<Thing>`; public routes `/api/v1/public/{slug}/…` mounted without JWT.
- `sha2`/`hmac` crates: check `rg -n "sha2|hmac" Cargo.toml crates/*/Cargo.toml`. If absent add to workspace deps: `sha2 = "0.10"`, `hmac = "0.12"`, `hex = "0.4"` (hmac+hex used in PR3; adding now keeps one dep commit). Key generation randomness: use `uuid::Uuid::new_v4()` twice → 64 hex chars (uuid 1.x already a dep).
- Test harness: copy helpers from `crates/api/tests/integration_db.rs` (`spawn_test_db`, `seed_tenant_and_user`, `state_with_db`); JWT login flow visible in `crates/api/tests/auth.rs`.

## Migration 0019 (exact)

```surql
-- pharma-server 0019: web_api_key — storefront credentials (Free Web PR2)
DEFINE TABLE web_api_key SCHEMAFULL;
DEFINE FIELD tenant       ON web_api_key TYPE record<tenant>;
DEFINE FIELD name         ON web_api_key TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD key_prefix   ON web_api_key TYPE string;   -- first 12 chars incl "rb_live_", for listing
DEFINE FIELD key_hash     ON web_api_key TYPE string;   -- hex(sha256(full key))
DEFINE FIELD hmac_secret  ON web_api_key TYPE string;   -- server verifies request signatures (PR3)
DEFINE FIELD scopes       ON web_api_key TYPE array<string> DEFAULT ['catalog:read','orders:write'];
DEFINE FIELD active       ON web_api_key TYPE bool DEFAULT true;
DEFINE FIELD created_at   ON web_api_key TYPE datetime DEFAULT time::now();
DEFINE FIELD last_used_at ON web_api_key TYPE option<datetime>;
DEFINE INDEX web_api_key_tenant      ON web_api_key FIELDS tenant, active;
DEFINE INDEX web_api_key_hash_unique ON web_api_key FIELDS key_hash UNIQUE;
```

## Key format

`rb_live_` + 64 lowercase hex chars (two v4 UUIDs, `simple()` concatenated).
`key_prefix` = first 12 chars of the full string. HMAC secret: same generation,
prefix `whsec_`.

## Produces (PR3 depends on these exact names)

New file `crates/api/src/middleware/public_auth.rs`:

```rust
/// Verified storefront credential, injected by the public-key layer.
#[derive(Clone, Debug)]
pub struct WebApiKeyCtx {
    pub tenant: surrealdb::sql::Thing,
    pub key_id: surrealdb::sql::Thing,
    pub scopes: Vec<String>,
    pub hmac_secret: String,
}

/// Extractor: reads Authorization Bearer rb_live_…, sha256-lookup in web_api_key,
/// rejects 401 INVALID_API_KEY ("Credencial de storefront inválida.") when missing/
/// unknown/inactive. Also fire-and-forget UPDATE last_used_at.
pub struct RequireApiKey(pub WebApiKeyCtx);   // axum FromRequestParts<AppState>

/// Scope check helper: 403 SCOPE_DENIED ("Alcance no autorizado.") when scope absent.
pub fn require_scope(ctx: &WebApiKeyCtx, scope: &str) -> Result<(), crate::error::ApiError>;
```

Domain-side key logic can live in `crates/domain/src/web_keys.rs` (create + list +
revoke + rotate + `find_by_hash`) — keep repo queries there, api layer thin.

## Admin routes (JWT + role admin/owner) — new file `crates/api/src/v1/admin_web.rs`

```
POST   /api/v1/admin/web/keys              {name, scopes?} → 201 {id, name, key, hmac_secret, key_prefix, scopes}  // plaintext ONCE
GET    /api/v1/admin/web/keys              → [{id, name, key_prefix, scopes, active, created_at, last_used_at}]    // no hash/secret
POST   /api/v1/admin/web/keys/{id}/rotate  → 200 same shape as create (old key inactive immediately)
DELETE /api/v1/admin/web/keys/{id}         → 204 (sets active=false)
```

Wire into `v1/mod.rs`. Web settings need NO new endpoints (existing `PUT /api/v1/settings/{key}` handles `web.published` etc.).

## Tests (`crates/api/tests/public_web_auth.rs`)

1. `create_key_returns_plaintext_once_and_list_hides_it`
2. `unknown_or_inactive_key_401_INVALID_API_KEY` (use extractor on a tiny test route, or via PR3 order route if trivial — else mount a `#[cfg(test)]`-style probe route is NOT needed: test the extractor through admin rotate + a direct call helper)
3. `revoked_key_401` · `rotate_invalidates_old_key`
4. `scope_denied_403` (`require_scope` unit test acceptable)
5. `key_of_tenant_a_cannot_be_bound_to_tenant_b` — extractor returns ctx.tenant = A; when path slug resolves tenant B, handler must 404/403 (write the check as a helper `ensure_key_matches_tenant(ctx, tenant) -> 403 SCOPE_DENIED`; PR3 uses it).

## Gate + ship

```powershell
cargo fmt --all
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
git add -A
git commit -m "feat(web): PR2 api keys — mig 0019 web_api_key, RequireApiKey middleware, admin keys CRUD"
git push
```

PR already open on this branch — push is enough. Red gate = fix first.
Done → print `✅ PR2 LISTO — pushed · next: pr3-web-orders.md in fresh session`.
