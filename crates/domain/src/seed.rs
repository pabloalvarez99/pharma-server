//! Demo-data seeding as a reusable service.
//!
//! Single source of truth for "llenar la farmacia/minimarket con datos de
//! ejemplo" — consumed by the CLI (`pharma seed-demo`) **and** the admin
//! endpoint (`POST /api/v1/admin/seed-demo`) so the in-app button and the
//! terminal seed identical data.
//!
//! ## Qué siembra (farmacia creíble al primer arranque)
//!
//! El objetivo es que una instalación fresca abra a una farmacia (o minimarket)
//! chilena verosímil, no a pantallas vacías:
//!
//! * **Catálogo** — SKUs reales del rubro con precios CLP plausibles, código de
//!   barra (EAN-13 prefijo GS1 Chile 780), lote + vencimiento escalonado (sanos,
//!   próximos a vencer, stock bajo) y, en farmacia, laboratorio + principio
//!   activo.
//! * **Proveedores** — ≥3 droguerías/distribuidoras con RUT, para que Compras y
//!   comparador de precios tengan con quién operar.
//! * **Órdenes de compra** — un par en estado `draft` (no mueven stock) para que
//!   la vista Compras no salga vacía.
//! * **Ventas históricas** — ventas pasadas repartidas en el último mes (vía
//!   [`crate::sales::historic`], que NO toca stock ni emite movimientos) para que
//!   Dashboard, sales-daily, top-products y márgenes tengan datos.
//!
//! ## Invariante de stock (no negociable)
//!
//! Cada producto se siembra con `stock = 0` y luego un **lote** con stock > 0,
//! que pasa por [`crate::inventory::service::create_batch`] → emite el
//! `stock_movement` (`reason = "batch_received"`) y materializa `product.stock`
//! en la misma transacción. Resultado: para cada producto sembrado
//! `product.stock == Σ product_batch.stock == Σ stock_movement.delta`. Nunca se
//! escribe `product.stock` directo (que rompería el ledger). Las ventas
//! históricas son post-hoc y deliberadamente NO descuentan stock.
//!
//! ## Marca DEMO + wipe
//!
//! Todo lo sembrado lleva una marca borrable: productos/lotes/movimientos via
//! `external_id = "DEMO-<slug>"`, órdenes de compra via `external_ref` con
//! prefijo `DEMO-PO-`, ventas históricas via `external_ref` con prefijo
//! `DEMO-SALE-`, y proveedores por su nombre (lista cerrada). El wipe (`force`)
//! borra exactamente esas filas del tenant — nunca toca data real del operador.
//! Sin `force`, si ya hay data demo se rechaza con [`DomainError::Conflict`]
//! (no duplica).
//!
//! NO se ejecuta solo: siempre lo dispara un humano (CLI o botón admin).

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use crate::catalog::model::NewProduct;
use crate::catalog::{repo as catalog_repo, service as catalog};
use crate::errors::{DomainError, DomainResult};
use crate::inventory::model::NewBatch;
use crate::inventory::service as inventory;
use crate::purchasing::model::{NewPurchaseOrder, NewPurchaseOrderItem, NewSupplier};
use crate::purchasing::service as purchasing;
use crate::sales::historic::{self, HistoricImportRequest, HistoricItemInput, HistoricOrderInput};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Prefijo del `external_id` que marca un producto como data demo borrable.
const DEMO_PREFIX: &str = "DEMO-";
/// Prefijo del `external_ref` de las órdenes de compra demo.
const DEMO_PO_PREFIX: &str = "DEMO-PO-";
/// Prefijo del `external_ref` de las ventas históricas demo.
const DEMO_SALE_PREFIX: &str = "DEMO-SALE-";

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
    /// Proveedores demo creados.
    pub suppliers_created: usize,
    /// Órdenes de compra demo creadas (estado `draft`).
    pub purchase_orders_created: usize,
    /// Ventas históricas demo creadas.
    pub historic_orders_created: usize,
    /// Productos demo previos borrados por `force` (0 si no hubo wipe).
    pub wiped: usize,
}

/// Un proveedor del pack demo.
struct DemoSupplier {
    name: &'static str,
    rut: &'static str,
    contact_name: &'static str,
    contact_phone: &'static str,
}

