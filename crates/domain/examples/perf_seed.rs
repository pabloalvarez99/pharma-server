//! Builds the catalog that [`perf_probe`](./perf_probe.rs) measures against: a
//! real SurrealKV directory with the real migrations and N products created
//! through the real service path (slug, defaults, indexes all identical to a
//! live install).
//!
//!   cargo run --release -p domain --example perf_seed -- <db-path> <slug> [count]
//!
//! Re-running against an existing directory tops the catalog up to `count`
//! instead of duplicating it. Run with the server stopped (SurrealKV is
//! single-writer).

use std::time::Instant;

use domain::catalog::model::NewProduct;
use domain::catalog::service;
use rust_decimal::Decimal;
use surrealdb::engine::local::SurrealKv;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

/// Ingredient + brand pairs, cycled so search terms hit a realistic slice of
/// the catalog instead of everything or nothing.
const INGREDIENTS: &[(&str, &str)] = &[
    ("Paracetamol", "Panadol"),
    ("Ibuprofeno", "Advil"),
    ("Amoxicilina", "Amoxival"),
    ("Loratadina", "Clarityne"),
    ("Omeprazol", "Losec"),
    ("Metformina", "Glucophage"),
    ("Losartan", "Cozaar"),
    ("Atorvastatina", "Lipitor"),
    ("Salbutamol", "Ventolin"),
    ("Cetirizina", "Zyrtec"),
    ("Diclofenaco", "Voltaren"),
    ("Ranitidina", "Zantac"),
    ("Clonazepam", "Rivotril"),
    ("Levotiroxina", "Eutirox"),
    ("Enalapril", "Renitec"),
    ("Naproxeno", "Naprosyn"),
];

const FORMS: &[&str] = &["comprimidos", "capsulas", "jarabe", "crema", "gotas"];
const STRENGTHS: &[&str] = &["100 mg", "250 mg", "500 mg", "750 mg", "1 g"];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: perf_seed <db-path> <slug> [count]");
    let slug = args.next().expect("tenant slug");
    let count: usize = args
        .next()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(3_000);

    let db = Surreal::new::<SurrealKv>(path.as_str()).await?;
    db.use_ns("pharma").use_db("main").await?;

    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    let applied = db::run_migrations(&db, migrations).await?;
    println!(
        "migrations: {} applied",
        applied.iter().filter(|m| m.applied).count()
    );

    let mut r = db
        .query("SELECT id FROM tenant WHERE slug = $s LIMIT 1")
        .bind(("s", slug.clone()))
        .await?;
    let tenant: Thing = match r.take::<Option<Thing>>((0, "id"))? {
        Some(t) => t,
        None => {
            let mut r = db
                .query("CREATE tenant SET name = $n, slug = $s RETURN id")
                .bind(("n", format!("Perf {slug}")))
                .bind(("s", slug.clone()))
                .await?;
            r.take::<Option<Thing>>((0, "id"))?.expect("tenant created")
        }
    };

    let mut r = db
        .query("SELECT count() FROM product WHERE tenant = $t GROUP ALL")
        .bind(("t", tenant.clone()))
        .await?;
    let have: usize = r.take::<Option<i64>>((0, "count"))?.unwrap_or(0) as usize;
    println!("tenant {tenant} has {have} products, target {count}");

    let t0 = Instant::now();
    for i in have..count {
        let (ingredient, brand) = INGREDIENTS[i % INGREDIENTS.len()];
        let form = FORMS[(i / INGREDIENTS.len()) % FORMS.len()];
        let strength = STRENGTHS[(i / 7) % STRENGTHS.len()];
        let p = NewProduct {
            name: format!("{brand} {strength} {form} x{i}"),
            slug: None,
            description: None,
            price: Decimal::from((i % 40_000 + 500) as i64),
            cost_price: None,
            stock: (i % 90) as i64,
            category: None,
            image_url: None,
            external_id: Some(format!("SKU{i:06}")),
            laboratory: None,
            therapeutic_action: None,
            active_ingredient: Some(ingredient.to_string()),
            prescription_type: None,
            presentation: None,
            physical_stock: None,
            discount_percent: None,
            attrs: None,
        };
        service::create_product(&db, &tenant, p).await?;
        if (i + 1) % 500 == 0 {
            println!("  {} products ({:.1}s)", i + 1, t0.elapsed().as_secs_f64());
        }
    }
    println!(
        "done: {count} products in {:.1}s",
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
