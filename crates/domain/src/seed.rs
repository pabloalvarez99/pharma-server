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
/// Vencimiento (días) de un bien no perecible (retail): lejano a propósito para
/// que el lote exista por el invariante del ledger pero nunca dispare near-expiry.
const SHELF_STABLE_DAYS: i64 = 1825; // ~5 años

/// Vertical de negocio para elegir el pack de datos demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedVertical {
    Pharmacy,
    Minimarket,
    /// Café / pastelería: perecibles con lote + vencimiento corto.
    Cafe,
    /// Tienda / retail: bienes no perecibles, sin clínica. El stock igual entra
    /// por lote (invariante del ledger) pero con vencimiento lejano → nunca cae
    /// en near-expiry. Coherente con `featuresForRubro("tienda").lotes = false`.
    Tienda,
    /// Belleza / servicios: el core agnóstico vendiendo un **servicio** (corte,
    /// manicure, color…), NO un bien físico. Se siembra con
    /// `physical_stock = false`, stock 0 y SIN lote: la venta salta el chequeo de
    /// stock y el plan FEFO (migración 0031), así el servicio se vende N veces sin
    /// "agotarse" y sin emitir movimientos de inventario.
    Servicios,
    /// Restaurant / comida preparada: rubro **mixto** que prueba el core con
    /// ambos modos a la vez. Los **insumos** (harina, aceite, carne) son bienes
    /// físicos con lote + vencimiento (stock entra por lote → emite movimiento).
    /// Los **platos preparados** (lomo a lo pobre, empanada) se venden SIN
    /// descontar inventario: se siembran `physical_stock = false`, stock 0 y SIN
    /// lote (`batch_code` vacío) — como un servicio, pero conviviendo con
    /// insumos físicos en el mismo catálogo. La señal por-ítem de "es físico" es
    /// `batch_code` no vacío.
    Restaurant,
}

