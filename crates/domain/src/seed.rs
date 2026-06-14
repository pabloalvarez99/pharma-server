//! Demo-data seeding as a reusable service.
//!
//! Single source of truth for "llenar la farmacia/minimarket con datos de
//! ejemplo" — consumed by the CLI (`pharma seed-demo`) **and** the admin
//! endpoint (`POST /api/v1/admin/seed-demo`) so the in-app button and the
//! terminal seed identical data.
//!
//! ## Invariante de stock (no negociable)
//!
//! Cada producto se siembra con `stock = 0` y luego un **lote** con stock > 0,
//! que pasa por [`crate::inventory::service::create_batch`] → emite el
//! `stock_movement` (`reason = "batch_received"`) y materializa `product.stock`
//! en la misma transacción. Resultado: para cada producto sembrado
//! `product.stock == Σ product_batch.stock == Σ stock_movement.delta`. Nunca se
//! escribe `product.stock` directo (que rompería el ledger).
//!
//! ## Marca DEMO + wipe
//!
//! Todo producto sembrado lleva `external_id = "DEMO-<slug>"`. El wipe (`force`)
//! borra exactamente esas filas (y sus lotes + movimientos) del tenant — nunca
//! toca data real del operador. Sin `force`, si ya hay data demo se rechaza con
//! [`DomainError::Conflict`] (no duplica).
//!
//! NO se ejecuta solo: siempre lo dispara un humano (CLI o botón admin).

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use crate::catalog::model::NewProduct;
use crate::catalog::service as catalog;
use crate::errors::{DomainError, DomainResult};
use crate::inventory::model::NewBatch;
use crate::inventory::service as inventory;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Prefijo del `external_id` que marca una fila como data demo borrable.
const DEMO_PREFIX: &str = "DEMO-";

/// Vertical de negocio para elegir el pack de datos demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedVertical {
    Pharmacy,
    Minimarket,
}

impl SeedVertical {
    /// Parsea el vertical desde el string del CLI/endpoint. Acepta sinónimos
    /// en español. Desconocido → `DomainError::Invalid`.
    pub fn parse(s: &str) -> DomainResult<Self> {
        match s.trim().to_lowercase().as_str() {
            "pharmacy" | "farmacia" | "" => Ok(Self::Pharmacy), // default = farmacia
            "minimarket" | "general" | "almacen" | "almacén" | "market" => Ok(Self::Minimarket),
            other => Err(DomainError::Invalid(format!(
                "vertical desconocido: «{other}» (use pharmacy|minimarket)"
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pharmacy => "pharmacy",
            Self::Minimarket => "minimarket",
        }
    }
}

/// Resultado de una corrida de seeding (respuesta del endpoint + log del CLI).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeedSummary {
    pub vertical: String,
    /// Productos demo creados en esta corrida.
    pub products_created: usize,
    /// Lotes creados (uno por producto).
    pub batches_created: usize,
    /// Movimientos de stock emitidos (uno por lote con stock > 0).
    pub movements_emitted: usize,
    /// Filas demo previas borradas por `force` (0 si no hubo wipe).
    pub wiped: usize,
}

/// Un ítem del pack demo: producto + su lote inicial.
struct SeedItem {
    name: &'static str,
    price: i64,
    cost: i64,
    stock: i64,
    /// Días desde hoy hasta el vencimiento del lote.
    expiry_in_days: i64,
    batch_code: &'static str,
    laboratory: Option<&'static str>,
    active_ingredient: Option<&'static str>,
    presentation: Option<&'static str>,
}

/// Pack farmacia: fármacos con laboratorio + principio activo.
fn pharmacy_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Paracetamol 500mg x16",
            price: 1290,
            cost: 700,
            stock: 120,
            expiry_in_days: 540,
            batch_code: "PARA-A1",
            laboratory: Some("Laboratorio Chile"),
            active_ingredient: Some("Paracetamol"),
            presentation: Some("16 comprimidos"),
        },
        SeedItem {
            name: "Ibuprofeno 400mg x20",
            price: 2490,
            cost: 1300,
            stock: 80,
            expiry_in_days: 400,
            batch_code: "IBU-A1",
            laboratory: Some("Saval"),
            active_ingredient: Some("Ibuprofeno"),
            presentation: Some("20 comprimidos"),
        },
        SeedItem {
            name: "Amoxicilina 500mg x12",
            price: 3990,
            cost: 2100,
            stock: 40,
            expiry_in_days: 300,
            batch_code: "AMOX-A1",
            laboratory: Some("Andrómaco"),
            active_ingredient: Some("Amoxicilina"),
            presentation: Some("12 cápsulas"),
        },
        SeedItem {
            name: "Loratadina 10mg x10",
            price: 1890,
            cost: 900,
            stock: 60,
            expiry_in_days: 600,
            batch_code: "LORA-A1",
            laboratory: Some("Mintlab"),
            active_ingredient: Some("Loratadina"),
            presentation: Some("10 comprimidos"),
        },
        SeedItem {
            name: "Omeprazol 20mg x14",
            price: 2790,
            cost: 1400,
            stock: 25,
            expiry_in_days: 25,
            batch_code: "OME-NEAR",
            laboratory: Some("Laboratorio Chile"),
            active_ingredient: Some("Omeprazol"),
            presentation: Some("14 cápsulas"),
        },
        SeedItem {
            name: "Suero Fisiológico 500ml",
            price: 1490,
            cost: 800,
            stock: 3,
            expiry_in_days: 200,
            batch_code: "SF-LOW",
            laboratory: Some("Sanderson"),
            active_ingredient: Some("Cloruro de sodio 0.9%"),
            presentation: Some("Bolsa 500ml"),
        },
    ]
}

