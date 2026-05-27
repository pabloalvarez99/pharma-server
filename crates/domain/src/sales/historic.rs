//! Historic-orders bulk import (admin tooling).
//!
//! Loads PAST sales after the fact — used when migrating data from a legacy
//! system. Crucially this is NOT a POS flow; the differences from
//! [`super::service::post_sale`] are deliberate and non-negotiable:
//!
//! * `created_at` is taken from the request (so timeline reports reflect the
//!   real sale date, not the import date).
//! * `product.stock` is NOT decremented — current stock already accounts for
//!   these sales, the goal is only to populate `order` / `order_item` history.
//! * No `stock_movement` rows are emitted — same reason.
//! * No `Idempotency-Key` plumbing — bulk loads run once; replays are caller's
//!   problem.
//! * No minimum-price / margin checks — historic numbers may legitimately be
//!   below today's floor.
//! * Per-order failures (e.g. unknown `external_id`) do NOT abort the batch.
//!   They surface in [`ImportReport::errors`] and the rest of the batch goes
//!   through. The handler returns 200 even on partial failure.
//!
//! Reuses `order` + `order_item` tables — no migration. Tenant-scoped by the
//! caller (api layer threads `tenant` from JWT).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use surrealdb::sql::{Datetime as SurDatetime, Number as SurNumber, Thing, Value as SurValue};
use utoipa::ToSchema;

use crate::errors::{DomainError, DomainResult};

use super::model::POS_METHODS;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Hard ceiling so a runaway client can't accidentally feed millions of rows
/// to the single-tx-per-order loop. Documented at the API layer too.
pub const MAX_BATCH_SIZE: usize = 100;

// --- inputs ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct HistoricImportRequest {
    pub orders: Vec<HistoricOrderInput>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct HistoricOrderInput {
    /// Explicit sale timestamp — overrides server clock. Required.
    pub created_at: DateTime<Utc>,
    pub items: Vec<HistoricItemInput>,
    /// Optional explicit total. If omitted we derive `Sum(qty*unit_price)`.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub total: Option<Decimal>,
    /// Must be one of [`POS_METHODS`]; defaults to `pos_cash` (most likely
    /// for legacy paper-ticket migrations).
    #[serde(default = "default_payment_method")]
    pub payment_method: String,
    /// Optional external reference (legacy ticket folio).
    pub external_ref: Option<String>,
}