impl SeedVertical {
    /// Parsea el vertical desde el string del CLI/endpoint. Acepta sinónimos
    /// en español. Desconocido → `DomainError::Invalid`.
    pub fn parse(s: &str) -> DomainResult<Self> {
        match s.trim().to_lowercase().as_str() {
            "pharmacy" | "farmacia" | "" => Ok(Self::Pharmacy), // default = farmacia
            "minimarket" | "general" | "almacen" | "almacén" | "market" => Ok(Self::Minimarket),
            "cafe" | "café" | "pasteleria" | "pastelería" | "coffee" => Ok(Self::Cafe),
            "tienda" | "retail" | "store" | "boutique" => Ok(Self::Tienda),
            "servicios" | "belleza" | "peluqueria" | "peluquería" | "salon" | "salón"
            | "estetica" | "estética" | "barberia" | "barbería" => Ok(Self::Servicios),
            "restaurant" | "restaurante" | "restorant" | "comida" | "food" | "cocina" => {
                Ok(Self::Restaurant)
            }
            other => Err(DomainError::Invalid(format!(
                "vertical desconocido: «{other}» (use pharmacy|minimarket|cafe|tienda|servicios|restaurant)"
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pharmacy => "pharmacy",
            Self::Minimarket => "minimarket",
            Self::Cafe => "cafe",
            Self::Tienda => "tienda",
            Self::Servicios => "servicios",
            Self::Restaurant => "restaurant",
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

/// Proveedores que abastecen el café/pastelería demo.
fn cafe_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Tostaduría Andina Café",
            rut: "76.901.330-7",
            contact_name: "Pedidos Tostaduría",
            contact_phone: "+56 2 2987 4400",
        },
        DemoSupplier {
            name: "Panificadora San Camilo",
            rut: "92.045.000-6",
            contact_name: "Ventas San Camilo",
            contact_phone: "+56 2 2555 8800",
        },
        DemoSupplier {
            name: "Insumos Pastelería del Valle",
            rut: "77.812.640-1",
            contact_name: "Reparto del Valle",
            contact_phone: "+56 9 8123 4567",
        },
    ]
}

/// Proveedores que abastecen la tienda/retail demo.
fn tienda_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Importadora Textil Pacífico",
            rut: "76.334.210-4",
            contact_name: "Ventas Pacífico",
            contact_phone: "+56 2 2640 3300",
        },
        DemoSupplier {
            name: "Distribuidora Electrónica Maipú",
            rut: "77.998.120-9",
            contact_name: "Pedidos Maipú",
            contact_phone: "+56 2 2712 9000",
        },
        DemoSupplier {
            name: "Mayorista Librería Norte",
            rut: "78.221.770-K",
            contact_name: "Ventas Librería Norte",
            contact_phone: "+56 9 7012 3456",
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

/// Pack café/pastelería: granos, leche, pastelería y sándwiches. Perecibles con
/// lote + vencimiento corto (igual que minimarket), sin campos clínicos. Un par
/// de stock bajo + un par próximos a vencer para que las alertas tengan datos.
fn cafe_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Café en grano 1kg",
            barcode: "7803000100012",
            price: 12990,
            cost: 8200,
            stock: 40,
            expiry_in_days: 240,
            batch_code: "GRANO-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 kg"),
        },
        SeedItem {
            name: "Leche entera 1L",
            barcode: "7803000100029",
            price: 1190,
            cost: 760,
            stock: 70,
            expiry_in_days: 18,
            batch_code: "LECHE-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 litro"),
        },
        SeedItem {
            name: "Croissant mantequilla",
            barcode: "7803000100036",
            price: 1490,
            cost: 700,
            stock: 36,
            expiry_in_days: 3, // pastelería perecible próxima a vencer
            batch_code: "CROIS-NEAR",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Unidad 70 g"),
        },
        SeedItem {
            name: "Torta de chocolate (porción)",
            barcode: "7803000100043",
            price: 3490,
            cost: 1800,
            stock: 12,
            expiry_in_days: 4,
            batch_code: "TORTA-NEAR",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Porción 120 g"),
        },
        SeedItem {
            name: "Muffin arándano",
            barcode: "7803000100050",
            price: 1890,
            cost: 900,
            stock: 5, // stock bajo
            expiry_in_days: 5,
            batch_code: "MUFFIN-LOW",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Unidad 90 g"),
        },
        SeedItem {
            name: "Jugo natural naranja 350ml",
            barcode: "7803000100067",
            price: 2290,
            cost: 1200,
            stock: 28,
            expiry_in_days: 6,
            batch_code: "JUGO-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("350 ml"),
        },
        SeedItem {
            name: "Té en bolsitas x20",
            barcode: "7803000100074",
            price: 2490,
            cost: 1400,
            stock: 50,
            expiry_in_days: 540,
            batch_code: "TE-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("20 bolsitas"),
        },
        SeedItem {
            name: "Azúcar blanca 1kg",
            barcode: "7803000100081",
            price: 1290,
            cost: 800,
            stock: 60,
            expiry_in_days: 600,
            batch_code: "AZUCAR-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("1 kg"),
        },
        SeedItem {
            name: "Vaso cartón 12oz x50",
            barcode: "7803000100098",
            price: 4990,
            cost: 3100,
            stock: 24,
            expiry_in_days: 720,
            batch_code: "VASO-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("50 unidades"),
        },
        SeedItem {
            name: "Sándwich jamón queso",
            barcode: "7803000100104",
            price: 2990,
            cost: 1500,
            stock: 4,          // stock bajo
            expiry_in_days: 2, // perecible próximo a vencer
            batch_code: "SAND-LOW",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Unidad 180 g"),
        },
        SeedItem {
            name: "Chocolate caliente sobre 30g",
            barcode: "7803000100111",
            price: 990,
            cost: 520,
            stock: 80,
            expiry_in_days: 480,
            batch_code: "CHOCO-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Sobre 30 g"),
        },
    ]
}

