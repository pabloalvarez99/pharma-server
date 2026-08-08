//! Libro de compras + resumen IVA (V3). Lecturas tenant-scoped sobre las OC
//! recepcionadas y las ventas del período.

use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::{IvaSummary, PurchaseBook, PurchaseBookRow};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// `Option<Decimal>` → valor SurrealDB (NONE cuando no viene), para bindear
/// montos sin pasar por JSON (que perdería precisión).
fn opt_dec(d: Option<Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(v) => surrealdb::sql::Number::from(v).into(),
        None => surrealdb::sql::Value::None,
    }
}

// Estados que entran a cada lado del cálculo. Van INLINE en las queries:
// SurrealDB no matchea `status IN $bind` con un array bindeado desde Rust (sí
// con el literal), y son constantes del dominio, no input del usuario.
//   compras: `received` | `partially_received` (una OC `draft`/`sent` todavía
//            no es un documento tributario)
//   ventas:  `paid` | `completed`

/// `YYYY-MM` → \[inicio, fin) del mes en UTC. Error en español si el formato no
/// calza (es un parámetro que escribe el operador).
pub fn period_bounds(period: &str) -> DomainResult<(DateTime<Utc>, DateTime<Utc>)> {
    let bad = || DomainError::Invalid(format!("período inválido: {period} (usa YYYY-MM)"));
    let (y, m) = period.split_once('-').ok_or_else(bad)?;
    let year: i32 = y.parse().map_err(|_| bad())?;
    let month: u32 = m.parse().map_err(|_| bad())?;
    if !(1..=12).contains(&month) {
        return Err(bad());
    }
    let start = Utc
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .ok_or_else(bad)?;
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = Utc
        .with_ymd_and_hms(ny, nm, 1, 0, 0, 0)
        .single()
        .ok_or_else(bad)?;
    Ok((start, end))
}

#[derive(Debug, Deserialize)]
struct PoRow {
    id: Thing,
    total: Decimal,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    invoice_folio: Option<String>,
    #[serde(default)]
    invoice_date: Option<DateTime<Utc>>,
    #[serde(default)]
    invoice_tipo: Option<i32>,
    #[serde(default)]
    invoice_neto: Option<Decimal>,
    #[serde(default)]
    invoice_iva: Option<Decimal>,
    #[serde(default)]
    invoice_total: Option<Decimal>,
    #[serde(default)]
    supplier_name: Option<String>,
    #[serde(default)]
    supplier_rut: Option<String>,
}

/// Libro de compras del período. Una OC entra por la fecha de su FACTURA cuando
/// está declarada (lo que exige el SII); si no, por su última actualización
/// (proxy de la recepción). Cuando el operador no declaró neto/IVA se derivan
/// del total a la tasa del tenant (19% por default) y la fila queda marcada
/// `declared = false`.
pub async fn purchase_book(db: &Db, tenant: &Thing, period: &str) -> DomainResult<PurchaseBook> {
    let (start, end) = period_bounds(period)?;
    let money = crate::settings::money_config(db, tenant).await?;
    let mut r = db
        .query(
            // `doc_date` = fecha de factura si el operador la capturó, si no la
            // última actualización (proxy de la recepción). Se aliasa porque
            // SurrealDB no acepta una expresión en ORDER BY.
            "SELECT id, total, updated_at, invoice_folio, invoice_date, invoice_tipo, \
             invoice_neto, invoice_iva, invoice_total, \
             supplier.name AS supplier_name, supplier.rut AS supplier_rut \
             FROM purchase_order \
             WHERE tenant = $t AND status IN ['received', 'partially_received'] \
             ORDER BY updated_at ASC",
        )
        .bind(("t", tenant.clone()))
        .await?;
    let rows: Vec<PoRow> = r.take(0)?;

    let mut out = Vec::with_capacity(rows.len());
    let mut total_neto = Decimal::ZERO;
    let mut total_iva = Decimal::ZERO;
    let mut total = Decimal::ZERO;
    let mut pending = 0usize;

    for row in rows {
        // Corte del período en Rust sobre la fecha efectiva (factura si fue
        // capturada, si no la recepción). Se filtra acá y no en SQL porque el
        // motor no evalúa la expresión de coalescencia dentro del WHERE de forma
        // fiable; el volumen de OC por tenant/mes es chico.
        let doc_date = row.invoice_date.unwrap_or(row.updated_at);
        if doc_date < start || doc_date >= end {
            continue;
        }
        let doc_total = row.invoice_total.unwrap_or(row.total);
        let (neto, iva, declared) = match (row.invoice_neto, row.invoice_iva) {
            (Some(n), Some(i)) => (n, i, true),
            _ => {
                let (n, i) = money.tax_breakdown(doc_total);
                (n, i, false)
            }
        };
        if !declared {
            pending += 1;
        }
        total_neto += neto;
        total_iva += iva;
        total += doc_total;
        out.push(PurchaseBookRow {
            purchase_order: row.id.to_string(),
            tipo: row.invoice_tipo.unwrap_or(33),
            folio: row.invoice_folio,
            supplier_name: row
                .supplier_name
                .unwrap_or_else(|| "(sin proveedor)".into()),
            supplier_rut: row.supplier_rut,
            date: doc_date,
            neto,
            iva,
            total: doc_total,
            declared,
        });
    }

    Ok(PurchaseBook {
        period: period.to_string(),
        rows: out,
        total_neto,
        total_iva,
        total,
        pending_declaration: pending,
    })
}