/// Pack minimarket: abarrotes/perecibles, sin campos clínicos pero CON
/// lote/vencimiento (pan, leche vencen igual que un fármaco).
fn minimarket_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Pan de molde grande",
            price: 2190,
            cost: 1300,
            stock: 30,
            expiry_in_days: 7,
            batch_code: "PAN-LOTE",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("500 g"),
        },
        SeedItem {
            name: "Leche entera 1L",
            price: 1190,
            cost: 750,
            stock: 90,
            expiry_in_days: 20,
            batch_code: "LECHE-LOTE",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 litro"),
        },
        SeedItem {
            name: "Bebida cola 1.5L",
            price: 1690,
            cost: 980,
            stock: 120,
            expiry_in_days: 180,
            batch_code: "COLA-LOTE",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1.5 litros"),
        },
        SeedItem {
            name: "Arroz grado 1 1kg",
            price: 1490,
            cost: 900,
            stock: 200,
            expiry_in_days: 540,
            batch_code: "ARROZ-LOTE",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 kg"),
        },
        SeedItem {
            name: "Huevos docena",
            price: 2990,
            cost: 2000,
            stock: 4,
            expiry_in_days: 21,
            batch_code: "HUEVO-LOW",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("12 unidades"),
        },
        SeedItem {
            name: "Yogurt natural pack x4",
            price: 2490,
            cost: 1500,
            stock: 18,
            expiry_in_days: 10,
            batch_code: "YOG-NEAR",
            laboratory: None,
            active_ingredient: None,
            presentation: Some("4 x 120 g"),
        },
    ]
}

fn pack_for(v: SeedVertical) -> Vec<SeedItem> {
    match v {
        SeedVertical::Pharmacy => pharmacy_pack(),
        SeedVertical::Minimarket => minimarket_pack(),
    }
}

/// Slug estable del producto (sin el sufijo único que agrega el catálogo) para
/// componer el `external_id` demo.
fn demo_external_id(name: &str) -> String {
    format!("{DEMO_PREFIX}{}", catalog::slugify(name))
}