/// Pack tienda/retail: bienes durables (vestuario, librería, electrónica menor).
/// Sin campos clínicos y NO perecibles → vencimiento lejano (`SHELF_STABLE_DAYS`)
/// para que nunca caigan en near-expiry; el lote existe sólo por el invariante
/// del ledger (stock entra por lote). Un par de stock bajo para alertas.
fn tienda_pack() -> Vec<SeedItem> {
    vec![
        SeedItem {
            name: "Polera algodón básica",
            barcode: "7804000100011",
            price: 7990,
            cost: 3800,
            stock: 45,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "POLERA-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Talla M"),
        },
        SeedItem {
            name: "Jeans hombre azul",
            barcode: "7804000100028",
            price: 19990,
            cost: 11000,
            stock: 22,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "JEANS-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Talla 42"),
        },
        SeedItem {
            name: "Calcetines pack x3",
            barcode: "7804000100035",
            price: 4990,
            cost: 2300,
            stock: 60,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "CALC-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("3 pares"),
        },
        SeedItem {
            name: "Cuaderno universitario 100h",
            barcode: "7804000100042",
            price: 2490,
            cost: 1200,
            stock: 120,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "CUAD-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("100 hojas"),
        },
        SeedItem {
            name: "Lápiz pasta azul x10",
            barcode: "7804000100059",
            price: 2990,
            cost: 1400,
            stock: 80,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "LAPIZ-LOTE",
            supplier_idx: 2,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("10 unidades"),
        },
        SeedItem {
            name: "Mochila escolar",
            barcode: "7804000100066",
            price: 14990,
            cost: 8000,
            stock: 3, // stock bajo
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "MOCHILA-LOW",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("22 litros"),
        },
        SeedItem {
            name: "Audífonos in-ear",
            barcode: "7804000100073",
            price: 9990,
            cost: 5200,
            stock: 30,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "AUDIO-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Con micrófono"),
        },
        SeedItem {
            name: "Cargador USB-C 20W",
            barcode: "7804000100080",
            price: 12990,
            cost: 7000,
            stock: 25,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "CARGA-LOTE",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("20 W"),
        },
        SeedItem {
            name: "Pilas AA pack x4",
            barcode: "7804000100097",
            price: 3490,
            cost: 1800,
            stock: 5, // stock bajo
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "PILAS-LOW",
            supplier_idx: 1,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("4 unidades"),
        },
        SeedItem {
            name: "Gorro lana invierno",
            barcode: "7804000100103",
            price: 5990,
            cost: 2800,
            stock: 38,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "GORRO-LOTE",
            supplier_idx: 0,
            laboratory: None,
            active_ingredient: None,
            presentation: Some("Talla única"),
        },
    ]
}

/// Proveedores de insumos que abastecen el salón de belleza demo. Un salón
/// igual compra insumos (tinturas, esmaltes, ceras), así que las órdenes de
/// compra demo siguen teniendo con quién operar aunque el catálogo sea de
/// servicios.
fn servicios_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Insumos Profesionales Belleza Ltda.",
            rut: "76.540.210-8",
            contact_name: "Ventas Insumos Belleza",
            contact_phone: "+56 2 2588 7700",
        },
        DemoSupplier {
            name: "Distribuidora Capilar Chile",
            rut: "77.310.450-2",
            contact_name: "Pedidos Capilar",
            contact_phone: "+56 2 2477 3300",
        },
        DemoSupplier {
            name: "Cosmética y Estética del Sur",
            rut: "78.660.920-K",
            contact_name: "Reparto del Sur",
            contact_phone: "+56 9 6543 2109",
        },
    ]
}