#[derive(Debug, Deserialize)]
struct SaleTotalRow {
    total: Decimal,
    created_at: DateTime<Utc>,
}

/// Resumen IVA del período: débito (ventas) − crédito (compras) = a pagar.
/// Las ventas del POS son IVA-incluido, así que el débito se desglosa del total.
pub async fn iva_summary(db: &Db, tenant: &Thing, period: &str) -> DomainResult<IvaSummary> {
    let (start, end) = period_bounds(period)?;
    let money = crate::settings::money_config(db, tenant).await?;
    let mut r = db
        .query(
            "SELECT total, created_at FROM order \
             WHERE tenant = $t AND status IN ['paid', 'completed']",
        )
        .bind(("t", tenant.clone()))
        .await?;
    let sales: Vec<SaleTotalRow> = r.take(0)?;

    // Corte del período en Rust, igual que el libro de compras (mismo motivo).
    let mut ventas_neto = Decimal::ZERO;
    let mut iva_debito = Decimal::ZERO;
    for s in sales {
        if s.created_at < start || s.created_at >= end {
            continue;
        }
        let (n, i) = money.tax_breakdown(s.total);
        ventas_neto += n;
        iva_debito += i;
    }

    let book = purchase_book(db, tenant, period).await?;

    Ok(IvaSummary {
        period: period.to_string(),
        iva_debito,
        iva_credito: book.total_iva,
        iva_a_pagar: iva_debito - book.total_iva,
        ventas_neto,
        compras_neto: book.total_neto,
    })
}

/// Capturar/actualizar los datos de la factura del proveedor sobre una OC.
/// Sólo se escriben los campos enviados (MERGE parcial).
pub async fn set_invoice(
    db: &Db,
    tenant: &Thing,
    po: &Thing,
    input: &super::model::InvoiceInput,
) -> DomainResult<()> {
    if input.folio.is_none()
        && input.tipo.is_none()
        && input.date.is_none()
        && input.neto.is_none()
        && input.iva.is_none()
        && input.total.is_none()
    {
        return Err(DomainError::Invalid(
            "no hay datos de factura para guardar".into(),
        ));
    }

    // Un solo SET con coalescencia por campo: lo no enviado conserva su valor.
    // (`MERGE` y `SET` no se combinan en un mismo UPDATE.)
    let mut r = db
        .query(
            "UPDATE purchase_order SET \
               invoice_folio = $folio ?? invoice_folio, \
               invoice_tipo  = $tipo  ?? invoice_tipo, \
               invoice_date  = $date  ?? invoice_date, \
               invoice_neto  = $neto  ?? invoice_neto, \
               invoice_iva   = $iva   ?? invoice_iva, \
               invoice_total = $total ?? invoice_total \
             WHERE id = $id AND tenant = $t RETURN id",
        )
        .bind((
            "folio",
            match input.folio.as_ref() {
                Some(v) => surrealdb::sql::Value::from(v.clone()),
                None => surrealdb::sql::Value::None,
            },
        ))
        .bind((
            "tipo",
            match input.tipo {
                Some(v) => surrealdb::sql::Value::from(v as i64),
                None => surrealdb::sql::Value::None,
            },
        ))
        .bind((
            "date",
            match input.date {
                Some(v) => surrealdb::sql::Value::from(v),
                None => surrealdb::sql::Value::None,
            },
        ))
        .bind(("neto", opt_dec(input.neto)))
        .bind(("iva", opt_dec(input.iva)))
        .bind(("total", opt_dec(input.total)))
        .bind(("id", po.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let id: Option<Thing> = r.take((0, "id"))?;
    id.map(|_| ()).ok_or(DomainError::NotFound)
}
