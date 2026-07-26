//! Inventory persistence. Pure async fns over `&Db`. Every query is
//! tenant-scoped: callers pass the tenant `Thing` from the JWT claim.
//!
//! Stock atomicity: [`apply_movement`] runs `BEGIN; CREATE stock_movement;
//! UPDATE product SET stock += $delta; COMMIT;` so the audit row and the
//! materialized counter cannot diverge. Pre-condition checks (negative-stock,
//! product belongs to tenant) live in [`super::service`].

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::catalog::model::ProductDto;
use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

// --- decimal bind helpers (mirror `catalog::repo`) -------------------------
// `rust_decimal` (with `serde-with-str`) serializes as a JSON string, which
// SurrealQL `decimal` rejects on bind. Convert to a native Number::Decimal.

fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

fn dec_opt(d: Option<Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(x) => dec_val(x),
        None => surrealdb::sql::Value::None,
    }
}

/// Bind a chrono `DateTime<Utc>` as a native SurrealDB `datetime` value.
/// Without this wrapper the default serde route ships an ISO string which
/// SurrealQL stores as a `string` and a `FIELD ... TYPE datetime` rejects.
fn dt_val(dt: DateTime<Utc>) -> surrealdb::sql::Value {
    surrealdb::sql::Datetime::from(dt).into()
}