/// Un ítem del pack demo: producto + su lote inicial.
struct SeedItem {
    name: &'static str,
    /// EAN-13 (prefijo GS1 Chile 780). Sembrado en `product_barcode`.
    barcode: &'static str,
    price: i64,
    cost: i64,
    stock: i64,
    /// Días desde hoy hasta el vencimiento del lote.
    expiry_in_days: i64,
    batch_code: &'static str,
    /// Índice al proveedor del vertical (para órdenes de compra demo).
    supplier_idx: usize,
    laboratory: Option<&'static str>,
    active_ingredient: Option<&'static str>,
    presentation: Option<&'static str>,
}

/// Droguerías que abastecen la farmacia demo.
fn pharmacy_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Socofar S.A.",
            rut: "96.565.480-9",
            contact_name: "Mesa de pedidos Socofar",
            contact_phone: "+56 2 2510 7000",
        },
        DemoSupplier {
            name: "Droguería Hofmann S.A.",
            rut: "90.286.000-2",
            contact_name: "Ventas Hofmann",
            contact_phone: "+56 2 2620 1000",
        },
        DemoSupplier {
            name: "Difarma Ltda.",
            rut: "78.913.450-K",
            contact_name: "Pedidos Difarma",
            contact_phone: "+56 2 2733 4500",
        },
    ]
}

/// Distribuidoras que abastecen el minimarket demo.
fn minimarket_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Distribuidora Central Ltda.",
            rut: "77.456.120-3",
            contact_name: "Pedidos Central",
            contact_phone: "+56 2 2899 1200",
        },
        DemoSupplier {
            name: "Embotelladora Andina S.A.",
            rut: "91.144.000-8",
            contact_name: "Reparto Andina",
            contact_phone: "+56 2 2338 0000",
        },
        DemoSupplier {
            name: "Comercializadora Las Brisas",
            rut: "76.220.880-5",
            contact_name: "Ventas Las Brisas",
            contact_phone: "+56 9 7654 3210",
        },
    ]
}

