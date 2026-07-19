# Pre-Production Security Review — pharma-server

**Date**: 2026-05-24 · **Reviewer**: automated security pass (read-only) · **Version**: workspace 0.1.24
**Scope**: attack surface added this session across ~22 feature branches — Auth/JWT, multi-tenant
isolation, rate-limiting, admin role gates, public catalog (unauthed), bulk imports (customers /
historic orders), DTE/SII cert encrypt-at-rest, SurrealQL injection, CORS.

> **Branch note (important for the gate)**: several reviewed features are **not yet merged** into
> `feature/erp-parity`. They were read on their origin branches:
> `feat/api-rate-limit-v2`, `feat/api-public-catalog`, `feat/api-import-customers`,
> `feat/api-import-historic-orders`, `feat/api-audit-log-query-v2`, `feat/dte-9-1-i-cert-encrypt`,
> `feat/order-receipt`. File:line references below point at the branch where the code currently
> lives. The **integration wiring bug (F1)** must be re-verified after these branches are merged,
> because it is a property of `crates/api/src/lib.rs::build_router`, which differs per branch.

---

## Resumen ejecutivo (ES)

La superficie nueva está, en general, **bien construida**: aislamiento multi-tenant correcto (el
`tenant` siempre sale del claim JWT, nunca del request), el cifrado del certificado DTE es de
manual (AES-256-GCM + Argon2id, passphrase nunca persistida), las queries usan parámetros `$`
ligados (sin inyección explotable), el catálogo público está bien escudado (opt-in, 404 uniforme,
proyección recortada, CORS allow-list) y el fix del bug `AllowedRoles` es correcto.

**Hay un hallazgo que BLOQUEA producción**: el rate-limiting **no está conectado al router global**.
Los limitadores per-IP y per-tenant se construyen en `AppState` pero **nunca se aplican como capa**;
solo el sub-router del catálogo público los usa, y aun ahí no puede obtener la IP real porque el
servidor no usa `into_make_service_with_connect_info`. Resultado: **`/api/v1/login` y `/agent/inbox`
no tienen ninguna protección contra fuerza bruta / credential stuffing / DoS**, contradiciendo la
documentación del propio módulo. Además, el login no tiene lockout por cuenta.

El segundo must-fix es operacional: el `jwt.secret` por defecto es el placeholder
`change-me-in-production` y **el binario arranca sin quejarse** si no se inyecta
`PHARMA__JWT__SECRET`. En una farmacia real eso significa que un JWT puede forjarse con un secreto
público.

---

## Findings

