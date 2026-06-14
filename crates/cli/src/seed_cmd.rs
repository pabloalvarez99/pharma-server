//! `pharma seed-demo` — puebla un tenant con un negocio chileno de demostración
//! para que se pueda PROBAR la app contra datos creíbles en vez de pantallas
//! vacías.
//!
//! **Multi-rubro a propósito** (ADR-0011 visión RutAgentIA — ver
//! `docs/strategy/rutagentia-vision.md`): el core es agnóstico de rubro; la
//! farmacia es el *beachhead*, no el límite. El seed demuestra esto con dos
//! "packs" verticales sobre el MISMO schema: `pharmacy` (con recetados, controlado
//! Ley 20.000, lotes por vencer) y `minimarket` (abarrotes, sin campos clínicos).
//! Más rubros = agregar un pack acá; el core no cambia.
//!
//! Diseño:
//! - **Menos invasivo**: CLI local (corre como `pharma migrate`), sin endpoint,
//!   sin migración, sin tocar el cliente. Reusa el handle de DB directo.
//! - **Idempotente**: lo sembrado se marca (`product.external_id`=`DEMO-…`,
//!   `stock_movement.reason`=`seed_demo`, `customer.email`=`demo.*@…`). Re-correr
//!   sin `--force` es no-op; con `--force` borra SOLO lo DEMO y re-siembra (sirve
//!   también para cambiar de vertical).
//! - **Honra invariantes**: por producto crea product(stock=N)+product_batch(stock=N)
//!   +stock_movement(delta=+N) → `product.stock == Σ batch.stock == Σ movimientos`.

use anyhow::{anyhow, Context};
use chrono::{Duration, Utc};
use surrealdb::sql::Thing;

const DEMO_EMAIL_DOMAIN: &str = "demo.rutagentia.local";
const SEED_REASON: &str = "seed_demo";

/// Rubro del pack demo. El core ERP es el mismo; sólo cambian los datos.
#[derive(Clone, Copy)]
enum Vertical {
    Pharmacy,
    Minimarket,
}

impl Vertical {
    fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "pharmacy" | "farmacia" => Ok(Self::Pharmacy),
            "minimarket" | "almacen" | "abarrotes" => Ok(Self::Minimarket),
            other => Err(anyhow!(
                "vertical '{other}' sin pack demo todavía. Disponibles: pharmacy, minimarket. \
                 (Agregar un pack = editar crates/cli/src/seed_cmd.rs; el core no cambia.)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pharmacy => "farmacia",
            Self::Minimarket => "minimarket",
        }
    }

    fn products(self) -> &'static [DemoProduct] {
        match self {
            Self::Pharmacy => PHARMACY_PRODUCTS,
            Self::Minimarket => MINIMARKET_PRODUCTS,
        }
    }
}

/// Un producto demo genérico (sirve a cualquier rubro). Precios CLP en string
/// para el cast `<decimal>`. Campos clínicos quedan vacíos en rubros no-farmacia.
struct DemoProduct {
    sku: &'static str,
    name: &'static str,
    price: &'static str,
    cost: &'static str,
    stock: i64,
    ingrediente: &'static str, // vacío salvo farmacia
    laboratorio: &'static str, // marca/proveedor
    receta: &'static str,      // direct | receta | receta_retenida | controlado
    dias_venc: i64,
}