/// Pack farmacia: fármacos con laboratorio + principio activo, vencimientos
/// escalonados (sanos / próximos a vencer / stock bajo) y proveedor asignado.
fn pharmacy_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Paracetamol 500mg x16",
            barcode: "7801234500016",
            price: 1290,
            cost: 700,
            stock: 120,
            expiry_in_days: 540,
            batch_code: "PARA-A1",
            supplier_idx: 0,
            laboratory: Some("Laboratorio Chile"),
            active_ingredient: Some("Paracetamol"),
            presentation: Some("16 comprimidos"),
        },
        SeedItem {
            name: "Ibuprofeno 400mg x20",
            barcode: "7801234500023",
            price: 2490,
            cost: 1300,
            stock: 80,
            expiry_in_days: 400,
            batch_code: "IBU-A1",
            supplier_idx: 1,
            laboratory: Some("Saval"),
            active_ingredient: Some("Ibuprofeno"),
            presentation: Some("20 comprimidos"),
        },
        SeedItem {
            name: "Amoxicilina 500mg x12",
            barcode: "7801234500030",
            price: 3990,
            cost: 2100,
            stock: 40,
            expiry_in_days: 300,
            batch_code: "AMOX-A1",
            supplier_idx: 0,
            laboratory: Some("Andrómaco"),
            active_ingredient: Some("Amoxicilina"),
            presentation: Some("12 cápsulas"),
        },
        SeedItem {
            name: "Loratadina 10mg x10",
            barcode: "7801234500047",
            price: 1890,
            cost: 900,
            stock: 60,
            expiry_in_days: 600,
            batch_code: "LORA-A1",
            supplier_idx: 2,
            laboratory: Some("Mintlab"),
            active_ingredient: Some("Loratadina"),
            presentation: Some("10 comprimidos"),
        },
        SeedItem {
            name: "Omeprazol 20mg x14",
            barcode: "7801234500054",
            price: 2790,
            cost: 1400,
            stock: 25,
            expiry_in_days: 25, // próximo a vencer
            batch_code: "OME-NEAR",
            supplier_idx: 1,
            laboratory: Some("Laboratorio Chile"),
            active_ingredient: Some("Omeprazol"),
            presentation: Some("14 cápsulas"),
        },
        SeedItem {
            name: "Suero Fisiológico 500ml",
            barcode: "7801234500061",
            price: 1490,
            cost: 800,
            stock: 3, // stock bajo
            expiry_in_days: 200,
            batch_code: "SF-LOW",
            supplier_idx: 2,
            laboratory: Some("Sanderson"),
            active_ingredient: Some("Cloruro de sodio 0.9%"),
            presentation: Some("Bolsa 500ml"),
        },
        SeedItem {
            name: "Aspirina 100mg x30",
            barcode: "7801234500078",
            price: 1690,
            cost: 850,
            stock: 95,
            expiry_in_days: 720,
            batch_code: "ASP-A1",
            supplier_idx: 0,
            laboratory: Some("Bayer"),
            active_ingredient: Some("Ácido acetilsalicílico"),
            presentation: Some("30 comprimidos"),
        },
        SeedItem {
            name: "Metformina 850mg x30",
            barcode: "7801234500085",
            price: 3490,
            cost: 1800,
            stock: 50,
            expiry_in_days: 480,
            batch_code: "MET-A1",
            supplier_idx: 1,
            laboratory: Some("Saval"),
            active_ingredient: Some("Metformina"),
            presentation: Some("30 comprimidos"),
        },
        SeedItem {
            name: "Losartán 50mg x30",
            barcode: "7801234500092",
            price: 4290,
            cost: 2300,
            stock: 38,
            expiry_in_days: 365,
            batch_code: "LOS-A1",
            supplier_idx: 2,
            laboratory: Some("Andrómaco"),
            active_ingredient: Some("Losartán potásico"),
            presentation: Some("30 comprimidos"),
        },
        SeedItem {
            name: "Salbutamol Inhalador 100mcg",
            barcode: "7801234500108",
            price: 5990,
            cost: 3400,
            stock: 18,
            expiry_in_days: 40, // próximo a vencer
            batch_code: "SALB-NEAR",
            supplier_idx: 0,
            laboratory: Some("Saval"),
            active_ingredient: Some("Salbutamol"),
            presentation: Some("Inhalador 200 dosis"),
        },
        SeedItem {
            name: "Clorfenamina 4mg x20",
            barcode: "7801234500115",
            price: 990,
            cost: 480,
            stock: 70,
            expiry_in_days: 540,
            batch_code: "CLOR-A1",
            supplier_idx: 1,
            laboratory: Some("Laboratorio Chile"),
            active_ingredient: Some("Clorfenamina maleato"),
            presentation: Some("20 comprimidos"),
        },
        SeedItem {
            name: "Alcohol Gel 250ml",
            barcode: "7801234500122",
            price: 1990,
            cost: 1100,
            stock: 4, // stock bajo
            expiry_in_days: 300,
            batch_code: "AGEL-LOW",
            supplier_idx: 2,
            laboratory: Some("Sanderson"),
            active_ingredient: Some("Etanol 70%"),
            presentation: Some("Frasco 250ml"),
        },
    ]
}

/// Pack minimarket: abarrotes/perecibles, sin campos clínicos pero CON
/// lote/vencimiento (pan, leche vencen igual que un fármaco). Vencimientos
/// escalonados + un par de stock bajo.
fn minimarket_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Pan de molde grande",
            barcode: "7802000100013",
            price: 2190,
            cost: 1300,
            stock: 30,
            expiry_in_days: 7, // perecible próximo a vencer
            batch_code: "PAN-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("500 g"),
        },
        SeedItem {
            name: "Leche entera 1L",
            barcode: "7802000100020",
            price: 1190,
            cost: 750,
            stock: 90,
            expiry_in_days: 20,
            batch_code: "LECHE-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 litro"),
        },
        SeedItem {
            name: "Bebida cola 1.5L",
            barcode: "7802000100037",
            price: 1690,
            cost: 980,
            stock: 120,
            expiry_in_days: 180,
            batch_code: "COLA-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1.5 litros"),
        },
        SeedItem {
            name: "Arroz grado 1 1kg",
            barcode: "7802000100044",
            price: 1490,
            cost: 900,
            stock: 200,
            expiry_in_days: 540,
            batch_code: "ARROZ-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 kg"),
        },
        SeedItem {
            name: "Huevos docena",
            barcode: "7802000100051",
            price: 2990,
            cost: 2000,
            stock: 4, // stock bajo
            expiry_in_days: 21,
            batch_code: "HUEVO-LOW",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("12 unidades"),
        },
        SeedItem {
            name: "Yogurt natural pack x4",
            barcode: "7802000100068",
            price: 2490,
            cost: 1500,
            stock: 18,
            expiry_in_days: 10, // perecible próximo a vencer
            batch_code: "YOG-NEAR",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("4 x 120 g"),
        },
        SeedItem {
            name: "Fideos spaghetti 400g",
            barcode: "7802000100075",
            price: 990,
            cost: 560,
            stock: 150,
            expiry_in_days: 600,
            batch_code: "FIDEO-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("400 g"),
        },
        SeedItem {
            name: "Aceite vegetal 1L",
            barcode: "7802000100082",
            price: 2790,
            cost: 1900,
            stock: 60,
            expiry_in_days: 365,
            batch_code: "ACEITE-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 litro"),
        },
        SeedItem {
            name: "Agua mineral 2L",
            barcode: "7802000100099",
            price: 1290,
            cost: 720,
            stock: 110,
            expiry_in_days: 270,
            batch_code: "AGUA-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("2 litros"),
        },
        SeedItem {
            name: "Café instantáneo 170g",
            barcode: "7802000100105",
            price: 4490,
            cost: 3100,
            stock: 5, // stock bajo
            expiry_in_days: 540,
            batch_code: "CAFE-LOW",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Frasco 170 g"),
        },
    ]
}