| # | Severity | Location (file:line) | Issue | Recommendation |
|---|----------|----------------------|-------|----------------|
| **F1** | **Critical** | `crates/api/src/lib.rs:48-58` (`build_router`, on `feat/api-rate-limit-v2` and `feat/api-public-catalog`) | **Rate-limit middleware is never wired into the global router.** `build_router` only applies `audit::layer`. `rate_limit::ip_layer` / `tenant_layer` are *constructed* (`lib.rs:177` builds `RateLimitState`) but never attached. The per-IP limiter is applied **only** as a `route_layer` inside `public_catalog::router` (`v1/public_catalog.rs:84`), so it covers exactly one route. Login (`routes.rs:53`), `/agent/inbox`, and every authed mutation have **zero throttling**. The module doc (`middleware/rate_limit.rs:7-13`) explicitly claims it "Protects unauth endpoints (`/api/v1/login`, `/agent/inbox`) against credential-stuffing / brute force" — it does not. | Apply both layers in `build_router`: outermost `rate_limit::ip_layer(state.clone())`, then `tenant_layer`, then existing `audit::layer`. Add an integration test that fires N+1 logins from one IP and asserts a `429`. |
| **F2** | **High** | `crates/api/src/lib.rs:221` (`axum::serve(listener, app)`) | **Client socket IP is never available**, so even the one route that *does* apply `ip_layer` can only rate-limit on `X-Forwarded-For` (`middleware/rate_limit.rs:90-99`), which is **client-spoofable**. There is no `into_make_service_with_connect_info::<SocketAddr>()` anywhere in the tree (only referenced in a doc comment). On a LAN/on-prem deploy with no trusted reverse proxy setting XFF, the per-IP limiter is a permanent no-op (`client_ip()` returns `None` → request passes). | Serve with `app.into_make_service_with_connect_info::<SocketAddr>()`. Only trust `X-Forwarded-For` when a known proxy is configured; otherwise prefer the socket peer IP. Pairs with F1 — fixing F1 without F2 still leaves IP throttling ineffective. |
| **F3** | **High** | `config/default.toml:9-11`; no guard in `crates/api/src/lib.rs` startup | **Default JWT secret is the placeholder `change-me-in-production` and the server boots silently with it.** `auth::issue/verify` (`crates/auth/src/lib.rs:33,41`) sign/verify HS256 with `cfg.secret`. If `PHARMA__JWT__SECRET` is not injected in prod, tokens are forgeable with a publicly-known secret → full auth bypass + cross-tenant access (attacker mints any `tenant_id`/`roles`). No length/placeholder check fires. | At startup, **refuse to boot** (or hard-error `/health/ready`) if `jwt.secret` equals the known placeholder, is empty, or is < ~32 bytes. Document `PHARMA__JWT__SECRET` as a required prod env in the install runbook. |
| **F4** | **Low** | `crates/auth/src/lib.rs:38-42` | JWT validation: `exp` is enforced (jsonwebtoken default) and `iss` is set explicitly (good). No `aud` claim is validated and there is no `nbf`/leeway tightening. HS256 is symmetric so there is **no** alg-confusion-to-RS256 risk here. Token revocation: a `session` row with `jti` is written at login (`routes.rs:127`) but `verify()` never checks it — logout/forced-revocation is not possible before `exp`. | Acceptable for go-live. Post-launch: add `aud`, and (if revocation matters) check the `session.jti` is still present/active during verify, or shorten `ttl_seconds`. |
| **F5** | **Low** | `crates/domain/src/catalog/repo.rs:485` + `catalog/service.rs:256-279` | `bulk_update_price` builds an UPDATE via `format!("... SET price = {expr} ...")`. **Not exploitable**: `expr` is assembled only from `BulkPrice.value` (a `rust_decimal::Decimal`, parsed strict via `rust_decimal::serde::str`, `model.rs:161-163`) and a closed `BulkPriceMode` enum; a `Decimal` cannot carry SurrealQL syntax. The `WHERE` `cond` is static literals. It is the **only** place a value is interpolated rather than `$`-bound. | Defense-in-depth: bind the decimal as `$delta` and keep the arithmetic in the query template, to preserve the absolute "always bind user data" invariant and avoid a future regression if `value`'s type ever loosens. |
| **F6** | **Low / Info** | `crates/api/src/middleware/audit.rs:58-59` | Audit attribution captures client IP **only** from `x-forwarded-for` / `x-real-ip` headers — spoofable, and `None` when no proxy is present. The audit row's `ip` field can therefore be forged or empty. Body is SHA-256 hashed (`payload_hash`), so login passwords are **not** stored in cleartext (good). | After F2, also feed the trusted socket IP into the audit row. Until then, treat `audit_log.ip` as advisory, not forensic. |

### Positives verified (no action required)

- **Multi-tenant isolation is sound.** Every new domain handler derives the tenant from the JWT
  claim via `tenant_of(&claims)`/`surrealdb::sql::thing(&claims.tenant_id)` and passes it as a bound
  `$tenant`/`$t` param — never from request body or query string. Checked: `customers.rs`,
  `audit.rs` ("Tenant filter ALWAYS comes from the JWT claim" — `v1/audit.rs:118`),
  `sales.rs` historic import, `purchasing.rs` receive, `order-receipt`, `public_catalog.rs`.
- **Public catalog (unauthed) is well-scoped.** Opt-in (`enabled=false` default → uniform 404,
  `v1/public_catalog.rs:118`), no tenant enumeration (unknown slug = same 404), scrubbed projection
  (`PublicProductDto` exposes only `external_id/name/price/in_stock/active_ingredient` — never
  `id`, `cost_price`, `stock` count, `discount`), `active=true` filter, and CORS locked to
  `cors_origins` default `["https://tu-farmacia.cl"]` (`core/src/config.rs`). Tenant slug + search
  term are `$`-bound; only `LIMIT/START` are interpolated from clamped `u32` (`query_products`).
- **SurrealQL injection: no exploitable vector found.** `format!`-built queries
  (`audit.rs`, `expenses/service.rs:497,631`, `historic.rs` order-item loop) interpolate **only**
  static `&'static str` condition fragments or loop indices (`$p{i}`); all user values are `$`-bound.
  The one Decimal interpolation (F5) is not injectable.
