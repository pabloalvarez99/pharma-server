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

/// One stock change to deliver: a built payload plus the delta that produced
/// it (used by [`should_fire`] before enqueueing).
#[derive(Debug, Clone)]
pub struct StockChange {
    pub external_id: String,
    pub new_stock: i64,
    pub delta: i64,
}

/// Fire-and-forget. Spawns a detached task that signs and POSTs one webhook per
/// change, with the ADR-0013 retry policy. Returns immediately — never blocks
/// the caller (the POS hot path). A no-op clone of the config that is disabled
/// or lacks a target/secret costs one boolean check and returns at once.
pub fn notify(cfg: Arc<StockWebhookConfig>, tenant_slug: String, changes: Vec<StockChange>) {
    if !cfg.enabled || cfg.target_url.is_empty() || cfg.hmac_secret.is_empty() {
        return;
    }
    let to_send: Vec<StockChangePayload> = changes
        .into_iter()
        .filter(|c| should_fire(&cfg, c.delta, Some(c.external_id.as_str())))
        .map(|c| {
            let ts = chrono::Utc::now().to_rfc3339();
            let key = uuid::Uuid::now_v7().to_string();
            StockChangePayload::new(&tenant_slug, c.external_id, c.new_stock, ts, key)
        })
        .collect();
    if to_send.is_empty() {
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
        for payload in to_send {
            deliver(&client, &cfg, &payload).await;
        }
    });
}

/// Hook point for a committed POS sale (ADR-0013 trigger `pos.sale`).
///
/// Fire-and-forget: returns immediately and never touches the sale's result.
/// When the webhook is disabled/unconfigured this costs one boolean check.
/// Otherwise it spawns a task that resolves the tenant slug, reads each sold
/// product's *post-sale* `stock` + `external_id` (the sale already committed,
/// so this reflects the new value), and dispatches one webhook per SKU.
///
/// `sold` is `(product_record_id, quantity_sold)`. The delta reported is
/// `-quantity` (a sale decrements). SKUs with no `external_id` are skipped.
pub fn notify_sale(
    state: &crate::AppState,
    tenant: surrealdb::sql::Thing,
    sold: Vec<(String, i64)>,
) {
    let cfg = state.stock_webhook.clone();
    if !cfg.enabled || cfg.target_url.is_empty() || cfg.hmac_secret.is_empty() {
        return; // common case: zero overhead
    }
    let Some(db) = state.db.clone() else {
        return;
    };
    if sold.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let Some(slug) = resolve_tenant_slug(db.as_ref(), &tenant).await else {
            tracing::warn!("stock webhook: tenant slug unresolved; dropping");
            return;
        };
        let changes = match collect_changes(db.as_ref(), &tenant, &sold).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "stock webhook: stock read failed; dropping");
                return;
            }
        };
        notify(cfg, slug, changes);
    });
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

/// Read post-sale `(external_id, stock)` for the sold products and pair each
/// with the line's delta (`-quantity`). Products with no `external_id` are
/// dropped (the web keys on it).
async fn collect_changes(
    db: &db::Db,
    tenant: &surrealdb::sql::Thing,
    sold: &[(String, i64)],
) -> Result<Vec<StockChange>, surrealdb::Error> {
    use std::collections::HashMap;
    let things: Vec<surrealdb::sql::Thing> = sold
        .iter()
        .filter_map(|(id, _)| surrealdb::sql::thing(id).ok())
        .collect();
    if things.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
        stock: i64,
        external_id: Option<String>,
    }
    let rows: Vec<Row> = db
        .query("SELECT id, stock, external_id FROM product WHERE tenant = $t AND id IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", things))
        .await?
        .check()?
        .take(0)?;
    // qty per product id (a cart may list the same SKU twice; sum the deltas).
    let mut qty: HashMap<String, i64> = HashMap::new();
    for (id, q) in sold {
        if let Ok(t) = surrealdb::sql::thing(id) {
            *qty.entry(t.to_string()).or_default() += *q;
        }
    }
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let ext = r.external_id.filter(|s| !s.is_empty())?;
            let q = qty.get(&r.id.to_string()).copied().unwrap_or(0);
            (q != 0).then_some(StockChange {
                external_id: ext,
                new_stock: r.stock,
                delta: -q,
            })
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
    fn notify_disabled_is_noop() {
        // No tokio runtime needed: disabled config short-circuits before spawn.
        let c = Arc::new(cfg(false, "https://nope.invalid", "s"));
        notify(
            c,
            "t".into(),
            vec![StockChange {
                external_id: "SKU".into(),
                new_stock: 1,
                delta: -1,
            }],
        );
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