/// Pack farmacia (beachhead): OTC de alta rotación, crónicos recetados, un
/// controlado (Ley 20.000), y lotes próximos a vencer para el reporte near-expiry.
const PHARMACY_PRODUCTS: &[DemoProduct] = &[
    DemoProduct {
        sku: "PARA500",
        name: "Paracetamol 500mg x16 comp",
        price: "1290",
        cost: "650",
        stock: 120,
        ingrediente: "paracetamol",
        laboratorio: "Laboratorio Chile",
        receta: "direct",
        dias_venc: 540,
    },
    DemoProduct {
        sku: "IBUP400",
        name: "Ibuprofeno 400mg x20 comp",
        price: "2490",
        cost: "1200",
        stock: 90,
        ingrediente: "ibuprofeno",
        laboratorio: "Mintlab",
        receta: "direct",
        dias_venc: 420,
    },
    DemoProduct {
        sku: "ASPI100",
        name: "Aspirina 100mg x30 comp",
        price: "1990",
        cost: "900",
        stock: 60,
        ingrediente: "acido acetilsalicilico",
        laboratorio: "Bayer",
        receta: "direct",
        dias_venc: 700,
    },
    DemoProduct {
        sku: "OMEP20",
        name: "Omeprazol 20mg x14 cáps",
        price: "3290",
        cost: "1500",
        stock: 75,
        ingrediente: "omeprazol",
        laboratorio: "Andromaco",
        receta: "direct",
        dias_venc: 365,
    },
    DemoProduct {
        sku: "LORA10",
        name: "Loratadina 10mg x10 comp",
        price: "1690",
        cost: "700",
        stock: 80,
        ingrediente: "loratadina",
        laboratorio: "Saval",
        receta: "direct",
        dias_venc: 480,
    },
    DemoProduct {
        sku: "VITC1G",
        name: "Vitamina C 1g x10 eferv",
        price: "2190",
        cost: "1000",
        stock: 50,
        ingrediente: "acido ascorbico",
        laboratorio: "Mintlab",
        receta: "direct",
        dias_venc: 300,
    },
    DemoProduct {
        sku: "SALB100",
        name: "Salbutamol inhalador 100mcg",
        price: "4990",
        cost: "2600",
        stock: 35,
        ingrediente: "salbutamol",
        laboratorio: "Saval",
        receta: "receta",
        dias_venc: 540,
    },
    DemoProduct {
        sku: "LOSA50",
        name: "Losartán 50mg x30 comp",
        price: "3990",
        cost: "1800",
        stock: 70,
        ingrediente: "losartan",
        laboratorio: "Laboratorio Chile",
        receta: "receta",
        dias_venc: 600,
    },
    DemoProduct {
        sku: "METF850",
        name: "Metformina 850mg x30 comp",
        price: "2890",
        cost: "1300",
        stock: 65,
        ingrediente: "metformina",
        laboratorio: "Andromaco",
        receta: "receta",
        dias_venc: 560,
    },
    DemoProduct {
        sku: "ATOR20",
        name: "Atorvastatina 20mg x30 comp",
        price: "5490",
        cost: "2700",
        stock: 55,
        ingrediente: "atorvastatina",
        laboratorio: "Mintlab",
        receta: "receta",
        dias_venc: 520,
    },
    DemoProduct {
        sku: "AMOX500",
        name: "Amoxicilina 500mg x21 cáps",
        price: "3690",
        cost: "1700",
        stock: 40,
        ingrediente: "amoxicilina",
        laboratorio: "Saval",
        receta: "receta_retenida",
        dias_venc: 280,
    },
    DemoProduct {
        sku: "CLON05",
        name: "Clonazepam 0.5mg x30 comp",
        price: "4290",
        cost: "2100",
        stock: 25,
        ingrediente: "clonazepam",
        laboratorio: "Laboratorio Chile",
        receta: "controlado",
        dias_venc: 610,
    },
    DemoProduct {
        sku: "IBUPED",
        name: "Ibuprofeno pediátrico jarabe 100ml",
        price: "3490",
        cost: "1600",
        stock: 45,
        ingrediente: "ibuprofeno",
        laboratorio: "Mintlab",
        receta: "direct",
        dias_venc: 45,
    },
    DemoProduct {
        sku: "TESTEMB",
        name: "Test de embarazo",
        price: "2990",
        cost: "1200",
        stock: 30,
        ingrediente: "",
        laboratorio: "Genérico",
        receta: "direct",
        dias_venc: 365,
    },
    DemoProduct {
        sku: "ALCGEL",
        name: "Alcohol gel 250ml",
        price: "1990",
        cost: "800",
        stock: 100,
        ingrediente: "etanol",
        laboratorio: "Genérico",
        receta: "direct",
        dias_venc: 50,
    },
    DemoProduct {
        sku: "SUERO",
        name: "Suero fisiológico 500ml",
        price: "1490",
        cost: "600",
        stock: 85,
        ingrediente: "cloruro de sodio",
        laboratorio: "Sanderson",
        receta: "direct",
        dias_venc: 720,
    },
];