- **`AllowedRoles` role-gate bug is fixed correctly.** `feat/api-import-customers`
  `middleware/role.rs` uses `Stack::new(from_fn(role_gate), Extension(AllowedRoles))` — outer
  `Extension` runs first and attaches the extension before the gate reads it (load-bearing comment
  + `role_extension.rs` regression test). Admin endpoints (`import-customers`, `import-historic-orders`,
  customer/sales mutations, PO receive) are gated by `WRITE_ROLES = ["admin","owner"]` via
  `route_layer`; audit-log additionally gates in-handler (`require_admin`). Gate logic is a correct
  set intersection (`roles.iter().any(|r| allowed.contains(...))`).
- **DTE cert encrypt-at-rest is correct.** `crates/dte/src/cert.rs`: AES-256-GCM, fresh
  `OsRng` salt(16)+nonce(12) per encrypt, Argon2id KDF (64 MiB / t=3 / p=1), AES key in `Zeroizing`,
  plaintext returned as `Zeroizing<Vec<u8>>`, AAD `b"pharma-dte-cert-v1"`. **Passphrase is never
  persisted** (the legacy-named `password_encrypted` column holds only public KDF params — documented
  at `cert.rs` header). GCM tag mismatch fails loud with no padding-oracle leak; PFX/passphrase never
  logged. Tenant-scoped queries, bound params.
- **Historic-orders import** is admin/owner-gated, batch-capped (`MAX_BATCH_SIZE = 100`), resolves
  products tenant-scoped, and its deliberate skips (no stock decrement, no `stock_movement`, no
  idempotency, no min-price check) are documented and appropriate for a one-time migration tool.
- **No hardcoded production secrets** in committed Rust/TOML across reviewed branches. Only
  intentional placeholders (`change-me-in-production`, `test-secret`, license pubkey placeholder).
  `/metrics` is protected by a constant-time bearer compare (`lib.rs:authorize_metrics` +
  `constant_time_eq`).
- **`/agent/inbox`** is unauthenticated **by design** — authenticity is the Ed25519 envelope
  signature (`v1/agent.rs:78-80`). This makes F1 a real DoS concern (signature verification is CPU
  work an unthrottled caller can spam), reinforcing the need to wire the per-IP limiter.

---

## Go-live gate

### BLOCKS production (must fix before go-live)

- **F1 — rate-limiting not wired (Critical).** A real pharmacy on a LAN/internet-reachable host
  with an unthrottled `/api/v1/login` is exposed to offline-grade credential brute force and to
  DoS on `/agent/inbox`. The feature exists and is configured-on by default but is inert. Wiring it
  in `build_router` is a small, low-risk change. **Hard blocker.**
- **F3 — placeholder JWT secret boots silently (High → blocker for this deployment).** If the
  operator forgets `PHARMA__JWT__SECRET`, the install runs with a world-known signing key =
  complete auth bypass and cross-tenant compromise. Mitigation is trivial (refuse-to-boot guard).
  Treat as a blocker because the failure mode is silent and catastrophic, and this is a first
  production cutover.

### Acceptable with mitigation (should fix, not a hard blocker)

- **F2 — socket IP unavailable (High).** Strictly, F1 cannot be *fully* effective without F2, so in
  practice F2 should be fixed **together with F1**. If a trusted reverse proxy that sets a
  non-spoofable `X-Forwarded-For` fronts the server, per-IP limiting works without the code change —
  but on a bare LAN deploy it does not. Recommend fixing alongside F1.
- **F4 — no `aud`, no token revocation (Low).** Acceptable with the current short `ttl_seconds`
  (3600). Revisit post-launch if forced logout / token revocation becomes a requirement.
- **F5 — Decimal interpolation in bulk reprice (Low).** Not exploitable today; fix as
  defense-in-depth in a follow-up.
- **F6 — audit IP spoofable (Low/Info).** Audit log remains useful (user/tenant/method/path/hash);
  only the `ip` field is unreliable until F2 lands.

### Operational reminders (not code findings)

- Never run production with demo seed credentials / demo JWT secret (the session's demo tooling
  lives in untracked `scripts/` / `demo-data/` — keep it out of prod).
- Set `PHARMA__METRICS__TOKEN` in prod (empty ⇒ `/metrics` returns 401, which is the safe default).
- Public catalog: leave `public_catalog.enabled = false` unless the Tu Farmacia website integration
  is actually deployed, and confirm `cors_origins` matches the real frontend origin.

---

*Read-only review. No application code was modified. The only file added is this document.*