/// Pack belleza/servicios: ítems vendibles que son **servicios**, no bienes
/// físicos (corte, manicure, color…). Sin campos clínicos, precios CLP
/// plausibles. Se siembran con `stock = 0` y SIN lote: el seed los marca
/// `physical_stock = false` (migración 0031), así la venta salta el chequeo de
/// stock y no toca inventario. `expiry_in_days`/`batch_code` quedan en el
/// `SeedItem` por uniformidad del struct pero NO se usan (no se crea lote).
fn servicios_pack() -> Vec<SeedItem> {
    // Helper local: todos los servicios comparten stock/vencimiento/clínica.
    fn svc(
        name: &'static str,
        barcode: &'static str,
        price: i64,
        cost: i64,
        batch_code: &'static str,
        supplier_idx: usize,
        presentation: &'static str,
    ) -> SeedItem {
        SeedItem {
            name,
            barcode,
            price,
            cost,
            // Servicio = sin inventario: se siembra con stock 0 y SIN lote (el
            // seed lo trata como `physical_stock = false`). No hay stock-proxy.
            stock: 0,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code,
            supplier_idx,
            laboratory: None,
            active_ingredient: None,
            presentation: Some(presentation),
        }
    }
    vec![
        svc(
            "Corte de pelo dama",
            "7805000100010",
            12990,
            3000,
            "SVC-CORTE-D",
            1,
            "Servicio · 45 min",
        ),
        svc(
            "Corte de pelo varón",
            "7805000100027",
            8990,
            2000,
            "SVC-CORTE-V",
            1,
            "Servicio · 30 min",
        ),
        svc(
            "Manicure tradicional",
            "7805000100034",
            9990,
            2500,
            "SVC-MANICURE",
            0,
            "Servicio · 40 min",
        ),
        svc(
            "Pedicure spa",
            "7805000100041",
            13990,
            3500,
            "SVC-PEDICURE",
            0,
            "Servicio · 60 min",
        ),
        svc(
            "Color / tintura completa",
            "7805000100058",
            29990,
            9000,
            "SVC-COLOR",
            2,
            "Servicio · 90 min",
        ),
        svc(
            "Mechas balayage",
            "7805000100065",
            49990,
            15000,
            "SVC-BALAYAGE",
            2,
            "Servicio · 150 min",
        ),
        svc(
            "Peinado de evento",
            "7805000100072",
            19990,
            4000,
            "SVC-PEINADO",
            1,
            "Servicio · 60 min",
        ),
        svc(
            "Depilación cera media pierna",
            "7805000100089",
            11990,
            3000,
            "SVC-DEPILA",
            0,
            "Servicio · 30 min",
        ),
        svc(
            "Masaje relajación 60 min",
            "7805000100096",
            24990,
            6000,
            "SVC-MASAJE",
            2,
            "Servicio · 60 min",
        ),
        svc(
            "Tratamiento facial hidratante",
            "7805000100102",
            22990,
            6500,
            "SVC-FACIAL",
            0,
            "Servicio · 50 min",
        ),
        svc(
            "Alisado keratina",
            "7805000100119",
            39990,
            12000,
            "SVC-KERATINA",
            1,
            "Servicio · 120 min",
        ),
        svc(
            "Diseño de cejas",
            "7805000100126",
            6990,
            1500,
            "SVC-CEJAS",
            0,
            "Servicio · 20 min",
        ),
    ]
}

/// Proveedores que abastecen el restaurant demo (insumos: abarrotes, carnes,
/// verduras). Los platos preparados no se compran, pero los insumos sí, así que
/// Compras igual tiene con quién operar.
fn restaurant_suppliers() -> Vec<DemoSupplier> {
    vec![
        DemoSupplier {
            name: "Distribuidora de Alimentos del Pacífico",
            rut: "76.455.330-1",
            contact_name: "Pedidos Alimentos Pacífico",
            contact_phone: "+56 2 2630 7700",
        },
        DemoSupplier {
            name: "Frigorífico Carnes del Sur",
            rut: "77.620.140-5",
            contact_name: "Ventas Carnes del Sur",
            contact_phone: "+56 2 2444 9900",
        },
        DemoSupplier {
            name: "Verdulería Mayorista La Vega",
            rut: "78.330.910-K",
            contact_name: "Reparto La Vega",
            contact_phone: "+56 9 8765 1234",
        },
    ]
}

