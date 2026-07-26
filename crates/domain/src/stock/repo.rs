//! Persistencia de stock por sucursal. Fns async sobre `&Db`, todas
//! tenant-scoped (el caller pasa el `Thing` del tenant desde el claim JWT).
//!
//! El stock por sucursal NO se escribe directo: lo mantiene el evento
//! `product_branch_stock_maint` (migración 0041) desde `stock_movement`. Este
//! repo sólo LEE `product_branch_stock` y, para la transferencia, emite los dos
//! movimientos de suma cero que el evento reparte entre buckets — `product.stock`
//! queda intacto (el invariante global `product.stock = Σ stock_movement.delta`
//! se preserva porque la suma de los dos deltas es cero).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BranchStockRow {
    product: Thing,
    product_name: Option<String>,
    branch: Option<Thing>,
    branch_name: Option<String>,
    stock: i64,
    updated_at: DateTime<Utc>,
}

impl From<BranchStockRow> for BranchStockDto {
    fn from(r: BranchStockRow) -> Self {
        Self {
            product: r.product.to_string(),
            product_name: r.product_name,
            branch: r.branch.map(|t| t.to_string()),
            branch_name: r.branch_name,
            stock: r.stock,
            updated_at: r.updated_at,
        }
    }
}

/// Proyección común: resuelve los nombres siguiendo los links para que la UI no
/// tenga que cruzar dos listados. `branch.name` es NONE en casa matriz.
const SELECT_COLS: &str = "SELECT product, product.name AS product_name, branch, \
                           branch.name AS branch_name, stock, updated_at \
                           FROM product_branch_stock";

// --- reads -----------------------------------------------------------------

/// Lista el stock por sucursal del tenant. `filters.branch = "none"` aísla la
/// casa matriz; un `branch:<key>` filtra esa sucursal.
pub async fn list_branch_stock(
    db: &Db,
    tenant: &Thing,
    f: &BranchStockFilters,
) -> DomainResult<Vec<BranchStockDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.product.is_some() {
        conds.push("product = $p".to_string());
    }
    // Filtro de sucursal: el literal "none" = bucket casa matriz (branch = NONE);
    // cualquier otro valor se parsea como record-id de sucursal.
    let branch_none = matches!(f.branch.as_deref(), Some("none"));
    let branch_thing = match f.branch.as_deref() {
        Some("none") | None => None,
        Some(s) => Some(
            surrealdb::sql::thing(s)
                .map_err(|_| DomainError::Invalid(format!("sucursal inválida: {s}")))?,
        ),
    };
    if branch_none {
        conds.push("branch = NONE".to_string());
    } else if branch_thing.is_some() {
        conds.push("branch = $b".to_string());
    }
    if f.non_zero {
        conds.push("stock != 0".to_string());
    }
    let p = match f.product.as_deref() {
        None => None,
        Some(s) => Some(
            surrealdb::sql::thing(s)
                .map_err(|_| DomainError::Invalid(format!("producto inválido: {s}")))?,
        ),
    };
    let q = format!("{SELECT_COLS} WHERE {}", conds.join(" AND "));
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", p))
        .bind(("b", branch_thing))
        .await?
        .check()?;
    let rows: Vec<BranchStockRow> = r.take(0)?;
    let mut out: Vec<BranchStockDto> = rows.into_iter().map(Into::into).collect();
    // Orden estable para la UI: por nombre de producto y después por sucursal
    // (casa matriz primero). Se ordena acá y no en SurrealQL porque `ORDER BY`
    // sobre un alias derivado de un link no es fiable.
    out.sort_by(|a, b| {
        a.product_name
            .cmp(&b.product_name)
            .then_with(|| a.branch_name.cmp(&b.branch_name))
    });
    Ok(out)
}

/// On-hand actual de `product` en una sucursal (`None` = casa matriz). `0` si no
/// hay fila todavía (esa sucursal nunca tuvo movimiento de ese producto). Lo usa
/// el servicio para validar suficiencia antes de transferir y antes de vender.
pub async fn branch_stock_qty(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    branch: Option<&Thing>,
) -> DomainResult<i64> {
    let cond_branch = if branch.is_some() {
        "branch = $b"
    } else {
        "branch = NONE"
    };
    let q = format!(
        "SELECT stock FROM product_branch_stock \
         WHERE tenant = $t AND product = $p AND {cond_branch} LIMIT 1",
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("b", branch.cloned()))
        .await?
        .check()?;
    let stock: Option<i64> = r.take((0, "stock"))?;
    Ok(stock.unwrap_or(0))
}