fn pack_for(v: SeedVertical) -> Vec<SeedItem> {
    match v {
        SeedVertical::Pharmacy => pharmacy_pack(),
        SeedVertical::Minimarket => minimarket_pack(),
    }
}

fn suppliers_for(v: SeedVertical) -> Vec<DemoSupplier> {
    match v {
        SeedVertical::Pharmacy => pharmacy_suppliers(),
        SeedVertical::Minimarket => minimarket_suppliers(),
    }
}

/// Nombres de TODOS los proveedores demo (ambos verticales) — la marca borrable
/// de proveedores es su nombre (no tienen `external_id`). El wipe los acota por
/// esta lista cerrada para nunca tocar un proveedor real del operador.
fn all_demo_supplier_names() -> Vec<String> {
    pharmacy_suppliers()
        .into_iter()
        .chain(minimarket_suppliers())
        .map(|s| s.name.to_string())
        .collect()
}

/// Slug estable del producto (sin el sufijo único que agrega el catálogo) para
/// componer el `external_id` demo.
fn demo_external_id(name: &str) -> String {
    format!("{DEMO_PREFIX}{}", catalog::slugify(name))
}

/// Ventas históricas demo: repartidas en el último mes, montos plausibles,
/// medios de pago variados. Referencian productos por su `external_id` demo —
/// `historic::import_historic_orders` NO descuenta stock ni emite movimientos,
/// así que esto puebla reportes sin romper el ledger.
fn demo_historic_orders(pack: &[SeedItem]) -> Vec<HistoricOrderInput> {
    // Rotación determinística: cada venta toma 1–3 ítems del pack. La cantidad y
    // el día varían para que sales-daily / top-products no salgan planos.
    let methods = ["pos_cash", "pos_debit", "pos_credit", "pos_cash"];
    let now = Utc::now();
    let mut orders = Vec::new();
    // 12 ventas en los últimos ~28 días.
    for n in 0..12usize {
        let days_ago = (n as i64 * 2) + 1; // 1,3,5,... días atrás
        let created_at = now - Duration::days(days_ago) - Duration::hours((n % 8) as i64);
        let item_count = 1 + (n % 3); // 1..=3 líneas
        let mut items = Vec::new();
        for k in 0..item_count {
            let idx = (n + k * 5) % pack.len();
            let it = &pack[idx];
            let quantity = 1 + ((n + k) % 3) as i64; // 1..=3
            items.push(HistoricItemInput {
                external_id: demo_external_id(it.name),
                quantity,
                unit_price: Decimal::from(it.price),
            });
        }
        orders.push(HistoricOrderInput {
            created_at,
            items,
            total: None,
            payment_method: methods[n % methods.len()].to_string(),
            external_ref: Some(format!("{DEMO_SALE_PREFIX}{n:03}")),
        });
    }
    orders
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

    // 1) Proveedores demo.
    let mut supplier_ids: Vec<String> = Vec::new();
    for s in suppliers_for(v) {
        let dto = purchasing::create_supplier(
            db,
            tenant,
            NewSupplier {
                name: s.name.to_string(),
                rut: Some(s.rut.to_string()),
                contact_name: Some(s.contact_name.to_string()),
                contact_email: None,
                contact_phone: Some(s.contact_phone.to_string()),
                default_invoice_format: None,
            },
        )
        .await?;
        supplier_ids.push(dto.id);
    }
    let suppliers_created = supplier_ids.len();

    // 2) Catálogo: producto (stock 0) → lote (stock>0, emite movimiento) →
    //    código de barra. Guardamos (external_id, product_id, supplier_idx,
    //    cost) para componer las órdenes de compra demo.
    let pack = pack_for(v);
    let mut products_created = 0usize;
    let mut batches_created = 0usize;
    let mut movements_emitted = 0usize;
    let mut catalogued: Vec<(String, i64, usize)> = Vec::new(); // (product_id, cost, supplier_idx)

    for item in &pack {
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

        // Código de barra (EAN-13) → product_barcode (lo lee el scan del POS).
        let product_thing = surrealdb::sql::thing(&product.id)
            .map_err(|e| DomainError::Other(anyhow::anyhow!("product id parse: {e}")))?;
        catalog_repo::upsert_barcode(db, tenant, &product_thing, item.barcode).await?;

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

        catalogued.push((product.id, item.cost, item.supplier_idx));
    }

    // 3) Órdenes de compra demo (estado `draft` — NO mueven stock). Dos OC, cada
    //    una con líneas de un proveedor distinto, para poblar la vista Compras.
    let purchase_orders_created =
        seed_purchase_orders(db, tenant, &supplier_ids, &pack, &catalogued).await?;

    // 4) Ventas históricas (post-hoc; NO descuentan stock) → pueblan Dashboard,
    //    sales-daily, top-products y márgenes.
    let report = historic::import_historic_orders(
        db,
        tenant,
        None,
        Some("Demo"),
        HistoricImportRequest {
            orders: demo_historic_orders(&pack),
        },
    )
    .await?;
    let historic_orders_created = report.created as usize;

    Ok(SeedSummary {
        vertical: v.label().to_string(),
        products_created,
        batches_created,
        movements_emitted,
        suppliers_created,
        purchase_orders_created,
        historic_orders_created,
        wiped,
    })
}