/// Pack restaurant: rubro **mixto**. Dos clases de ítem en un mismo catálogo:
///
/// * **Insumos** (harina, aceite, carne, verduras) — bienes físicos: `batch_code`
///   no vacío → el seed les crea lote + movimiento (stock entra por el ledger),
///   con vencimientos escalonados (sanos / próximos a vencer / stock bajo).
/// * **Platos preparados** (lomo a lo pobre, empanada de pino…) — vendibles SIN
///   inventario físico: `batch_code` vacío, `stock = 0` → el seed los marca
///   `physical_stock = false`, sin lote y sin movimiento. La venta de un plato
///   salta el chequeo de stock (igual que un servicio).
///
/// La señal por-ítem que el seed usa para decidir "físico vs. vendible-sin-stock"
/// es `batch_code.is_empty()`.
fn restaurant_pack() -> Vec<SeedItem> {
    // Insumo físico (entra stock por lote). Mismos campos que un `SeedItem`
    // físico; helper local para no repetir `laboratory/active_ingredient: None`.
    #[allow(clippy::too_many_arguments)]
    fn insumo(
        name: &'static str,
        barcode: &'static str,
        price: i64,
        cost: i64,
        stock: i64,
        expiry_in_days: i64,
        batch_code: &'static str,
        supplier_idx: usize,
        presentation: &'static str,
    ) -> SeedItem {
        SeedItem {
            name,
            barcode,
            price,
            cost,
            stock,
            expiry_in_days,
            batch_code,
            supplier_idx,
            laboratory: None,
            active_ingredient: None,
            presentation: Some(presentation),
        }
    }
    // Plato preparado: vendible sin inventario. `batch_code` vacío = señal de
    // "no físico"; stock 0 y vencimiento lejano (no se usa, no hay lote).
    fn plato(
        name: &'static str,
        barcode: &'static str,
        price: i64,
        cost: i64,
        supplier_idx: usize,
        presentation: &'static str,
    ) -> SeedItem {
        SeedItem {
            name,
            barcode,
            price,
            cost,
            stock: 0,
            expiry_in_days: SHELF_STABLE_DAYS,
            batch_code: "", // vacío = no físico (sin lote)
            supplier_idx,
            laboratory: None,
            active_ingredient: None,
            presentation: Some(presentation),
        }
    }
    vec![
        // --- Insumos físicos (con lote + vencimiento) ---
        insumo(
            "Harina sin polvos 1kg",
            "7806000100013",
            1490,
            900,
            40,
            365,
            "HARINA-LOTE",
            0,
            "1 kg",
        ),
        insumo(
            "Aceite vegetal 5L",
            "7806000100020",
            9990,
            6800,
            25,
            365,
            "ACEITE-LOTE",
            0,
            "5 litros",
        ),
        insumo(
            "Carne molida 1kg",
            "7806000100037",
            7990,
            5200,
            12,
            5, // próximo a vencer
            "CARNE-NEAR",
            1,
            "1 kg",
        ),
        insumo(
            "Tomate kg",
            "7806000100044",
            1290,
            700,
            30,
            6, // perecible próximo a vencer
            "TOMATE-NEAR",
            2,
            "1 kg",
        ),
        insumo(
            "Papa saco 25kg",
            "7806000100051",
            18990,
            12000,
            8,
            90,
            "PAPA-LOTE",
            2,
            "Saco 25 kg",
        ),
        insumo(
            "Queso laminado 1kg",
            "7806000100068",
            8990,
            5800,
            4, // stock bajo
            20,
            "QUESO-LOW",
            1,
            "1 kg",
        ),
        // --- Platos preparados (vendibles sin stock físico) ---
        plato(
            "Lomo a lo pobre",
            "7806000100075",
            8990,
            3500,
            1,
            "Plato · 1 porción",
        ),
        plato(
            "Churrasco italiano",
            "7806000100082",
            6490,
            2400,
            1,
            "Sándwich · 1 unidad",
        ),
        plato(
            "Empanada de pino",
            "7806000100099",
            2490,
            900,
            0,
            "Unidad horneada",
        ),
        plato(
            "Completo italiano",
            "7806000100105",
            3490,
            1300,
            1,
            "Hot dog · 1 unidad",
        ),
        plato(
            "Cazuela de vacuno",
            "7806000100112",
            6990,
            2800,
            1,
            "Plato · 1 porción",
        ),
        plato(
            "Papas fritas familiares",
            "7806000100129",
            4990,
            1600,
            2,
            "Porción familiar",
        ),
        plato(
            "Ensalada césar",
            "7806000100136",
            5490,
            2000,
            2,
            "Plato · 1 porción",
        ),
        plato(
            "Menú del día",
            "7806000100143",
            5990,
            2600,
            0,
            "Almuerzo · entrada+fondo",
        ),
    ]
}

