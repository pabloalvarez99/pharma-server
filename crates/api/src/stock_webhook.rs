//! ERP→web stock-change push webhook (ADR-0013, Patrón B).
//!
//! When stock changes (POS sale, refund, PO receive, manual adjust) the ERP is
//! the canonical source of truth (ADR-0013 truth matrix). This module fires a
//! best-effort, HMAC-signed webhook at the web storefront so `tu-farmacia.cl`
//! reflects current stock within seconds, without a persistent connection.
//!
//! ## Non-blocking by construction
//!
//! [`notify`] spawns a detached `tokio` task and returns immediately. The POS
//! hot path never awaits delivery — the sale is already committed in the ERP;
//! the webhook is a side effect. A web outage cannot slow or fail a sale
//! (offline-first invariant, ADR-0005). Eventual consistency on the web side is
//! guaranteed by the nightly pull-catalog reconcile (ADR-0013), not by this
//! push.
//!
//! ## Delivery & retry
//!
//! Per ADR-0013: attempt once, then up to 3 retries on `5xx`/timeout/connect
//! errors with backoff `[1, 5, 30]` s (≈36 s max wall-clock). A `4xx` other
//! than 408/429 is a contract error (bad signature, malformed payload) → no
//! retry, log ERROR, drop. After exhausting retries → WARN + drop (no
//! persistence: avoids unbounded memory growth during long outages). Every
//! drop increments `pharma_stock_webhook_dropped_total{tenant,reason}` so
//! oncall can alert on a web storefront that stops accepting pushes.
//!
//! ## Pure core
//!
//! [`sign`], [`StockChangePayload`], and [`should_fire`] are pure and unit
//! tested without any network. [`deliver`] is the only part that does I/O.

use std::sync::Arc;
use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

pub use pharma_core::config::StockWebhookConfig;

type HmacSha256 = Hmac<Sha256>;

/// JSON body POSTed to the web endpoint. `schema_version` is pinned to `"1.0"`
/// (ADR-0013 § Contrato del payload); bumps require a sub-ADR.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StockChangePayload {
    pub schema_version: &'static str,
    /// Multi-tenant key — same format as the pull-catalog `?tenant=` slug.
    pub tenant_slug: String,
    /// Commercial SKU (`product.external_id`); the web keys on this.
    pub external_id: String,
    /// Stock **after** the movement. Never the delta (deltas aren't idempotent
    /// under retry).
    pub new_stock: i64,
    /// Convenience boolean for the web: `new_stock > 0`.
    pub in_stock: bool,
    /// RFC3339 timestamp of the movement that triggered the webhook. The web
    /// uses it to discard out-of-order events.
    pub ts: String,
    /// Idempotency key — UUID v7 (monotonic). The web persists it and skips
    /// duplicates.
    pub idempotency_key: String,
}

impl StockChangePayload {
    /// Build a payload for one product's post-movement stock. `ts` is the
    /// movement time (RFC3339); `idempotency_key` should be a fresh UUID v7.
    pub fn new(
        tenant_slug: impl Into<String>,
        external_id: impl Into<String>,
        new_stock: i64,
        ts: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "1.0",
            tenant_slug: tenant_slug.into(),
            external_id: external_id.into(),
            new_stock,
            in_stock: new_stock > 0,
            ts: ts.into(),
            idempotency_key: idempotency_key.into(),
        }
    }
}

