//! Near-expiry alert scan — nightly digest backend.
//!
//! A pure-logic pass that finds product batches (lotes) on hand that expire
//! within `within_days`, grouped per tenant, and emits a structured `WARN`
//! tracing digest per tenant **and sucursal** (migración 0042: el lote tiene
//! domicilio). This is the *digest backend* only: real
//! notification channels (Telegram/email) are layered on later — here we just
//! compute the alerts and log them.
//!
//! The query shape is deliberately identical to the existing
//! `GET /api/v1/reports/near-expiry` report
//! (`crates/domain/src/expenses/service.rs::near_expiry`): tenant-scoped,
//! only `active = true` batches with `stock > 0` and
//! `expiry_date <= now + within_days` (so already-expired lots, with negative
//! `days_to_expiry`, are included). Sorted by `days_to_expiry` ascending — most
//! urgent first.
//!
//! Unlike the API report, this job scans **all tenants** in one pass (it is not
//! driven by a JWT tenant claim) and resolves nothing beyond the batch row — it
//! is meant for an unattended cron, not a user request.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::Thing;
use surrealdb::{Connection, Surreal};

use crate::JobError;

/// One soon-to-expire (or already-expired) batch with on-hand stock, flattened
/// for the alert digest. `days_to_expiry` is negative when the lot is already
/// past due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearExpiryAlert {
    /// Owning tenant record id (string form, e.g. `tenant:t1`).
    pub tenant: String,
    /// Product record id (string form). The schema has no free-standing SKU on
    /// `product_batch`; the product link is the stable per-tenant identifier.
    pub sku: String,
    /// Batch code (lot number).
    pub lot: String,
    /// Sucursal donde está físicamente el lote (migración 0042), en forma de
    /// string (`branch:<key>`). `None` = casa matriz. El job no resuelve el
    /// nombre: es un cron desatendido, el nombre bonito lo pone el reporte de
    /// API que ya hace lookup batcheado.
    pub branch: Option<String>,
    /// Expiry date (UTC).
    pub expiry_date: DateTime<Utc>,
    /// Whole days from today (UTC) until expiry. Negative = already expired.
    pub days_to_expiry: i64,
    /// On-hand units in this lot (always > 0 here).
    pub stock: i64,
}

/// Scan every tenant for batches expiring within `within_days` and return the
/// alerts sorted by `days_to_expiry` ascending (most urgent first; already
/// expired lots — negative days — come first). Emits one `tracing::warn!`
/// digest line per (tenant, sucursal) that has at least one alert — la alerta
/// dice a QUÉ LOCAL hay que ir.
///
/// Semantics mirror the API near-expiry report exactly:
/// `active = true AND stock > 0 AND expiry_date <= now + within_days`,
/// tenant-scoped. `within_days` is clamped to `0..=3650` (matching the report's
/// guard) so a stray huge/negative window can't pull the whole catalog or
/// invert the cutoff.
///
/// Empty database (or no matching lots) → `Ok(vec![])`, no warning emitted.
pub async fn run_near_expiry_scan<C: Connection>(
    db: &Surreal<C>,
    within_days: i64,
) -> Result<Vec<NearExpiryAlert>, JobError> {
    let within = within_days.clamp(0, 3650);
    let now = Utc::now();
    let cutoff = now + chrono::Duration::days(within);

    #[derive(Deserialize)]
    struct Row {
        tenant: Thing,
        product: Thing,
        #[serde(default)]
        branch: Option<Thing>,
        batch_code: String,
        expiry_date: DateTime<Utc>,
        stock: i64,
    }

    // Same WHERE shape as `domain::expenses::service::near_expiry`, minus the
    // per-tenant `tenant = $t` clause (this is an all-tenants nightly sweep).
    // `branch` (migración 0042) viaja en la proyección porque el digest agrupa
    // por local — y porque en SurrealDB el campo del ORDER BY tiene que estar
    // seleccionado, así que la proyección y el orden se mantienen juntos.
    let rows: Vec<Row> = db
        .query(
            "SELECT tenant, product, branch, batch_code, expiry_date, stock \
             FROM product_batch \
             WHERE active = true AND stock > 0 AND expiry_date <= $c \
             ORDER BY expiry_date ASC",
        )
        .bind(("c", surrealdb::sql::Datetime::from(cutoff)))
        .await?
        .check()?
        .take(0)?;

    let today = now.date_naive();
    let mut alerts: Vec<NearExpiryAlert> = rows
        .into_iter()
        .map(|r| {
            let days_to_expiry = (r.expiry_date.date_naive() - today).num_days();
            NearExpiryAlert {
                tenant: r.tenant.to_string(),
                sku: r.product.to_string(),
                lot: r.batch_code,
                branch: r.branch.map(|t| t.to_string()),
                expiry_date: r.expiry_date,
                days_to_expiry,
                stock: r.stock,
            }
        })
        .collect();

    // Most urgent first. `expiry_date ASC` already orders this way, but sort
    // explicitly on the derived field so the public contract holds regardless
    // of date→day rounding.
    alerts.sort_by_key(|a| a.days_to_expiry);

    emit_digest(&alerts, within);
    Ok(alerts)
}

/// Emit one `WARN` digest per **(tenant, sucursal)**. Agrupado así y no sólo por
/// tenant porque la acción es física y local: el dueño de dos locales necesita
/// saber a cuál ir a sacar el lote, y un digest mezclado lo obliga a filtrar a
/// mano. `BTreeMap` para salida determinística y ordenada.
fn emit_digest(alerts: &[NearExpiryAlert], within_days: i64) {
    let mut by_local: BTreeMap<(&str, Option<&str>), (usize, i64)> = BTreeMap::new();
    for a in alerts {
        let e = by_local
            .entry((a.tenant.as_str(), a.branch.as_deref()))
            .or_insert((0, i64::MAX));
        e.0 += 1;
        e.1 = e.1.min(a.days_to_expiry);
    }
    for ((tenant, branch), (count, soonest)) in by_local {
        // `branch = NONE` es casa matriz (bucket de la migración 0041/0042):
        // se rotula en castellano, no como `NONE`, porque este log lo lee un
        // humano en soporte.
        let local = branch.unwrap_or("casa matriz");
        tracing::warn!(
            tenant,
            local,
            lotes = count,
            within_days,
            soonest_days = soonest,
            "{count} lotes por vencer en ≤{within_days}días en {local}"
        );
    }
}