fn pack_for(v: SeedVertical) -> Vec<SeedItem> {
    match v {
        SeedVertical::Pharmacy => pharmacy_pack(),
        SeedVertical::Minimarket => minimarket_pack(),
        SeedVertical::Cafe => cafe_pack(),
        SeedVertical::Tienda => tienda_pack(),
        SeedVertical::Servicios => servicios_pack(),
        SeedVertical::Restaurant => restaurant_pack(),
    }
}

fn suppliers_for(v: SeedVertical) -> Vec<DemoSupplier> {
    match v {
        SeedVertical::Pharmacy => pharmacy_suppliers(),
        SeedVertical::Minimarket => minimarket_suppliers(),
        SeedVertical::Cafe => cafe_suppliers(),
        SeedVertical::Tienda => tienda_suppliers(),
        SeedVertical::Servicios => servicios_suppliers(),
        SeedVertical::Restaurant => restaurant_suppliers(),
    }
}

/// Nombres de TODOS los proveedores demo (ambos verticales) — la marca borrable
/// de proveedores es su nombre (no tienen `external_id`). El wipe los acota por
/// esta lista cerrada para nunca tocar un proveedor real del operador.
fn all_demo_supplier_names() -> Vec<String> {
    pharmacy_suppliers()
        .into_iter()
        .chain(minimarket_suppliers())
        .chain(cafe_suppliers())
        .chain(tienda_suppliers())
        .chain(servicios_suppliers())
        .chain(restaurant_suppliers())
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
        // Físico por-ítem = entra stock por lote. Es no físico si (a) el vertical
        // es servicios (catálogo entero de servicios), o (b) el ítem no trae lote
        // (`batch_code` vacío) — p.ej. un plato preparado en un restaurant, que
        // se vende sin descontar inventario. Restaurant mezcla ambos.
        let physical_stock = v != SeedVertical::Servicios && !item.batch_code.is_empty();

        let product = catalog::create_product(
            db,
            tenant,
            NewProduct {
                name: item.name.to_string(),
                slug: None,
                description: None,
                price: Decimal::from(item.price),
                cost_price: Some(Decimal::from(item.cost)),
                stock: 0, // el stock físico entra por el lote → emite movimiento
                category: None,
                image_url: None,
                external_id: Some(demo_external_id(item.name)),
                laboratory: item.laboratory.map(str::to_string),
                therapeutic_action: None,
                active_ingredient: item.active_ingredient.map(str::to_string),
                prescription_type: None,
                presentation: item.presentation.map(str::to_string),
                discount_percent: None,
                attrs: None,
            },
        )
        .await?;
        products_created += 1;

        // Código de barra (EAN-13) → product_barcode (lo lee el scan del POS).
        let product_thing = surrealdb::sql::thing(&product.id)
            .map_err(|e| DomainError::Other(anyhow::anyhow!("product id parse: {e}")))?;
        catalog_repo::upsert_barcode(db, tenant, &product_thing, item.barcode).await?;

        // Servicio = no físico: marcarlo `physical_stock = false` (migración
        // 0031). `create_product` deja el DEFAULT `true`; este UPDATE lo baja
        // para los servicios antes de (no) sembrar lote.
        if !physical_stock {
            catalog_repo::set_physical_stock(db, tenant, &product_thing, false).await?;
        }

        // Un servicio (`physical_stock = false`) no tiene inventario: no se crea
        // lote ni se emite movimiento. Los bienes físicos entran su stock por un
        // lote (emite `stock_movement`), preservando
        // `product.stock == Σ batch == Σ movement`.
        if physical_stock {
            let expiry = Utc::now() + Duration::days(item.expiry_in_days);
            inventory::create_batch(
                db,
                tenant,
                NewBatch {
                    product: product.id.clone(),
                    // Demo de un solo local: los lotes nacen en casa matriz,
                    // igual que el stock que siembra el resto del seed.
                    branch: None,
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
                branch: None,
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

    /// Todos los verticales con pack demo. Fuente única para los tests que
    /// barren los cuatro rubros.
    const ALL_VERTICALS: [SeedVertical; 6] = [
        SeedVertical::Pharmacy,
        SeedVertical::Minimarket,
        SeedVertical::Cafe,
        SeedVertical::Tienda,
        SeedVertical::Servicios,
        SeedVertical::Restaurant,
    ];

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
        assert_eq!(SeedVertical::parse("cafe").unwrap(), SeedVertical::Cafe);
        assert_eq!(SeedVertical::parse("Café").unwrap(), SeedVertical::Cafe);
        assert_eq!(
            SeedVertical::parse("pastelería").unwrap(),
            SeedVertical::Cafe
        );
        assert_eq!(SeedVertical::parse("tienda").unwrap(), SeedVertical::Tienda);
        assert_eq!(
            SeedVertical::parse("  Retail ").unwrap(),
            SeedVertical::Tienda
        );
        assert_eq!(
            SeedVertical::parse("servicios").unwrap(),
            SeedVertical::Servicios
        );
        assert_eq!(
            SeedVertical::parse("Belleza").unwrap(),
            SeedVertical::Servicios
        );
        assert_eq!(
            SeedVertical::parse("  peluquería ").unwrap(),
            SeedVertical::Servicios
        );
        assert!(SeedVertical::parse("casino").is_err());
    }

    #[test]
    fn label_round_trips_through_parse() {
        for v in ALL_VERTICALS {
            assert_eq!(SeedVertical::parse(v.label()).unwrap(), v);
        }
    }

    #[test]
    fn cafe_pack_is_perishable_no_clinical() {
        let pack = cafe_pack();
        for item in &pack {
            assert!(item.laboratory.is_none(), "{} sin laboratorio", item.name);
            assert!(item.active_ingredient.is_none());
            assert!(!item.batch_code.is_empty(), "perecible necesita lote");
        }
        // Pastelería: al menos un perecible próximo a vencer (≤7 días).
        assert!(
            pack.iter().any(|i| i.expiry_in_days <= 7),
            "café/pastelería debe tener perecibles próximos a vencer"
        );
    }

    #[test]
    fn tienda_pack_is_shelf_stable_no_clinical() {
        let pack = tienda_pack();
        assert!(
            pack.len() >= 8,
            "catálogo retail creíble (≥8 SKUs demo), got {}",
            pack.len()
        );
        for item in &pack {
            assert!(item.laboratory.is_none(), "{} sin laboratorio", item.name);
            assert!(item.active_ingredient.is_none());
            // Retail no perecible: vencimiento lejano → nunca near-expiry.
            assert_eq!(
                item.expiry_in_days, SHELF_STABLE_DAYS,
                "{} debe ser no perecible",
                item.name
            );
            // Stock entra por lote (invariante ledger) aunque el pack UI tenga
            // `lotes:false` — el lote es transporte del stock, no feature de
            // vencimiento.
            assert!(
                !item.batch_code.is_empty(),
                "{} necesita batch_code para el ledger",
                item.name
            );
            assert!(item.stock > 0, "{} es bien físico → stock > 0", item.name);
        }
        // Al menos un ítem en stock bajo para alertas de reposición.
        assert!(
            pack.iter().any(|i| i.stock <= 5),
            "tienda demo debe incluir stock bajo"
        );
    }

    /// Coherencia rubro-pack ↔ seed: todo `seed_vertical` del catálogo rubro
    /// parsea a un `SeedVertical` real (y el pack de ese vertical no está vacío).
    #[test]
    fn rubro_seed_verticals_resolve_to_nonempty_packs() {
        for key in crate::rubro::all_rubros() {
            let pack = crate::rubro::pack_for(key);
            let Some(sv) = pack.seed_vertical else {
                continue;
            };
            let v = SeedVertical::parse(sv).unwrap_or_else(|e| {
                panic!("rubro «{key}» seed_vertical «{sv}» no parsea: {e}");
            });
            let items = pack_for(v);
            assert!(
                !items.is_empty(),
                "seed pack de «{sv}» (rubro {key}) no debe estar vacío"
            );
        }
        // Beachhead P1: tienda es first-class en ambos lados.
        assert_eq!(
            crate::rubro::pack_for("tienda").seed_vertical,
            Some("tienda")
        );
        assert_eq!(SeedVertical::parse("tienda").unwrap(), SeedVertical::Tienda);
        assert!(!tienda_pack().is_empty());
    }

    #[test]
    fn servicios_pack_is_service_no_clinical_no_stock() {
        let pack = servicios_pack();
        assert!(pack.len() >= 10, "catálogo de servicios creíble (≥10)");
        for item in &pack {
            assert!(item.laboratory.is_none(), "{} sin laboratorio", item.name);
            assert!(item.active_ingredient.is_none());
            // Servicio honesto: sin inventario → stock 0 (sin stock-proxy). El
            // seed lo siembra `physical_stock = false`, sin lote: la venta salta
            // el chequeo de stock (migración 0031).
            assert_eq!(
                item.stock, 0,
                "{} es servicio: stock 0, sin proxy de inventario",
                item.name
            );
        }
    }

    #[test]
    fn restaurant_pack_mixes_physical_insumos_and_serviceable_platos() {
        let pack = restaurant_pack();
        // Insumo físico = trae lote (`batch_code` no vacío) y stock > 0.
        let insumos: Vec<&SeedItem> = pack.iter().filter(|i| !i.batch_code.is_empty()).collect();
        // Plato vendible sin stock = `batch_code` vacío y stock 0.
        let platos: Vec<&SeedItem> = pack.iter().filter(|i| i.batch_code.is_empty()).collect();
        assert!(
            insumos.len() >= 3,
            "restaurant debe traer insumos físicos (con lote)"
        );
        assert!(
            platos.len() >= 3,
            "restaurant debe traer platos vendibles sin stock"
        );
        for i in &insumos {
            assert!(i.stock > 0, "{} es insumo físico → stock > 0", i.name);
            assert!(i.laboratory.is_none() && i.active_ingredient.is_none());
        }
        for p in &platos {
            assert_eq!(
                p.stock, 0,
                "{} es plato vendible-sin-stock → stock 0",
                p.name
            );
        }
        // Al menos un insumo perecible próximo a vencer (≤7 días) para alertas.
        assert!(
            insumos.iter().any(|i| i.expiry_in_days <= 7),
            "restaurant debe tener insumos perecibles próximos a vencer"
        );
    }

    #[test]
    fn barcodes_are_globally_unique_across_packs() {
        let mut codes: Vec<&str> = ALL_VERTICALS
            .into_iter()
            .flat_map(|v| {
                pack_for(v)
                    .into_iter()
                    .map(|i| i.barcode)
                    .collect::<Vec<_>>()
            })
            .collect();
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(
            codes.len(),
            n,
            "barcodes deben ser únicos entre todos los packs"
        );
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
        for v in ALL_VERTICALS {
            let pack = pack_for(v);
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
        for v in ALL_VERTICALS {
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