/// Compute `HMAC-SHA256(body, secret)` and render it lowercase-hex. The header
/// value is `sha256=<hex>` (GitHub-webhook compatible); this returns the bare
/// hex so callers can format the prefix.
pub fn sign(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Whether a stock movement should fire a webhook. Pure predicate so the
/// gating rule is unit-testable. Fires only when the feature is enabled, a
/// target URL and secret are configured, the delta is non-zero, and the
/// product carries an `external_id` (the web keys on it — a SKU with no
/// external id can't be matched on the storefront).
pub fn should_fire(cfg: &StockWebhookConfig, delta: i64, external_id: Option<&str>) -> bool {
    cfg.enabled
        && !cfg.target_url.is_empty()
        && !cfg.hmac_secret.is_empty()
        && delta != 0
        && external_id.is_some_and(|s| !s.is_empty())
}

/// True when a product row is eligible for a stock webhook: the operator marked
/// it `publish_to_web` (ADR-0013 trigger filter) AND it carries a non-empty
/// commercial `external_id` (the storefront keys on it). Pure so the gate is
/// unit-tested without a DB.
pub fn publishable(external_id: Option<&str>, publish_to_web: bool) -> bool {
    publish_to_web && external_id.is_some_and(|s| !s.is_empty())
}

/// Spawn the delivery loop for already-built payloads — the only part that does
/// network I/O. Split out so every trigger path ([`notify_products`],
/// [`notify_po_receive`]) and any future synthetic caller reuse the exact
/// ADR-0013 retry policy. No-op (no task spawned, no runtime required) when the
/// webhook is disabled/unconfigured or there is nothing to send.
fn dispatch(cfg: Arc<StockWebhookConfig>, payloads: Vec<StockChangePayload>) {
    if !cfg.enabled || cfg.target_url.is_empty() || cfg.hmac_secret.is_empty() {
        return;
    }
    if payloads.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let client = match build_client() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "stock webhook: client build failed; dropping");
                return;
            }
        };
        for payload in payloads {
            deliver(&client, &cfg, &payload).await;
        }
    });
}

/// Fire-and-forget ERP→web push for products whose stock just changed
/// (ADR-0013 triggers `pos.sale`, `pos.refund`, `manual.adjust`). Reads each
/// product's *post-change* `stock`, `external_id`, and `publish_to_web`, then
/// pushes one HMAC-signed webhook per publishable SKU. Returns immediately and
/// never fails the caller's request (offline-first, ADR-0005). No-op when the
/// webhook is disabled/unconfigured or `product_ids` is empty.
pub fn notify_products(
    state: &crate::AppState,
    tenant: surrealdb::sql::Thing,
    product_ids: Vec<String>,
) {
    let cfg = state.stock_webhook.clone();
    if !cfg.enabled || cfg.target_url.is_empty() || cfg.hmac_secret.is_empty() {
        return; // common case: zero overhead
    }
    let Some(db) = state.db.clone() else {
        return;
    };
    if product_ids.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let Some(slug) = resolve_tenant_slug(db.as_ref(), &tenant).await else {
            tracing::warn!("stock webhook: tenant slug unresolved; dropping");
            return;
        };
        let payloads = match collect_payloads(db.as_ref(), &slug, &tenant, &product_ids).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "stock webhook: stock read failed; dropping");
                return;
            }
        };
        dispatch(cfg, payloads);
    });
}

/// PO-receive hook (ADR-0013 trigger `po.receive`). Receipt lines reference
/// `purchase_order_item` ids, not products; resolve them to product ids first,
/// then push their post-receive stock. Fire-and-forget like the others.
pub fn notify_po_receive(
    state: &crate::AppState,
    tenant: surrealdb::sql::Thing,
    po_line_ids: Vec<String>,
) {
    let cfg = state.stock_webhook.clone();
    if !cfg.enabled || cfg.target_url.is_empty() || cfg.hmac_secret.is_empty() {
        return;
    }
    let Some(db) = state.db.clone() else {
        return;
    };
    if po_line_ids.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let products = match resolve_line_products(db.as_ref(), &tenant, &po_line_ids).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "stock webhook: po-line resolve failed; dropping");
                return;
            }
        };
        if products.is_empty() {
            return;
        }
        let Some(slug) = resolve_tenant_slug(db.as_ref(), &tenant).await else {
            tracing::warn!("stock webhook: tenant slug unresolved; dropping");
            return;
        };
        let payloads = match collect_payloads(db.as_ref(), &slug, &tenant, &products).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "stock webhook: stock read failed; dropping");
                return;
            }
        };
        dispatch(cfg, payloads);
    });
}

/// Hook point for a committed POS sale (ADR-0013 trigger `pos.sale`).
///
/// Fire-and-forget: returns immediately and never touches the sale's result.
/// `sold` is `(product_record_id, quantity_sold)`; only the ids matter for the
/// push — the absolute post-sale stock is read fresh in [`collect_payloads`],
/// which is idempotent under retry (deltas are not). Thin wrapper over
/// [`notify_products`].
pub fn notify_sale(
    state: &crate::AppState,
    tenant: surrealdb::sql::Thing,
    sold: Vec<(String, i64)>,
) {
    notify_products(state, tenant, sold.into_iter().map(|(id, _)| id).collect());
}