/// On-hand de VARIOS productos en una sucursal, en una sola query. Lo usa el
/// pre-chequeo de la venta (una venta tiene N líneas y el hot path no puede
/// pagar N round-trips). Los productos sin fila no aparecen en el mapa → el
/// caller los trata como 0.
pub async fn branch_stock_qty_many(
    db: &Db,
    tenant: &Thing,
    products: &[Thing],
    branch: Option<&Thing>,
) -> DomainResult<std::collections::HashMap<String, i64>> {
    if products.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let cond_branch = if branch.is_some() {
        "branch = $b"
    } else {
        "branch = NONE"
    };
    #[derive(Deserialize)]
    struct Row {
        product: Thing,
        stock: i64,
    }
    let q = format!(
        "SELECT product, stock FROM product_branch_stock \
         WHERE tenant = $t AND product IN $ids AND {cond_branch}",
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("ids", products.to_vec()))
        .bind(("b", branch.cloned()))
        .await?
        .check()?;
    let rows: Vec<Row> = r.take(0)?;
    Ok(rows
        .into_iter()
        .map(|r| (r.product.to_string(), r.stock))
        .collect())
}

/// Reporte "stock por sucursal": una fila por producto con su desglose por local
/// y el total. El total es la suma del desglose, que por el invariante V2 es
/// `product.stock`.
pub async fn branch_stock_report(
    db: &Db,
    tenant: &Thing,
    f: &BranchStockFilters,
) -> DomainResult<Vec<BranchStockReportRow>> {
    let rows = list_branch_stock(db, tenant, f).await?;
    // Agrupar por producto preservando un orden estable (BTreeMap por nombre +
    // id, así dos productos homónimos no se pisan).
    let mut grouped: BTreeMap<(String, String), Vec<BranchStockSlice>> = BTreeMap::new();
    for r in rows {
        let key = (
            r.product_name.clone().unwrap_or_default(),
            r.product.clone(),
        );
        grouped.entry(key).or_default().push(BranchStockSlice {
            branch: r.branch,
            branch_name: r.branch_name,
            stock: r.stock,
        });
    }
    Ok(grouped
        .into_iter()
        .map(|((product_name, product), by_branch)| {
            let total = by_branch.iter().map(|s| s.stock).sum();
            BranchStockReportRow {
                product,
                product_name,
                by_branch,
                total,
            }
        })
        .collect())
}

// --- transferencia (atómica) -----------------------------------------------

/// Ids de los dos movimientos de auditoría emitidos por una transferencia.
pub struct TransferMovements {
    pub out_id: String,
    pub in_id: String,
}

/// Un lote que viaja en una transferencia (migración 0042). El lote no "se
/// mueve" de fila: se descuenta del lote de origen y se acumula en su espejo en
/// el destino (mismo `batch_code` y vencimiento), creándolo si el destino aún no
/// lo tenía. Dos filas porque el lote es FÍSICO: 30 cajas del lote L1 en el
/// local A y 10 en el B son dos existencias distintas, cada una con su FEFO.
pub struct TransferLotMove {
    /// Lote de origen a descontar.
    pub from_batch: Thing,
    /// Espejo en el destino si ya existe; `None` ⇒ se crea.
    pub to_batch: Option<Thing>,
    pub batch_code: String,
    pub expiry_date: DateTime<Utc>,
    pub cost: Option<rust_decimal::Decimal>,
    pub qty: i64,
}

fn dec_opt(d: Option<rust_decimal::Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(x) => surrealdb::sql::Number::from(x).into(),
        None => surrealdb::sql::Value::None,
    }
}