/// Crea hasta dos órdenes de compra demo en estado `draft` (no recibidas, sin
/// impacto de stock). Cada OC agrupa líneas catalogadas de un mismo proveedor.
async fn seed_purchase_orders(
    db: &Db,
    tenant: &Thing,
    supplier_ids: &[String],
    pack: &[SeedItem],
    catalogued: &[(String, i64, usize)],
) -> DomainResult<usize> {
    if supplier_ids.is_empty() || catalogued.is_empty() {
        return Ok(0);
    }
    let mut created = 0usize;
    // Hasta 2 OC, una por proveedor 0 y 1 (los que existan).
    for (po_n, sup_idx) in [0usize, 1usize].into_iter().enumerate() {
        if sup_idx >= supplier_ids.len() {
            break;
        }
        let items: Vec<NewPurchaseOrderItem> = catalogued
            .iter()
            .enumerate()
            .filter(|(_, (_, _, s))| *s == sup_idx)
            .take(3)
            .map(|(i, (pid, cost, _))| NewPurchaseOrderItem {
                product: Some(pid.clone()),
                product_name: pack[i].name.to_string(),
                quantity: 24,
                unit_cost: Decimal::from(*cost),
            })
            .collect();
        if items.is_empty() {
            continue;
        }
        purchasing::create_purchase_order(
            db,
            tenant,
            NewPurchaseOrder {
                supplier: supplier_ids[sup_idx].clone(),
                currency: None,
                notes: Some("Orden de compra demo".to_string()),
                external_ref: Some(format!("{DEMO_PO_PREFIX}{po_n:03}")),
                items,
            },
        )
        .await?;
        created += 1;
    }
    Ok(created)
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

/// Ids de filas con `external_ref` que empieza con `prefix` en `table`.
async fn ids_by_external_ref_prefix(
    db: &Db,
    tenant: &Thing,
    table: &str,
    prefix: &str,
) -> DomainResult<Vec<Thing>> {
    let q = format!(
        "SELECT VALUE id FROM {table} \
         WHERE tenant = $t AND external_ref != NONE \
         AND string::starts_with(external_ref, $p)"
    );
    let ids: Vec<Thing> = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("p", prefix.to_string()))
        .await?
        .take(0)?;
    Ok(ids)
}