/// Siembra data demo del `vertical` en el `tenant`. Ver el doc del módulo para
/// el invariante de stock y la semántica de `force`.
pub async fn seed_demo(
    db: &Db,
    tenant: &Thing,
    vertical: &str,
    force: bool,
) -> DomainResult<SeedSummary> {
    let v = SeedVertical::parse(vertical)?;

    // Idempotencia: data demo previa → wipe (force) o rechazo.
    let existing = demo_product_ids(db, tenant).await?;
    let wiped = if existing.is_empty() {
        0
    } else if force {
        wipe_demo(db, tenant, &existing).await?;
        existing.len()
    } else {
        return Err(DomainError::Conflict(
            "ya existe data demo en este tenant; use force=true para regenerarla".into(),
        ));
    };

    let mut products_created = 0usize;
    let mut batches_created = 0usize;
    let mut movements_emitted = 0usize;

    for item in pack_for(v) {
        let product = catalog::create_product(
            db,
            tenant,
            NewProduct {
                name: item.name.to_string(),
                slug: None,
                description: None,
                price: Decimal::from(item.price),
                cost_price: Some(Decimal::from(item.cost)),
                stock: 0, // el stock entra por el lote → emite movimiento
                category: None,
                image_url: None,
                external_id: Some(demo_external_id(item.name)),
                laboratory: item.laboratory.map(str::to_string),
                therapeutic_action: None,
                active_ingredient: item.active_ingredient.map(str::to_string),
                prescription_type: None,
                presentation: item.presentation.map(str::to_string),
                discount_percent: None,
            },
        )
        .await?;
        products_created += 1;

        let expiry = Utc::now() + Duration::days(item.expiry_in_days);
        inventory::create_batch(
            db,
            tenant,
            NewBatch {
                product: product.id.clone(),
                batch_code: item.batch_code.to_string(),
                expiry_date: expiry,
                stock: item.stock,
                cost: Some(Decimal::from(item.cost)),
                notes: Some("Lote demo".to_string()),
            },
            None,
        )
        .await?;
        batches_created += 1;
        if item.stock > 0 {
            movements_emitted += 1;
        }
    }

    Ok(SeedSummary {
        vertical: v.label().to_string(),
        products_created,
        batches_created,
        movements_emitted,
        wiped,
    })
}

/// Ids de los productos marcados como demo en el tenant.
async fn demo_product_ids(db: &Db, tenant: &Thing) -> DomainResult<Vec<Thing>> {
    let mut r = db
        .query(
            "SELECT VALUE id FROM product \
             WHERE tenant = $t AND external_id != NONE \
             AND string::starts_with(external_id, $p)",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", DEMO_PREFIX.to_string()))
        .await?;
    let ids: Vec<Thing> = r.take(0)?;
    Ok(ids)
}

/// Borra los lotes, movimientos y productos demo del tenant (en ese orden:
/// hijos antes que el producto). `ids` ya está acotado al tenant + marca demo.
async fn wipe_demo(db: &Db, tenant: &Thing, ids: &[Thing]) -> DomainResult<()> {
    db.query("DELETE stock_movement WHERE tenant = $t AND product IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await?;
    db.query("DELETE product_batch WHERE tenant = $t AND product IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await?;
    db.query("DELETE product WHERE id IN $ids")
        .bind(("ids", ids.to_vec()))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vertical_accepts_synonyms_and_default() {
        assert_eq!(
            SeedVertical::parse("pharmacy").unwrap(),
            SeedVertical::Pharmacy
        );
        assert_eq!(
            SeedVertical::parse("Farmacia").unwrap(),
            SeedVertical::Pharmacy
        );
        assert_eq!(SeedVertical::parse("").unwrap(), SeedVertical::Pharmacy);
        assert_eq!(
            SeedVertical::parse("minimarket").unwrap(),
            SeedVertical::Minimarket
        );
        assert_eq!(
            SeedVertical::parse("  General ").unwrap(),
            SeedVertical::Minimarket
        );
        assert!(SeedVertical::parse("casino").is_err());
    }

    #[test]
    fn demo_external_id_is_prefixed_slug() {
        assert_eq!(
            demo_external_id("Paracetamol 500mg x16"),
            "DEMO-paracetamol-500mg-x16"
        );
    }

    #[test]
    fn minimarket_pack_has_no_clinical_fields_but_keeps_batches() {
        for item in minimarket_pack() {
            assert!(
                item.laboratory.is_none(),
                "{} no debe tener laboratorio",
                item.name
            );
            assert!(item.active_ingredient.is_none());
            assert!(!item.batch_code.is_empty(), "perecible necesita lote");
            assert!(item.stock > 0);
        }
    }

    #[test]
    fn pharmacy_pack_carries_clinical_fields() {
        let pack = pharmacy_pack();
        assert!(pack.iter().all(|i| i.active_ingredient.is_some()));
        assert!(pack.iter().any(|i| i.laboratory.is_some()));
    }
}