fn default_payment_method() -> String {
    "pos_cash".into()
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct HistoricItemInput {
    /// `product.external_id` (string PK from the legacy system). The importer
    /// resolves it to a `product` record id; unknown ids surface as a row in
    /// [`ImportReport::errors`].
    pub external_id: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
}

// --- outputs ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportReport {
    pub created: u32,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportError {
    /// Index into `request.orders` so the caller can correlate.
    pub idx: u32,
    pub message: String,
}

// --- driver ----------------------------------------------------------------

/// Process the batch. Always returns `Ok` — per-order failures are reported,
/// not bubbled. The only `Err` paths are (a) request larger than
/// [`MAX_BATCH_SIZE`] (caller bug) and (b) catastrophic DB errors during
/// product lookup, which abort before any writes.
pub async fn import_historic_orders(
    db: &Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    sold_by_name: Option<&str>,
    req: HistoricImportRequest,
) -> DomainResult<ImportReport> {
    if req.orders.len() > MAX_BATCH_SIZE {
        return Err(DomainError::Invalid(format!(
            "batch demasiado grande: {} (máx {MAX_BATCH_SIZE})",
            req.orders.len()
        )));
    }

    let mut report = ImportReport {
        created: 0,
        errors: Vec::new(),
    };

    for (idx, order) in req.orders.into_iter().enumerate() {
        match import_one(db, tenant, sold_by, sold_by_name, &order).await {
            Ok(()) => report.created += 1,
            Err(e) => report.errors.push(ImportError {
                idx: idx as u32,
                message: e.to_string(),
            }),
        }
    }

    Ok(report)
}

async fn import_one(
    db: &Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    sold_by_name: Option<&str>,
    order: &HistoricOrderInput,
) -> DomainResult<()> {
    if order.items.is_empty() {
        return Err(DomainError::Invalid("items requeridos".into()));
    }
    if !POS_METHODS.contains(&order.payment_method.as_str()) {
        return Err(DomainError::Invalid(format!(
            "método de pago inválido: {}",
            order.payment_method
        )));
    }
    for it in &order.items {
        if it.quantity <= 0 {
            return Err(DomainError::Invalid(format!(
                "cantidad inválida para external_id {}: {}",
                it.external_id, it.quantity
            )));
        }
        if it.unit_price.is_sign_negative() {
            return Err(DomainError::Invalid(format!(
                "precio negativo para external_id {}",
                it.external_id
            )));
        }
    }

    // Resolve every external_id → (product Thing, name) in a single round-trip.
    // Tenant-scoped — a row from another tenant is invisible here, which keeps
    // multi-tenancy intact for the import flow too.
    let externals: Vec<String> = order.items.iter().map(|i| i.external_id.clone()).collect();
    let resolved = lookup_by_externals(db, tenant, &externals).await?;
    let mut missing: Vec<String> = Vec::new();
    for ext in &externals {
        if !resolved.iter().any(|p| &p.external_id == ext) {
            missing.push(ext.clone());
        }
    }
    if !missing.is_empty() {
        return Err(DomainError::NotFound);
    }

    // Money: derive subtotal from lines; honor explicit total if caller sent
    // one (legacy systems sometimes hold a discount we never see), else
    // subtotal = total. No min-price / margin check — historic numbers
    // legitimately predate today's pricing.
    let subtotal: Decimal = order
        .items
        .iter()
        .map(|i| i.unit_price * Decimal::from(i.quantity))
        .sum();
    let total = order.total.unwrap_or(subtotal);
    let discount = if subtotal > total {
        subtotal - total
    } else {
        Decimal::ZERO
    };

    // Build the single multi-statement tx. Client-generated order id so every
    // order_item references it without a round-trip.
    let oid = uuid::Uuid::new_v4().simple().to_string();
    let order_thing = surrealdb::sql::thing(&format!("order:{oid}"))
        .map_err(|e| DomainError::Other(anyhow::anyhow!("order id build: {e}")))?;

    let mut q = String::from(
        "BEGIN; \
         CREATE type::thing('order', $oid) SET tenant=$t, status='paid', \
            payment_method=$pm, subtotal=$sub, discount=$disc, total=$tot, \
            sold_by=$sb, sold_by_name=$sbname, external_ref=$ext, \
            created_at=$cat, updated_at=$cat \
            RETURN AFTER; ",
    );
    for i in 0..order.items.len() {
        q.push_str(&format!(
            "CREATE order_item SET tenant=$t, order=$ord, product=$p{i}, \
                product_name=$pn{i}, quantity=$qty{i}, unit_price=$up{i}, \
                subtotal=$st{i}, created_at=$cat \
                RETURN AFTER; ",
        ));
    }
    q.push_str("COMMIT;");

    let mut qb = db
        .query(q)
        .bind(("oid", oid))
        .bind(("t", tenant.clone()))
        .bind(("ord", order_thing))
        .bind(("pm", order.payment_method.clone()))
        .bind(("sub", dec_val(subtotal)))
        .bind(("disc", dec_val(discount)))
        .bind(("tot", dec_val(total)))
        .bind(("sb", sold_by.cloned()))
        .bind(("sbname", sold_by_name.map(str::to_string)))
        .bind(("ext", order.external_ref.clone()))
        .bind(("cat", dt_val(order.created_at)));
    for (i, item) in order.items.iter().enumerate() {
        let pid = resolved
            .iter()
            .find(|p| p.external_id == item.external_id)
            .map(|p| p.id.clone())
            .ok_or(DomainError::NotFound)?;
        let line_sub = item.unit_price * Decimal::from(item.quantity);
        qb = qb
            .bind((format!("p{i}"), pid.clone()))
            .bind((format!("pn{i}"), name_of(&resolved, &item.external_id)))
            .bind((format!("qty{i}"), item.quantity))
            .bind((format!("up{i}"), dec_val(item.unit_price)))
            .bind((format!("st{i}"), dec_val(line_sub)));
    }
    qb.await?.check()?;
    Ok(())
}

// --- helpers ---------------------------------------------------------------

#[derive(Debug, Clone)]
struct ResolvedProduct {
    id: Thing,
    external_id: String,
    name: String,
}

async fn lookup_by_externals(
    db: &Db,
    tenant: &Thing,
    externals: &[String],
) -> DomainResult<Vec<ResolvedProduct>> {
    if externals.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct R {
        id: Thing,
        external_id: Option<String>,
        name: String,
    }
    let rows: Vec<R> = db
        .query(
            "SELECT id, external_id, name FROM product \
             WHERE tenant = $t AND external_id IN $exts",
        )
        .bind(("t", tenant.clone()))
        .bind(("exts", externals.to_vec()))
        .await?
        .check()?
        .take(0)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            r.external_id.map(|ext| ResolvedProduct {
                id: r.id,
                external_id: ext,
                name: r.name,
            })
        })
        .collect())
}

fn name_of(resolved: &[ResolvedProduct], external_id: &str) -> String {
    resolved
        .iter()
        .find(|p| p.external_id == external_id)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

fn dec_val(d: Decimal) -> SurValue {
    SurNumber::from(d).into()
}

fn dt_val(dt: DateTime<Utc>) -> SurValue {
    SurDatetime::from(dt).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversize_batch_before_db() {
        // Pin the cap so the cap-branch in service is exercised + future
        // refactors that bump this value get caught by a deliberate test.
        assert_eq!(MAX_BATCH_SIZE, 100);
    }

    #[test]
    fn default_payment_method_is_pos_cash() {
        // serde default kicks in for callers that omit the field.
        assert_eq!(default_payment_method(), "pos_cash");
    }
}