/// Emite los dos `stock_movement` de suma cero de una transferencia en UNA sola
/// transacción `BEGIN; … ; COMMIT;`:
/// * salida: `delta = -qty`, `reason = "transfer_out"`, `branch = from`,
/// * entrada: `delta = +qty`, `reason = "transfer_in"`, `branch = to`.
///
/// El evento `product_branch_stock_maint` aplica ambos deltas a sus buckets
/// dentro de la misma tx, así que al COMMIT el origen bajó y el destino subió
/// `qty`. La suma de deltas es cero ⇒ `product.stock` (y el invariante global) no
/// cambian: se mueve la distribución, no el total.
///
/// `lot_moves` (migración 0042) viaja en la MISMA transacción: si el producto
/// lleva lotes en el origen, la transferencia mueve las unidades lote por lote
/// (descuenta el de origen, acumula/crea su espejo en el destino). Sin esto la
/// cantidad llegaría al destino pero los lotes se quedarían en el origen, y el
/// FEFO del destino no encontraría nada que consumir: stock visible e
/// invendible, justo el dead-end que la vara de producto prohíbe. Va en la misma
/// tx que los movimientos porque `Σ product_batch.stock[X]` y
/// `product_branch_stock[X]` tienen que cruzar el COMMIT juntos.
///
/// El caller (servicio) ya validó: propiedad del tenant sobre el producto, que
/// sea físico, `qty > 0`, `from != to`, existencia de ambas sucursales y
/// suficiencia en el origen — todo bajo el lock por tenant.
#[allow(clippy::too_many_arguments)]
pub async fn apply_transfer(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    from: Option<&Thing>,
    to: Option<&Thing>,
    qty: i64,
    admin: Option<&Thing>,
    notes: Option<&str>,
    lot_moves: &[TransferLotMove],
) -> DomainResult<TransferMovements> {
    let ref_note = notes
        .map(str::to_string)
        .unwrap_or_else(|| "transferencia".to_string());
    let mut sql = String::from(
        "BEGIN; \
         CREATE stock_movement SET tenant = $t, product = $p, delta = $neg, \
             reason = 'transfer_out', admin = $a, ref = $ref, branch = $from \
             RETURN AFTER; \
         CREATE stock_movement SET tenant = $t, product = $p, delta = $pos, \
             reason = 'transfer_in', admin = $a, ref = $ref, branch = $to \
             RETURN AFTER; ",
    );
    for (i, m) in lot_moves.iter().enumerate() {
        sql.push_str(&format!(
            "UPDATE product_batch SET stock = stock - $lq{i} \
                WHERE id = $lfrom{i} AND tenant = $t; ",
        ));
        if m.to_batch.is_some() {
            sql.push_str(&format!(
                "UPDATE product_batch SET stock = stock + $lq{i}, active = true \
                    WHERE id = $lto{i} AND tenant = $t; ",
            ));
        } else {
            sql.push_str(&format!(
                "CREATE product_batch SET tenant = $t, product = $p, branch = $to, \
                    batch_code = $lc{i}, expiry_date = $le{i}, stock = $lq{i}, \
                    cost = $lcost{i}; ",
            ));
        }
    }
    sql.push_str("COMMIT;");

    let mut qb = db
        .query(sql)
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("neg", -qty))
        .bind(("pos", qty))
        .bind(("a", admin.cloned()))
        .bind(("ref", ref_note))
        .bind(("from", from.cloned()))
        .bind(("to", to.cloned()));
    for (i, m) in lot_moves.iter().enumerate() {
        qb = qb
            .bind((format!("lq{i}"), m.qty))
            .bind((format!("lfrom{i}"), m.from_batch.clone()))
            .bind((format!("lc{i}"), m.batch_code.clone()))
            .bind((
                format!("le{i}"),
                surrealdb::sql::Datetime::from(m.expiry_date),
            ))
            .bind((format!("lcost{i}"), dec_opt(m.cost)));
        if let Some(to_batch) = &m.to_batch {
            qb = qb.bind((format!("lto{i}"), to_batch.clone()));
        }
    }
    let mut r = qb.await?.check()?;

    #[derive(Deserialize)]
    struct IdRow {
        id: Thing,
    }
    let out: Option<IdRow> = r.take(0)?;
    let inc: Option<IdRow> = r.take(1)?;
    Ok(TransferMovements {
        out_id: out
            .ok_or_else(|| DomainError::Other(anyhow::anyhow!("transfer_out devolvió 0 filas")))?
            .id
            .to_string(),
        in_id: inc
            .ok_or_else(|| DomainError::Other(anyhow::anyhow!("transfer_in devolvió 0 filas")))?
            .id
            .to_string(),
    })
}