fn dt_opt(dt: Option<DateTime<Utc>>) -> surrealdb::sql::Value {
    match dt {
        Some(x) => dt_val(x),
        None => surrealdb::sql::Value::None,
    }
}

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MovementRow {
    id: Thing,
    product: Thing,
    delta: i64,
    reason: String,
    admin: Option<Thing>,
    r#ref: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<MovementRow> for StockMovementDto {
    fn from(r: MovementRow) -> Self {
        Self {
            id: r.id.to_string(),
            product: r.product.to_string(),
            delta: r.delta,
            reason: r.reason,
            admin: r.admin.map(|a| a.to_string()),
            r#ref: r.r#ref,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct BatchRow {
    id: Thing,
    product: Thing,
    #[serde(default)]
    branch: Option<Thing>,
    batch_code: String,
    expiry_date: DateTime<Utc>,
    stock: i64,
    cost: Option<Decimal>,
    notes: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<BatchRow> for BatchDto {
    fn from(r: BatchRow) -> Self {
        Self {
            id: r.id.to_string(),
            product: r.product.to_string(),
            branch: r.branch.map(|t| t.to_string()),
            batch_code: r.batch_code,
            expiry_date: r.expiry_date,
            stock: r.stock,
            cost: r.cost,
            notes: r.notes,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FaltaRow {
    id: Thing,
    product: Option<Thing>,
    name: String,
    qty: i64,
    customer_name: Option<String>,
    customer_phone: Option<String>,
    notes: Option<String>,
    resolved: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<FaltaRow> for FaltaDto {
    fn from(r: FaltaRow) -> Self {
        Self {
            id: r.id.to_string(),
            product: r.product.map(|p| p.to_string()),
            name: r.name,
            qty: r.qty,
            customer_name: r.customer_name,
            customer_phone: r.customer_phone,
            notes: r.notes,
            resolved: r.resolved,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// --- product helper --------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProductFull {
    id: Thing,
    name: String,
    slug: String,
    description: Option<String>,
    price: Decimal,
    cost_price: Option<Decimal>,
    stock: i64,
    #[serde(default = "default_true")]
    physical_stock: bool,
    category: Option<Thing>,
    image_url: Option<String>,
    active: bool,
    external_id: Option<String>,
    laboratory: Option<String>,
    therapeutic_action: Option<String>,
    active_ingredient: Option<String>,
    prescription_type: String,
    presentation: Option<String>,
    discount_percent: Option<i64>,
    /// Per-rubro flexible attributes (migration 0033); absent on older rows.
    #[serde(default)]
    attrs: Option<serde_json::Value>,
    /// Parent product when this row is a variant (migration 0034).
    #[serde(default)]
    parent_id: Option<Thing>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Serde fallback for rows persisted before migration 0031.
fn default_true() -> bool {
    true
}

impl From<ProductFull> for ProductDto {
    fn from(r: ProductFull) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            slug: r.slug,
            description: r.description,
            price: r.price,
            cost_price: r.cost_price,
            stock: r.stock,
            physical_stock: r.physical_stock,
            category: r.category.map(|c| c.to_string()),
            image_url: r.image_url,
            active: r.active,
            external_id: r.external_id,
            laboratory: r.laboratory,
            therapeutic_action: r.therapeutic_action,
            active_ingredient: r.active_ingredient,
            prescription_type: r.prescription_type,
            presentation: r.presentation,
            discount_percent: r.discount_percent,
            attrs: r.attrs,
            parent_id: r.parent_id.map(|p| p.to_string()),
            barcode: None,
            variants_stock: None,
            variant_count: None,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Fetch a tenant-scoped product (full row) — used by the service to gate
/// movements (existence + current stock for negative-stock check).
pub async fn product_for_movement(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<ProductDto>> {
    let mut r = db
        .query("SELECT * FROM product WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<ProductFull> = r.take(0)?;
    Ok(row.map(Into::into))
}

// --- stock_movement --------------------------------------------------------

/// Insert a `stock_movement` and update `product.stock` atomically.
/// Caller MUST have validated tenant ownership and non-negative resulting stock.
#[allow(clippy::too_many_arguments)]
pub async fn apply_movement(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    delta: i64,
    reason: &str,
    admin: Option<Thing>,
    reference: Option<String>,
) -> DomainResult<(StockMovementDto, ProductDto)> {
    let mut r = db
        .query(
            "BEGIN; \
             CREATE stock_movement SET tenant = $t, product = $p, delta = $d, \
                 reason = $r, admin = $a, ref = $ref RETURN AFTER; \
             UPDATE product SET stock = stock + $d \
                 WHERE id = $p AND tenant = $t RETURN AFTER; \
             COMMIT;",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("d", delta))
        .bind(("r", reason.to_string()))
        .bind(("a", admin))
        .bind(("ref", reference))
        .await?
        .check()?;
    let movement: Option<MovementRow> = r.take(0)?;
    let updated: Option<ProductFull> = r.take(1)?;
    let movement = movement.ok_or(DomainError::NotFound)?.into();
    let updated = updated.ok_or(DomainError::NotFound)?.into();
    Ok((movement, updated))
}

pub async fn list_movements(
    db: &Db,
    tenant: &Thing,
    f: &MovementFilters,
) -> DomainResult<Vec<StockMovementDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.product.is_some() {
        conds.push("product = $p".to_string());
    }
    if f.reason.is_some() {
        conds.push("reason = $r".to_string());
    }
    if f.from.is_some() {
        conds.push("created_at >= $from".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $to".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM stock_movement WHERE {} \
         ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset,
    );
    let p = f
        .product
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", p))
        .bind(("r", f.reason.clone().unwrap_or_default()))
        .bind(("from", dt_opt(f.from)))
        .bind(("to", dt_opt(f.to)))
        .await?;
    let rows: Vec<MovementRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

// --- product_batch ---------------------------------------------------------

/// Condición SurrealQL para acotar a una sucursal. La casa matriz se compara
/// contra el literal `NONE` en vez de contra un bind: un `Option::None` bindeado
/// llega como NONE igual, pero el literal deja la intención explícita en el SQL
/// y es el mismo patrón que usa `stock::repo` (migración 0041).
fn branch_cond(branch: Option<&Thing>) -> &'static str {
    if branch.is_some() {
        "branch = $br"
    } else {
        "branch = NONE"
    }
}

/// Create a batch + (if `initial_stock > 0`) emit the matching `+delta`
/// `stock_movement` and update `product.stock` — all in one transaction.
///
/// `branch` (migración 0042) es la sucursal donde queda el lote físico. Se
/// estampa en el lote **y** en el `stock_movement`, así el evento
/// `product_branch_stock_maint` sube el bucket del mismo local que va a poder
/// consumirlo por FEFO. Estampar sólo uno de los dos rompería
/// `Σ product_batch.stock[X] == product_branch_stock[X]`.
#[allow(clippy::too_many_arguments)]
pub async fn create_batch_atomic(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    branch: Option<&Thing>,
    input: &NewBatch,
    admin: Option<Thing>,
) -> DomainResult<(BatchDto, Option<ProductDto>)> {
    let initial = input.stock;
    let mut sql = String::from(
        "BEGIN; \
         CREATE product_batch SET tenant = $t, product = $p, branch = $br, \
             batch_code = $code, expiry_date = $exp, stock = $stock, cost = $cost, \
             notes = $notes \
             RETURN AFTER;",
    );
    if initial > 0 {
        sql.push_str(
            " CREATE stock_movement SET tenant = $t, product = $p, delta = $stock, \
              reason = 'batch_received', admin = $admin, ref = $code, branch = $br \
              RETURN AFTER;\
              UPDATE product SET stock = stock + $stock \
              WHERE id = $p AND tenant = $t RETURN AFTER;",
        );
    }
    sql.push_str(" COMMIT;");
    let mut r = db
        .query(sql)
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("br", branch.cloned()))
        .bind(("code", input.batch_code.clone()))
        .bind(("exp", dt_val(input.expiry_date)))
        .bind(("stock", initial))
        .bind(("cost", dec_opt(input.cost)))
        .bind(("notes", input.notes.clone()))
        .bind(("admin", admin))
        .await?
        .check()?;
    let batch: Option<BatchRow> = r.take(0)?;
    let batch: BatchDto = batch.ok_or(DomainError::NotFound)?.into();
    let updated_product: Option<ProductDto> = if initial > 0 {
        r.take::<Option<ProductFull>>(2)?.map(Into::into)
    } else {
        None
    };
    Ok((batch, updated_product))
}

pub async fn list_batches(
    db: &Db,
    tenant: &Thing,
    f: &BatchFilters,
) -> DomainResult<Vec<BatchDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.product.is_some() {
        conds.push("product = $p".to_string());
    }
    // Sucursal: tri-estado. Ausente = todas (el dueño mira el negocio entero);
    // `"none"` = sólo casa matriz; un `branch:<key>` = ese local.
    let branch_thing = match f.branch.as_deref() {
        Some("none") | Some("") | None => None,
        Some(s) => Some(
            surrealdb::sql::thing(s)
                .map_err(|_| DomainError::Invalid(format!("sucursal inválida: {s}")))?,
        ),
    };
    if matches!(f.branch.as_deref(), Some("none") | Some("")) {
        conds.push("branch = NONE".to_string());
    } else if branch_thing.is_some() {
        conds.push("branch = $br".to_string());
    }
    if f.only_available.unwrap_or(false) {
        conds.push("active = true AND stock > 0 AND expiry_date >= time::now()".to_string());
    }
    if f.expiring_within_days.is_some() {
        conds.push("expiry_date <= time::now() + duration::from::days($days)".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM product_batch WHERE {} \
         ORDER BY expiry_date ASC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset,
    );
    let p = f
        .product
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", p))
        .bind(("br", branch_thing))
        .bind(("days", f.expiring_within_days.unwrap_or(0)))
        .await?;
    let rows: Vec<BatchRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_batch(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<Option<BatchDto>> {
    let mut r = db
        .query("SELECT * FROM product_batch WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<BatchRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_batch(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateBatch,
) -> DomainResult<BatchDto> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.batch_code.is_some() {
        sets.push("batch_code = $code");
    }
    if patch.expiry_date.is_some() {
        sets.push("expiry_date = $exp");
    }
    if patch.cost.is_some() {
        sets.push("cost = $cost");
    }
    if patch.notes.is_some() {
        sets.push("notes = $notes");
    }
    if patch.active.is_some() {
        sets.push("active = $active");
    }
    if sets.is_empty() {
        return get_batch(db, tenant, id)
            .await?
            .ok_or(DomainError::NotFound);
    }
    let q = format!(
        "UPDATE product_batch SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", "),
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("code", patch.batch_code.clone()))
        .bind(("exp", dt_opt(patch.expiry_date)))
        .bind(("cost", dec_opt(patch.cost)))
        .bind(("notes", patch.notes.clone()))
        .bind(("active", patch.active))
        .await?;
    let row: Option<BatchRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Soft-delete: `active = false`. Preserves audit trail (FEFO will skip
/// inactive batches).
pub async fn soft_delete_batch(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query(
            "UPDATE product_batch SET active = false \
             WHERE id = $id AND tenant = $t RETURN AFTER",
        )
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- FEFO ------------------------------------------------------------------

/// Count of `active` batches for a product **in one branch** (any stock/expiry).
/// `0` means the product is not batch-tracked *in that branch*, so the sale
/// falls back to the plain `product.stock` path; `> 0` means FEFO consumption is
/// mandatory (a batch-tracked product with only expired/empty lots must NOT
/// sell).
///
/// Acotar el conteo a la sucursal es lo que evita el falso positivo inverso:
/// si el producto tiene lotes en el local B pero ninguno en A, vender en A NO
/// debe exigir FEFO contra lotes ajenos — la sucursal A simplemente no lleva
/// lotes de ese producto y cae al camino `product.stock` (que el pre-chequeo por
/// bucket de 0041 ya limitó a lo que A realmente tiene).
pub async fn count_active_batches(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    branch: Option<&Thing>,
) -> DomainResult<i64> {
    let q = format!(
        "SELECT count() AS n FROM product_batch \
         WHERE tenant = $t AND product = $p AND active = true AND {} GROUP ALL",
        branch_cond(branch),
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("br", branch.cloned()))
        .await?;
    let n: Option<i64> = r.take((0, "n"))?;
    Ok(n.unwrap_or(0))
}

/// Read-only FEFO plan **acotado a una sucursal**: order eligible batches by
/// `expiry_date ASC, created_at ASC` and allocate `qty`. Returns
/// `Err(InsufficientStock)` if total available < qty. Caller (sales Fase 4) is
/// responsible for writing the resulting decrements and the matching
/// `stock_movement`.
///
/// El filtro por `branch` (migración 0042) es la regla de negocio central de
/// esta lane: vender en el local A consume el lote de A que vence primero, nunca
/// el de B — aunque el de B venza antes. El frasco que el cajero tiene en la
/// mano y el vencimiento que el sistema descuenta son el mismo.
pub async fn plan_fefo(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    qty: i64,
    branch: Option<&Thing>,
) -> DomainResult<Vec<FefoAllocation>> {
    if qty <= 0 {
        return Err(DomainError::Invalid("qty debe ser > 0".into()));
    }
    // `expiry_date` y `created_at` van en el SELECT porque SurrealDB exige que
    // el campo del ORDER BY esté proyectado.
    let q = format!(
        "SELECT id, stock, expiry_date, created_at FROM product_batch \
         WHERE tenant = $t AND product = $p AND active = true AND stock > 0 \
           AND expiry_date >= time::now() AND {} \
         ORDER BY expiry_date ASC, created_at ASC",
        branch_cond(branch),
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("br", branch.cloned()))
        .await?;

    #[derive(Deserialize)]
    struct Row {
        id: Thing,
        stock: i64,
    }
    let rows: Vec<Row> = r.take(0)?;

    // Rows arrive already FEFO-sorted (expiry ASC, created ASC) from SQL; the
    // pure allocator in `crate::invariants` is property-tested.
    let lots: Vec<(String, i64)> = rows
        .into_iter()
        .map(|row| (row.id.to_string(), row.stock))
        .collect();
    crate::invariants::fefo_plan(&lots, qty)
}

// --- falta -----------------------------------------------------------------

pub async fn create_falta(
    db: &Db,
    tenant: &Thing,
    product: Option<Thing>,
    input: &NewFalta,
) -> DomainResult<FaltaDto> {
    let mut r = db
        .query(
            "CREATE falta SET tenant = $t, product = $p, name = $name, qty = $qty, \
             customer_name = $cn, customer_phone = $cp, notes = $notes RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product))
        .bind(("name", input.name.clone()))
        .bind(("qty", input.qty))
        .bind(("cn", input.customer_name.clone()))
        .bind(("cp", input.customer_phone.clone()))
        .bind(("notes", input.notes.clone()))
        .await?;
    let row: Option<FaltaRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_faltas(db: &Db, tenant: &Thing, f: &FaltaFilters) -> DomainResult<Vec<FaltaDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.resolved.is_some() {
        conds.push("resolved = $resolved".to_string());
    }
    if f.product.is_some() {
        conds.push("product = $p".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM falta WHERE {} ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset,
    );
    let p = f
        .product
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("resolved", f.resolved))
        .bind(("p", p))
        .await?;
    let rows: Vec<FaltaRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn update_falta(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateFalta,
) -> DomainResult<FaltaDto> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.name.is_some() {
        sets.push("name = $name");
    }
    if patch.qty.is_some() {
        sets.push("qty = $qty");
    }
    if patch.customer_name.is_some() {
        sets.push("customer_name = $cn");
    }
    if patch.customer_phone.is_some() {
        sets.push("customer_phone = $cp");
    }
    if patch.notes.is_some() {
        sets.push("notes = $notes");
    }
    if patch.resolved.is_some() {
        sets.push("resolved = $resolved");
    }
    if sets.is_empty() {
        let mut r = db
            .query("SELECT * FROM falta WHERE id = $id AND tenant = $t LIMIT 1")
            .bind(("id", id.clone()))
            .bind(("t", tenant.clone()))
            .await?;
        let row: Option<FaltaRow> = r.take(0)?;
        return row.map(Into::into).ok_or(DomainError::NotFound);
    }
    let q = format!(
        "UPDATE falta SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", "),
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("name", patch.name.clone()))
        .bind(("qty", patch.qty))
        .bind(("cn", patch.customer_name.clone()))
        .bind(("cp", patch.customer_phone.clone()))
        .bind(("notes", patch.notes.clone()))
        .bind(("resolved", patch.resolved))
        .await?;
    let row: Option<FaltaRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

// --- summary / abc / reorder ----------------------------------------------

pub async fn inventory_summary(
    db: &Db,
    tenant: &Thing,
    low: i64,
    expiring_days: i64,
) -> DomainResult<InventorySummary> {
    // Product-side aggregate (total / active / low / out-of-stock / value) is
    // served from the maintained `product_stats` view (migration 0029) — an
    // O(1) read. This used to run the same `GROUP ALL` full scan over `product`
    // that BUG-perf-001 fixed for `catalog::repo::stats` (2.7s p99 @50k SKUs);
    // this inventory-module-landing call site was a second copy of that scan.
    // `catalog::repo::stats` falls back to the live scan for a non-default `low`
    // or a tenant with no view row, so the result stays byte-identical.
    let prod = crate::catalog::repo::stats(db, tenant, low).await?;

    #[derive(Deserialize, Default)]
    struct BatchAgg {
        expiring_soon: i64,
        expired: i64,
    }
    // Batch expiry + open-faltas counts: bounded by the (far smaller) batch and
    // falta tables, not the catalog, so these stay cheap.
    let mut r = db
        .query(
            "SELECT \
               count(expiry_date <= time::now() + duration::from::days($days) \
                     AND expiry_date >= time::now() \
                     AND active = true AND stock > 0) AS expiring_soon, \
               count(expiry_date < time::now() AND stock > 0 AND active = true) AS expired \
             FROM product_batch WHERE tenant = $t GROUP ALL;\
             SELECT count() AS open FROM falta WHERE tenant = $t AND resolved = false GROUP ALL;",
        )
        .bind(("t", tenant.clone()))
        .bind(("days", expiring_days))
        .await?;
    let batch: Option<BatchAgg> = r.take(0)?;
    let open: Option<i64> = r.take((1, "open"))?;
    let batch = batch.unwrap_or_default();
    Ok(InventorySummary {
        total_skus: prod.total,
        active_skus: prod.active,
        low_stock: prod.low_stock,
        out_of_stock: prod.out_of_stock,
        inventory_value: prod.inventory_value,
        expiring_soon: batch.expiring_soon,
        expired: batch.expired,
        open_faltas: open.unwrap_or(0),
    })
}

#[derive(Debug, Deserialize)]
pub struct AbcRaw {
    pub product: Thing,
    pub name: String,
    pub stock: i64,
    pub value: Decimal,
}

pub async fn abc_raw_rows(db: &Db, tenant: &Thing) -> DomainResult<Vec<AbcRaw>> {
    let mut r = db
        .query(
            "SELECT id AS product, name, stock, \
                stock * (cost_price ?? 0dec) AS value \
             FROM product WHERE tenant = $t AND active = true \
             ORDER BY value DESC",
        )
        .bind(("t", tenant.clone()))
        .await?;
    let rows: Vec<AbcRaw> = r.take(0)?;
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct ReorderRaw {
    pub product: Thing,
    pub name: String,
    pub stock: i64,
}

pub async fn low_stock_rows(db: &Db, tenant: &Thing, low: i64) -> DomainResult<Vec<ReorderRaw>> {
    let mut r = db
        .query(
            "SELECT id AS product, name, stock FROM product \
             WHERE tenant = $t AND active = true AND stock <= $low \
             ORDER BY stock ASC, name ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("low", low))
        .await?;
    let rows: Vec<ReorderRaw> = r.take(0)?;
    Ok(rows)
}