/// Borra TODA la data demo del tenant: ventas históricas, órdenes de compra,
/// proveedores, lotes/movimientos/códigos de barra y los productos. `ids` ya
/// está acotado al tenant + marca demo (productos). El resto se acota por su
/// propia marca (external_ref prefijado / lista de nombres) — nunca toca data
/// real del operador.
async fn wipe_demo(db: &Db, tenant: &Thing, ids: &[Thing]) -> DomainResult<()> {
    // Ventas históricas demo (order + order_item).
    let order_ids = ids_by_external_ref_prefix(db, tenant, "order", DEMO_SALE_PREFIX).await?;
    if !order_ids.is_empty() {
        db.query("DELETE order_item WHERE tenant = $t AND order IN $ids")
            .bind(("t", tenant.clone()))
            .bind(("ids", order_ids.clone()))
            .await?;
        db.query("DELETE order WHERE id IN $ids")
            .bind(("ids", order_ids))
            .await?;
    }

    // Órdenes de compra demo (purchase_order + purchase_order_item).
    let po_ids = ids_by_external_ref_prefix(db, tenant, "purchase_order", DEMO_PO_PREFIX).await?;
    if !po_ids.is_empty() {
        db.query("DELETE purchase_order_item WHERE tenant = $t AND purchase_order IN $ids")
            .bind(("t", tenant.clone()))
            .bind(("ids", po_ids.clone()))
            .await?;
        db.query("DELETE purchase_order WHERE id IN $ids")
            .bind(("ids", po_ids))
            .await?;
    }

    // Proveedores demo (acotados por la lista cerrada de nombres).
    db.query("DELETE supplier WHERE tenant = $t AND name IN $names")
        .bind(("t", tenant.clone()))
        .bind(("names", all_demo_supplier_names()))
        .await?;

    // Lotes, movimientos y códigos de barra de los productos demo (hijos antes
    // que el producto).
    db.query("DELETE stock_movement WHERE tenant = $t AND product IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await?;
    db.query("DELETE product_batch WHERE tenant = $t AND product IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await?;
    db.query("DELETE product_barcode WHERE tenant = $t AND product IN $ids")
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

    #[test]
    fn packs_are_believable_size_with_unique_barcodes() {
        for pack in [pharmacy_pack(), minimarket_pack()] {
            assert!(pack.len() >= 10, "catálogo demo debe ser creíble (≥10)");
            // Códigos de barra EAN-13 únicos dentro del pack.
            let mut codes: Vec<&str> = pack.iter().map(|i| i.barcode).collect();
            let n = codes.len();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(codes.len(), n, "barcodes deben ser únicos");
            for c in &codes {
                assert_eq!(c.len(), 13, "EAN-13");
                assert!(c.chars().all(|d| d.is_ascii_digit()));
            }
        }
    }

    #[test]
    fn every_item_points_to_a_real_supplier_index() {
        for v in [SeedVertical::Pharmacy, SeedVertical::Minimarket] {
            let sups = suppliers_for(v);
            assert!(sups.len() >= 3, "≥3 proveedores por vertical");
            for item in pack_for(v) {
                assert!(
                    item.supplier_idx < sups.len(),
                    "{} apunta a proveedor inexistente",
                    item.name
                );
            }
        }
    }

    #[test]
    fn historic_orders_reference_demo_externals_only() {
        let pack = pharmacy_pack();
        let orders = demo_historic_orders(&pack);
        assert!(orders.len() >= 6, "histórico debe poblar reportes");
        let valid: std::collections::HashSet<String> =
            pack.iter().map(|i| demo_external_id(i.name)).collect();
        for o in &orders {
            assert!(!o.items.is_empty());
            assert!(o
                .external_ref
                .as_deref()
                .unwrap()
                .starts_with(DEMO_SALE_PREFIX));
            for it in &o.items {
                assert!(valid.contains(&it.external_id), "external_id desconocido");
                assert!(it.quantity > 0);
            }
        }
    }
}