/// Pack minimarket/almacén (demuestra que el core NO es sólo farmacia): abarrotes
/// genéricos, sin ingrediente activo ni tipo de receta clínico.
const MINIMARKET_PRODUCTS: &[DemoProduct] = &[
    DemoProduct {
        sku: "ARROZ1K",
        name: "Arroz grado 1 1kg",
        price: "1490",
        cost: "950",
        stock: 200,
        ingrediente: "",
        laboratorio: "Tucapel",
        receta: "direct",
        dias_venc: 540,
    },
    DemoProduct {
        sku: "ACEITE1L",
        name: "Aceite vegetal 1L",
        price: "2390",
        cost: "1600",
        stock: 120,
        ingrediente: "",
        laboratorio: "Chef",
        receta: "direct",
        dias_venc: 420,
    },
    DemoProduct {
        sku: "FIDEO400",
        name: "Fideos espagueti 400g",
        price: "990",
        cost: "550",
        stock: 180,
        ingrediente: "",
        laboratorio: "Carozzi",
        receta: "direct",
        dias_venc: 600,
    },
    DemoProduct {
        sku: "BEBIDA15",
        name: "Bebida cola 1.5L",
        price: "1790",
        cost: "1100",
        stock: 150,
        ingrediente: "",
        laboratorio: "Andina",
        receta: "direct",
        dias_venc: 120,
    },
    DemoProduct {
        sku: "AZUCAR1K",
        name: "Azúcar 1kg",
        price: "1290",
        cost: "850",
        stock: 160,
        ingrediente: "",
        laboratorio: "Iansa",
        receta: "direct",
        dias_venc: 365,
    },
    DemoProduct {
        sku: "DETERG1K",
        name: "Detergente polvo 1kg",
        price: "3490",
        cost: "2200",
        stock: 90,
        ingrediente: "",
        laboratorio: "Omo",
        receta: "direct",
        dias_venc: 720,
    },
    DemoProduct {
        sku: "PANMOLDE",
        name: "Pan de molde 500g",
        price: "1890",
        cost: "1150",
        stock: 60,
        ingrediente: "",
        laboratorio: "Ideal",
        receta: "direct",
        dias_venc: 15,
    },
    DemoProduct {
        sku: "LECHE1L",
        name: "Leche entera 1L",
        price: "1190",
        cost: "780",
        stock: 140,
        ingrediente: "",
        laboratorio: "Soprole",
        receta: "direct",
        dias_venc: 40,
    },
];

/// Clientes genéricos (sirven a cualquier rubro): nombre, RUT válido mód-11, fono.
const CUSTOMERS: &[(&str, &str, &str)] = &[
    ("María González", "12345678-5", "+56912345678"),
    ("Juan Pérez", "11111111-1", "+56987654321"),
    ("Ana Soto", "9876543-3", "+56955554444"),
    ("Pedro Ramírez", "22222222-2", "+56922221111"),
];