/// Resolve `tenant.slug` for a tenant record id. `None` if missing.
async fn resolve_tenant_slug(db: &db::Db, tenant: &surrealdb::sql::Thing) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct R {
        slug: String,
    }
    let mut r = db
        .query("SELECT slug FROM tenant WHERE id = $t LIMIT 1")
        .bind(("t", tenant.clone()))
        .await
        .ok()?
        .check()
        .ok()?;
    let row: Option<R> = r.take(0).ok()?;
    row.map(|r| r.slug)
}

/// Resolve `purchase_order_item` ids to their distinct `product` record ids
/// (string form). Lines with no linked product (free-text buys) are skipped.
async fn resolve_line_products(
    db: &db::Db,
    tenant: &surrealdb::sql::Thing,
    po_line_ids: &[String],
) -> Result<Vec<String>, surrealdb::Error> {
    let things: Vec<surrealdb::sql::Thing> = po_line_ids
        .iter()
        .filter_map(|s| surrealdb::sql::thing(s).ok())
        .collect();
    if things.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct R {
        product: Option<surrealdb::sql::Thing>,
    }
    let rows: Vec<R> = db
        .query("SELECT product FROM purchase_order_item WHERE tenant = $t AND id IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", things))
        .await?
        .check()?
        .take(0)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.product.map(|p| p.to_string()))
        .collect())
}

/// Read post-change `(stock, external_id, publish_to_web)` for the products and
/// build one payload per publishable SKU (see [`publishable`]). `new_stock` is
/// the absolute current value — idempotent under webhook retry. Non-publishable
/// rows (internal SKUs, or `external_id` missing) are dropped silently.
async fn collect_payloads(
    db: &db::Db,
    tenant_slug: &str,
    tenant: &surrealdb::sql::Thing,
    product_ids: &[String],
) -> Result<Vec<StockChangePayload>, surrealdb::Error> {
    let things: Vec<surrealdb::sql::Thing> = product_ids
        .iter()
        .filter_map(|s| surrealdb::sql::thing(s).ok())
        .collect();
    if things.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct Row {
        stock: i64,
        external_id: Option<String>,
        #[serde(default)]
        publish_to_web: bool,
    }
    // Record-id fetch (`FROM $ids`) instead of `FROM product WHERE id IN $ids`,
    // which full-scans the catalog (O(SKUs)) per webhook batch — BUG-perf-006.
    // The `WHERE tenant = $t` guard stays: it drops any id that resolves to
    // another tenant's product, so a forged cross-tenant id can't leak stock.
    let rows: Vec<Row> = db
        .query(
            "SELECT stock, external_id, publish_to_web \
             FROM $ids WHERE tenant = $t",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", things))
        .await?
        .check()?
        .take(0)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            if !publishable(r.external_id.as_deref(), r.publish_to_web) {
                return None;
            }
            let ext = r.external_id?;
            let ts = chrono::Utc::now().to_rfc3339();
            let key = uuid::Uuid::now_v7().to_string();
            Some(StockChangePayload::new(tenant_slug, ext, r.stock, ts, key))
        })
        .collect())
}

fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
}

/// Increment `pharma_stock_webhook_dropped_total{tenant,reason}` (ADR-0013 §
/// "Política de retry"). Lets oncall alert when the web storefront stops
/// accepting pushes. Rendered through the global Prometheus recorder the api
/// installs for `/metrics`; `reason` is a bounded set so label cardinality
/// stays small (`external_id` is deliberately NOT a label).
fn record_drop(tenant_slug: &str, reason: &'static str) {
    metrics::counter!(
        "pharma_stock_webhook_dropped_total",
        "tenant" => tenant_slug.to_string(),
        "reason" => reason,
    )
    .increment(1);
}

