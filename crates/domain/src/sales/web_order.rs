//! Web-channel order ingest (ADR-0012 pattern 2). The on-prem ERP receives
//! HMAC-signed orders from the external Tu Farmacia website and writes them
//! verbatim with `channel='web'`. Stock is *not* decremented — the order is
//! a reservation. The POS flow (`channel=NONE`) is what actually consumes
//! stock at pickup.
//!
//! ## Responsibilities of this module
//!
//! 1. **Validate** the incoming [`WebOrderRequest`] (RUT normalisation,
//!    payment-method whitelist, non-empty items, non-negative money).
//! 2. **Resolve / upsert** the [`customer`] row by RUT > phone > email > name.
//!    Pure best-effort match — a brand-new customer is created when no
//!    contact info matches.
//! 3. **Idempotency**: an existing `order` row with the same `(tenant,
//!    channel='web', external_ref)` short-circuits the create and returns
//!    the cached row. This is what makes the web push safe to retry without
//!    409 — the caller (a remote website) cannot guarantee at-most-once
//!    delivery semantics.
//! 4. **Atomic persist**: one `BEGIN; CREATE order; CREATE order_item×N;
//!    COMMIT;` so a partial write never leaves an order without items.
//!
//! ## Out of scope
//!
//! * No stock decrement. The order has status `reserved`. When the customer
//!   picks up, a counter cashier rings the POS sale referencing this order's
//!   id (BACKLOG: web→POS handoff flow).
//! * No loyalty award (deferred until pickup completes the sale).
//! * No FEFO planning (no stock change).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use crate::errors::{DomainError, DomainResult};
use crate::sales::model::POS_METHODS;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Set of payment methods accepted on the web channel. Mirrors POS plus the
/// online-only `webpay` rail and `store` (pay-at-counter on pickup).
pub const WEB_METHODS: &[&str] = &[
    "pos_cash",
    "pos_debit",
    "pos_credit",
    "pos_mixed",
    "webpay",
    "store",
    "transfer",
];

// --- request shape (deserialised by api crate from JSON body) ---------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct WebOrderRequest {
    /// Caller-supplied idempotency key. Persisted into `order.external_ref`.
    /// Repeat calls with the same `(tenant, external_order_id)` return the
    /// same order without creating duplicates.
    pub external_order_id: String,
    pub customer: WebOrderCustomer,
    pub items: Vec<WebOrderItem>,
    pub payment_method: String,
    /// Total in Chilean pesos, as a string to avoid float drift.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct WebOrderCustomer {
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub rut: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct WebOrderItem {
    /// Product `external_id` (commercial SKU) as exposed by the public
    /// catalog endpoint. Tenant-scoped lookup; missing SKU → 400.
    pub external_id: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
}

// --- response shape ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WebOrderResponse {
    pub order_id: String,
    /// `external_order_id` echoed back for the caller to correlate.
    pub external_order_id: String,
    /// `created` = first time we saw this `external_order_id`.
    /// `idempotent_replay` = matched a prior order, returned that one.
    pub status: WebOrderStatus,
    pub customer_id: String,
    pub total: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebOrderStatus {
    Created,
    IdempotentReplay,
}

// --- validation (pure, easy to unit-test) -----------------------------------