pub async fn run(tenant_slug: String, vertical: String, force: bool) -> anyhow::Result<()> {
    let vertical = Vertical::parse(&vertical)?;
    let cfg = pharma_core::config::AppConfig::load()?;
    let db = db::connect(&cfg.db).await?;

    // Resolver tenant por slug (mismo patrón que user-create).
    let mut tq = db
        .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", tenant_slug.clone()))
        .await
        .context("lookup tenant by slug")?;
    #[derive(serde::Deserialize)]
    struct T {
        id: Thing,
    }
    let t: Option<T> = tq.take(0)?;
    let tenant = t
        .ok_or_else(|| {
            anyhow!("tenant con slug '{tenant_slug}' no existe — créalo con `pharma tenant-create`")
        })?
        .id;

    // ¿Ya sembrado?
    let mut cq = db
        .query("SELECT count() FROM product WHERE tenant = $t AND string::starts_with(external_id ?? '', 'DEMO-') GROUP ALL")
        .bind(("t", tenant.clone()))
        .await
        .context("contar productos demo")?;
    let existing: Option<i64> = cq.take((0, "count"))?;
    let existing = existing.unwrap_or(0);

    if existing > 0 {
        if !force {
            println!(
                "El tenant '{tenant_slug}' ya tiene {existing} productos DEMO. \
                 No-op. Usa --force para borrar lo DEMO y re-sembrar (incl. cambiar de vertical)."
            );
            return Ok(());
        }
        println!("--force: borrando datos DEMO previos del tenant '{tenant_slug}'…");
        wipe_demo(&db, &tenant).await?;
    }

    // Sembrar productos + lote + movimiento (invariante de stock por producto).
    let mut n_prod = 0u32;
    for p in vertical.products() {
        let expiry = (Utc::now() + Duration::days(p.dias_venc)).to_rfc3339();
        let slug = format!("demo-{}", p.sku.to_lowercase());
        let external_id = format!("DEMO-{}", p.sku);
        let batch_code = format!("DEMO-L{}", p.sku);

        db.query(
            "LET $p = (CREATE product SET \
                tenant = $t, name = $name, slug = $slug, external_id = $ext, \
                price = <decimal>$price, cost_price = <decimal>$cost, stock = $stock, \
                active_ingredient = $ing, laboratory = $lab, prescription_type = $rx, \
                active = true RETURN AFTER)[0]; \
             CREATE product_batch SET \
                tenant = $t, product = $p.id, batch_code = $bcode, \
                expiry_date = <datetime>$expiry, stock = $stock, cost = <decimal>$cost, \
                active = true; \
             CREATE stock_movement SET \
                tenant = $t, product = $p.id, delta = $stock, reason = $reason; \
             RETURN $p.id;",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", p.name.to_string()))
        .bind(("slug", slug))
        .bind(("ext", external_id))
        .bind(("price", p.price.to_string()))
        .bind(("cost", p.cost.to_string()))
        .bind(("stock", p.stock))
        .bind(("ing", p.ingrediente.to_string()))
        .bind(("lab", p.laboratorio.to_string()))
        .bind(("rx", p.receta.to_string()))
        .bind(("bcode", batch_code))
        .bind(("expiry", expiry))
        .bind(("reason", SEED_REASON.to_string()))
        .await
        .with_context(|| format!("sembrar producto {}", p.sku))?
        .check()
        .with_context(|| format!("producto {} rechazado por el schema", p.sku))?;
        n_prod += 1;
    }

    // Sembrar clientes.
    let mut n_cust = 0u32;
    for (i, (name, rut, phone)) in CUSTOMERS.iter().enumerate() {
        let email = format!("demo.{i}@{DEMO_EMAIL_DOMAIN}");
        db.query(
            "CREATE customer SET tenant = $t, name = $name, rut = $rut, \
             phone = $phone, email = $email, active = true",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("rut", rut.to_string()))
        .bind(("phone", phone.to_string()))
        .bind(("email", email))
        .await
        .with_context(|| format!("sembrar cliente {name}"))?
        .check()
        .with_context(|| format!("cliente {name} rechazado por el schema"))?;
        n_cust += 1;
    }

    let extra = match vertical {
        Vertical::Pharmacy => {
            " Incluye recetados, un controlado (Ley 20.000) y lotes próximos a vencer."
        }
        Vertical::Minimarket => {
            " Abarrotes genéricos (sin campos clínicos): demuestra que el core no es sólo farmacia."
        }
    };
    println!(
        "DEMO ({}) sembrada en '{tenant_slug}': {n_prod} productos (con lote + \
         movimiento de stock) + {n_cust} clientes.{extra} Ya puedes abrir la app y probar.",
        vertical.label()
    );
    Ok(())
}

/// Borra SOLO las filas marcadas DEMO de este tenant. Nunca toca datos reales.
async fn wipe_demo(db: &db::Db, tenant: &Thing) -> anyhow::Result<()> {
    db.query(
        "DELETE stock_movement WHERE tenant = $t AND reason = $reason; \
         DELETE product_batch WHERE tenant = $t AND string::starts_with(batch_code, 'DEMO-'); \
         DELETE product WHERE tenant = $t AND string::starts_with(external_id ?? '', 'DEMO-'); \
         DELETE customer WHERE tenant = $t AND string::ends_with(email ?? '', $dom);",
    )
    .bind(("t", tenant.clone()))
    .bind(("reason", SEED_REASON.to_string()))
    .bind(("dom", format!("@{DEMO_EMAIL_DOMAIN}")))
    .await
    .context("borrar datos demo")?
    .check()
    .context("delete demo rechazado")?;
    Ok(())
}