/// Deliver one payload with the ADR-0013 retry policy. Best-effort: every exit
/// path is a log line, never an error propagated to the caller.
async fn deliver(client: &reqwest::Client, cfg: &StockWebhookConfig, payload: &StockChangePayload) {
    let body = match serde_json::to_vec(payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, sku = %payload.external_id,
                "stock webhook: payload serialize failed; dropping");
            return;
        }
    };
    let signature = format!("sha256={}", sign(cfg.hmac_secret.as_bytes(), &body));

    // attempt 0 is immediate; retries wait backoff_secs[i].
    let backoff = &cfg.retry.backoff_secs;
    let max_attempts = cfg.retry.tries as usize + 1;
    let mut last_status: Option<u16> = None;

    for attempt in 0..max_attempts {
        if attempt > 0 {
            let wait = backoff
                .get(attempt - 1)
                .or_else(|| backoff.last())
                .copied()
                .unwrap_or(0);
            if wait > 0 {
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
        }

        let req = client
            .post(&cfg.target_url)
            .header("content-type", "application/json")
            .header("x-pharma-signature", &signature)
            .header("x-pharma-timestamp", &payload.ts)
            .header("x-pharma-tenant", &payload.tenant_slug)
            .header("idempotency-key", &payload.idempotency_key)
            .body(body.clone());

        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                last_status = Some(status.as_u16());
                if status.is_success() {
                    tracing::debug!(sku = %payload.external_id, status = status.as_u16(),
                        attempt, "stock webhook delivered");
                    return;
                }
                // 4xx (except 408/429) is a contract error: never retried.
                let retriable_4xx = status == reqwest::StatusCode::REQUEST_TIMEOUT
                    || status == reqwest::StatusCode::TOO_MANY_REQUESTS;
                if status.is_client_error() && !retriable_4xx {
                    tracing::error!(sku = %payload.external_id, status = status.as_u16(),
                        "stock webhook: contract error (4xx); dropping without retry");
                    record_drop(&payload.tenant_slug, "contract_error_4xx");
                    return;
                }
                tracing::warn!(sku = %payload.external_id, status = status.as_u16(),
                    attempt, "stock webhook: non-2xx; will retry if attempts remain");
            }
            Err(e) => {
                tracing::warn!(error = %e, sku = %payload.external_id, attempt,
                    "stock webhook: send failed; will retry if attempts remain");
            }
        }
    }

    tracing::warn!(
        sku = %payload.external_id,
        idempotency_key = %payload.idempotency_key,
        last_status = ?last_status,
        attempts = max_attempts,
        "stock webhook giving up after retries; dropping (web reconciles via nightly pull)"
    );
    record_drop(&payload.tenant_slug, "retry_exhausted");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, url: &str, secret: &str) -> StockWebhookConfig {
        StockWebhookConfig {
            enabled,
            target_url: url.into(),
            hmac_secret: secret.into(),
            ..Default::default()
        }
    }

    #[test]
    fn sign_matches_known_vector() {
        // RFC-style fixed vector so any reimplementation (e.g. Node web side)
        // can be cross-checked. key="key", msg="The quick brown fox jumps over the lazy dog".
        let sig = sign(b"key", b"The quick brown fox jumps over the lazy dog");
        assert_eq!(
            sig,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn should_fire_gating() {
        let on = cfg(true, "https://x", "s");
        assert!(should_fire(&on, -1, Some("PARA-500")));
        assert!(should_fire(&on, 5, Some("PARA-500")));
        // delta 0 never fires
        assert!(!should_fire(&on, 0, Some("PARA-500")));
        // disabled never fires
        assert!(!should_fire(
            &cfg(false, "https://x", "s"),
            -1,
            Some("PARA-500")
        ));
        // empty url never fires
        assert!(!should_fire(&cfg(true, "", "s"), -1, Some("PARA-500")));
        // empty secret never fires
        assert!(!should_fire(
            &cfg(true, "https://x", ""),
            -1,
            Some("PARA-500")
        ));
        // no external_id never fires
        assert!(!should_fire(&on, -1, None));
        assert!(!should_fire(&on, -1, Some("")));
    }

    #[test]
    fn default_retry_schedule_is_1_5_30() {
        let c = StockWebhookConfig::default();
        assert_eq!(c.retry.tries, 3);
        assert_eq!(c.retry.backoff_secs, vec![1, 5, 30]);
    }

    #[test]
    fn payload_serializes_with_expected_shape() {
        let p = StockChangePayload::new(
            "coquimbo-centro",
            "PARA-500-20",
            42,
            "2026-05-24T15:34:12+00:00",
            "01J0K5R8X2",
        );
        let v: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(v["schema_version"], "1.0");
        assert_eq!(v["tenant_slug"], "coquimbo-centro");
        assert_eq!(v["external_id"], "PARA-500-20");
        assert_eq!(v["new_stock"], 42);
        assert_eq!(v["in_stock"], true);
        assert_eq!(v["ts"], "2026-05-24T15:34:12+00:00");
        assert_eq!(v["idempotency_key"], "01J0K5R8X2");
    }

    #[test]
    fn in_stock_false_when_zero() {
        let p = StockChangePayload::new("t", "SKU", 0, "2026-05-24T00:00:00+00:00", "k");
        assert!(!p.in_stock);
    }

    #[test]
    fn dispatch_disabled_is_noop() {
        // No tokio runtime needed: disabled config short-circuits before spawn.
        let c = Arc::new(cfg(false, "https://nope.invalid", "s"));
        let p = StockChangePayload::new("t", "SKU", 1, "2026-05-24T00:00:00+00:00", "k");
        dispatch(c, vec![p]);
    }

    #[test]
    fn publishable_gate() {
        // Must be flagged publishable AND carry a non-empty external_id.
        assert!(publishable(Some("PARA-500"), true));
        assert!(!publishable(Some("PARA-500"), false)); // internal SKU
        assert!(!publishable(None, true)); // no commercial id
        assert!(!publishable(Some(""), true)); // empty id
        assert!(!publishable(None, false));
    }

    /// `collect_payloads` resolves the passed record-ids directly (`FROM $ids`,
    /// BUG-perf-006) and the `WHERE tenant = $t` guard drops any id that belongs
    /// to another tenant — a forged cross-tenant product id can't leak stock.
    #[tokio::test]
    async fn collect_payloads_resolves_ids_and_blocks_cross_tenant() {
        use surrealdb::engine::local::Mem;
        use surrealdb::Surreal;

        let db: db::Db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("t").use_db("t").await.unwrap();
        db::run_embedded(&db).await.unwrap();

        let new_tenant = |slug: &str| {
            let db = &db;
            let slug = slug.to_string();
            async move {
                let mut r = db
                    .query("CREATE tenant SET name=$n, slug=$s RETURN id")
                    .bind(("n", slug.clone()))
                    .bind(("s", slug))
                    .await
                    .unwrap();
                r.take::<Option<surrealdb::sql::Thing>>((0, "id"))
                    .unwrap()
                    .unwrap()
            }
        };
        let tenant_a = new_tenant("farmacia-a").await;
        let tenant_b = new_tenant("farmacia-b").await;

        let new_product = |tenant: &surrealdb::sql::Thing, slug: &str, sku: &str, stock: i64| {
            let db = &db;
            let (tenant, slug, sku) = (tenant.clone(), slug.to_string(), sku.to_string());
            async move {
                let mut r = db
                    .query(
                        "CREATE product SET tenant=$t, name=$s, slug=$s, price=1000dec, \
                         stock=$st, external_id=$x, publish_to_web=true RETURN id",
                    )
                    .bind(("t", tenant))
                    .bind(("s", slug))
                    .bind(("x", sku))
                    .bind(("st", stock))
                    .await
                    .unwrap();
                r.take::<Option<surrealdb::sql::Thing>>((0, "id"))
                    .unwrap()
                    .unwrap()
                    .to_string()
            }
        };
        let prod_a = new_product(&tenant_a, "para-500", "SKU-A", 42).await;
        let prod_b = new_product(&tenant_b, "ibu-400", "SKU-B", 7).await;

        // Ask as tenant A, passing BOTH ids. B's id must be dropped by the guard.
        let payloads = collect_payloads(&db, "farmacia-a", &tenant_a, &[prod_a, prod_b])
            .await
            .unwrap();

        assert_eq!(payloads.len(), 1, "only tenant A's product should resolve");
        assert_eq!(payloads[0].external_id, "SKU-A");
        assert_eq!(payloads[0].new_stock, 42);
    }

    #[test]
    fn record_drop_increments_labelled_counter() {
        use metrics_util::debugging::{DebugValue, DebuggingRecorder};

        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            record_drop("coquimbo-centro", "retry_exhausted");
            record_drop("coquimbo-centro", "retry_exhausted");
        });

        let hit = snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .find(|(ck, _, _, _)| ck.key().name() == "pharma_stock_webhook_dropped_total");
        let (ck, _, _, value) = hit.expect("dropped counter recorded");
        assert!(matches!(value, DebugValue::Counter(2)), "got {value:?}");
        // Labels carry tenant + reason for alerting.
        let labels: Vec<_> = ck.key().labels().map(|l| (l.key(), l.value())).collect();
        assert!(labels.contains(&("tenant", "coquimbo-centro")));
        assert!(labels.contains(&("reason", "retry_exhausted")));
    }
}