/// Normalise + validate request inputs. Does not touch the DB.
pub fn validate(req: &WebOrderRequest) -> DomainResult<()> {
    if req.external_order_id.trim().is_empty() {
        return Err(DomainError::Invalid("external_order_id requerido".into()));
    }
    if !WEB_METHODS.contains(&req.payment_method.as_str()) {
        return Err(DomainError::Invalid(format!(
            "método de pago inválido: {}",
            req.payment_method
        )));
    }
    if !POS_METHODS.contains(&req.payment_method.as_str())
        && !matches!(req.payment_method.as_str(), "webpay" | "store" | "transfer")
    {
        // Unreachable thanks to WEB_METHODS gate; double-check.
        return Err(DomainError::Invalid("método de pago desconocido".into()));
    }
    if req.items.is_empty() {
        return Err(DomainError::Invalid("items requeridos".into()));
    }
    for it in &req.items {
        if it.quantity <= 0 {
            return Err(DomainError::Invalid(format!(
                "cantidad inválida para {}: {}",
                it.external_id, it.quantity
            )));
        }
        if it.unit_price.is_sign_negative() {
            return Err(DomainError::Invalid(format!(
                "precio negativo para {}",
                it.external_id
            )));
        }
        if it.external_id.trim().is_empty() {
            return Err(DomainError::Invalid(
                "external_id de producto requerido".into(),
            ));
        }
    }
    if req.total.is_sign_negative() {
        return Err(DomainError::Invalid("total negativo".into()));
    }
    if req.customer.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre de cliente requerido".into()));
    }
    Ok(())
}

fn normalize_rut(raw: &str) -> String {
    raw.trim()
        .replace([' ', '.'], "")
        .replace('-', "")
        .to_uppercase()
}

fn clean_opt(s: Option<&str>) -> Option<String> {
    s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

// --- DB rows ----------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OrderIdRow {
    id: Thing,
    customer: Option<Thing>,
    total: Decimal,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CustomerIdRow {
    id: Thing,
}

#[derive(Debug, Deserialize)]
struct ProductIdRow {
    id: Thing,
    external_id: Option<String>,
}

// --- persistence -----------------------------------------------------------

/// Idempotent ingest: returns the existing order if one already exists for
/// `(tenant, channel='web', external_ref=external_order_id)`; otherwise
/// validates, upserts the customer, resolves products, and atomically writes
/// the order + its items.
pub async fn ingest(
    db: &Db,
    tenant: &Thing,
    req: WebOrderRequest,
) -> DomainResult<WebOrderResponse> {
    validate(&req)?;

    // --- idempotent short-circuit -----------------------------------------
    if let Some(existing) = lookup_existing(db, tenant, &req.external_order_id).await? {
        return Ok(WebOrderResponse {
            order_id: existing.id.to_string(),
            external_order_id: req.external_order_id,
            status: WebOrderStatus::IdempotentReplay,
            customer_id: existing.customer.map(|t| t.to_string()).unwrap_or_default(),
            total: existing.total.to_string(),
            created_at: existing.created_at,
        });
    }

    // --- customer upsert --------------------------------------------------
    let cust_name = req.customer.name.trim().to_string();
    let cust_phone = clean_opt(req.customer.phone.as_deref());
    let cust_email = clean_opt(req.customer.email.as_deref());
    let cust_rut = clean_opt(req.customer.rut.as_deref()).map(|r| normalize_rut(&r));
    let customer_id =
        upsert_customer(db, tenant, &cust_name, &cust_rut, &cust_phone, &cust_email).await?;

    // --- product resolution: map external_id → product Thing --------------
    let externals: Vec<String> = req.items.iter().map(|i| i.external_id.clone()).collect();
    let resolved = resolve_products_by_external_id(db, tenant, &externals).await?;
    // Validate every line resolves; missing → 400 with the offending sku.
    let mut resolved_ids: Vec<Thing> = Vec::with_capacity(req.items.len());
    for it in &req.items {
        let pid = resolved
            .iter()
            .find(|(ext, _)| ext.as_deref() == Some(it.external_id.as_str()))
            .map(|(_, id)| id.clone())
            .ok_or_else(|| {
                DomainError::Invalid(format!(
                    "producto no encontrado: external_id={}",
                    it.external_id
                ))
            })?;
        resolved_ids.push(pid);
    }

    let subtotal: Decimal = req
        .items
        .iter()
        .map(|i| i.unit_price * Decimal::from(i.quantity))
        .sum();

    // --- atomic persist ---------------------------------------------------
    let created = persist_web_order(
        db,
        tenant,
        &req.external_order_id,
        &req.payment_method,
        subtotal,
        req.total,
        &customer_id,
        &cust_name,
        cust_phone.as_deref(),
        &req,
        &resolved_ids,
    )
    .await?;

    Ok(WebOrderResponse {
        order_id: created.id.to_string(),
        external_order_id: req.external_order_id,
        status: WebOrderStatus::Created,
        customer_id: customer_id.to_string(),
        total: req.total.to_string(),
        created_at: created.created_at,
    })
}

async fn lookup_existing(
    db: &Db,
    tenant: &Thing,
    external_order_id: &str,
) -> DomainResult<Option<OrderIdRow>> {
    let mut r = db
        .query(
            "SELECT id, customer, total, created_at FROM order \
             WHERE tenant = $t AND channel = 'web' AND external_ref = $ext LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("ext", external_order_id.to_string()))
        .await?;
    let row: Option<OrderIdRow> = r.take(0)?;
    Ok(row)
}

/// Best-effort customer matcher. Priority: RUT > phone > email > newly created.
/// We deliberately don't update existing customer rows here — the customers
/// service is the authority for that.
async fn upsert_customer(
    db: &Db,
    tenant: &Thing,
    name: &str,
    rut: &Option<String>,
    phone: &Option<String>,
    email: &Option<String>,
) -> DomainResult<Thing> {
    // 1. RUT match (strongest signal — uniqueness is enforced at app layer).
    if let Some(r) = rut.as_deref().filter(|s| !s.is_empty()) {
        let mut q = db
            .query("SELECT id FROM customer WHERE tenant = $t AND rut = $rut LIMIT 1")
            .bind(("t", tenant.clone()))
            .bind(("rut", r.to_string()))
            .await?;
        let row: Option<CustomerIdRow> = q.take(0)?;
        if let Some(c) = row {
            return Ok(c.id);
        }
    }
    // 2. phone match.
    if let Some(p) = phone.as_deref().filter(|s| !s.is_empty()) {
        let mut q = db
            .query("SELECT id FROM customer WHERE tenant = $t AND phone = $p LIMIT 1")
            .bind(("t", tenant.clone()))
            .bind(("p", p.to_string()))
            .await?;
        let row: Option<CustomerIdRow> = q.take(0)?;
        if let Some(c) = row {
            return Ok(c.id);
        }
    }
    // 3. email match.
    if let Some(e) = email.as_deref().filter(|s| !s.is_empty()) {
        let mut q = db
            .query("SELECT id FROM customer WHERE tenant = $t AND email = $e LIMIT 1")
            .bind(("t", tenant.clone()))
            .bind(("e", e.to_string()))
            .await?;
        let row: Option<CustomerIdRow> = q.take(0)?;
        if let Some(c) = row {
            return Ok(c.id);
        }
    }
    // 4. create.
    let mut q = db
        .query(
            "CREATE customer SET tenant = $t, name = $name, rut = $rut, \
             phone = $phone, email = $email RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("rut", rut.clone()))
        .bind(("phone", phone.clone()))
        .bind(("email", email.clone()))
        .await?;
    let row: Option<CustomerIdRow> = q.take(0)?;
    row.map(|r| r.id).ok_or(DomainError::NotFound)
}

async fn resolve_products_by_external_id(
    db: &Db,
    tenant: &Thing,
    externals: &[String],
) -> DomainResult<Vec<(Option<String>, Thing)>> {
    if externals.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = db
        .query(
            "SELECT id, external_id FROM product \
             WHERE tenant = $t AND active = true AND external_id IN $exts",
        )
        .bind(("t", tenant.clone()))
        .bind(("exts", externals.to_vec()))
        .await?;
    let rows: Vec<ProductIdRow> = r.take(0)?;
    Ok(rows.into_iter().map(|r| (r.external_id, r.id)).collect())
}

#[allow(clippy::too_many_arguments)]
async fn persist_web_order(
    db: &Db,
    tenant: &Thing,
    external_order_id: &str,
    payment_method: &str,
    subtotal: Decimal,
    total: Decimal,
    customer: &Thing,
    customer_name: &str,
    customer_phone: Option<&str>,
    req: &WebOrderRequest,
    resolved_ids: &[Thing],
) -> DomainResult<OrderIdRow> {
    let oid = uuid::Uuid::new_v4().simple().to_string();
    let order_thing = surrealdb::sql::thing(&format!("order:{oid}"))
        .map_err(|e| DomainError::Other(anyhow::anyhow!("order id build: {e}")))?;

    let mut q = String::from(
        "BEGIN; \
         CREATE type::thing('order', $oid) SET tenant=$t, status='reserved', \
            payment_method=$pm, subtotal=$sub, discount=0, total=$tot, \
            customer=$cust, customer_name=$cname, customer_phone=$cphone, \
            external_ref=$ext, channel='web' RETURN AFTER; ",
    );
    for i in 0..req.items.len() {
        q.push_str(&format!(
            "CREATE order_item SET tenant=$t, order=$ord, product=$p{i}, \
                product_name=$pn{i}, quantity=$qty{i}, unit_price=$up{i}, \
                subtotal=$st{i}; ",
        ));
    }
    q.push_str("COMMIT;");

    let dec_val = |d: Decimal| -> surrealdb::sql::Value { surrealdb::sql::Number::from(d).into() };

    let mut qb = db
        .query(q)
        .bind(("oid", oid.clone()))
        .bind(("t", tenant.clone()))
        .bind(("ord", order_thing.clone()))
        .bind(("pm", payment_method.to_string()))
        .bind(("sub", dec_val(subtotal)))
        .bind(("tot", dec_val(total)))
        .bind(("cust", customer.clone()))
        .bind(("cname", customer_name.to_string()))
        .bind(("cphone", customer_phone.map(str::to_string)))
        .bind(("ext", external_order_id.to_string()));
    for (i, (item, pid)) in req.items.iter().zip(resolved_ids.iter()).enumerate() {
        let line_sub = item.unit_price * Decimal::from(item.quantity);
        qb = qb
            .bind((format!("p{i}"), pid.clone()))
            .bind((format!("pn{i}"), item.external_id.clone()))
            .bind((format!("qty{i}"), item.quantity))
            .bind((format!("up{i}"), dec_val(item.unit_price)))
            .bind((format!("st{i}"), dec_val(line_sub)));
    }
    let mut resp = qb.await?.check()?;
    // Take the first result (the order CREATE).
    let row: Option<OrderIdRow> = resp.take(0)?;
    row.ok_or(DomainError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req_template() -> WebOrderRequest {
        WebOrderRequest {
            external_order_id: "ord-1".into(),
            customer: WebOrderCustomer {
                name: "Juan Pérez".into(),
                phone: Some("+56912345678".into()),
                rut: Some("12345678-9".into()),
                email: None,
            },
            items: vec![WebOrderItem {
                external_id: "SKU-1".into(),
                quantity: 2,
                unit_price: Decimal::from(1000),
            }],
            payment_method: "webpay".into(),
            total: Decimal::from(2000),
        }
    }

    #[test]
    fn validate_ok() {
        validate(&req_template()).expect("valid");
    }

    #[test]
    fn validate_rejects_empty_external_order_id() {
        let mut r = req_template();
        r.external_order_id = "  ".into();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn validate_rejects_unknown_payment_method() {
        let mut r = req_template();
        r.payment_method = "crypto".into();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn validate_rejects_zero_quantity() {
        let mut r = req_template();
        r.items[0].quantity = 0;
        assert!(validate(&r).is_err());
    }

    #[test]
    fn validate_rejects_negative_price() {
        let mut r = req_template();
        r.items[0].unit_price = Decimal::from(-1);
        assert!(validate(&r).is_err());
    }

    #[test]
    fn validate_rejects_empty_items() {
        let mut r = req_template();
        r.items.clear();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn validate_rejects_empty_customer_name() {
        let mut r = req_template();
        r.customer.name = "   ".into();
        assert!(validate(&r).is_err());
    }
}
