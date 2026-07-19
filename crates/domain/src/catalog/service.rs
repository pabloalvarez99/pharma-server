//! Catalog business logic: slug generation, category validation, bulk
//! repricing, stock adjustment. Thin orchestration over [`super::repo`].

use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// ASCII slug: lowercase, alnum runs joined by `-`. Accents folded best-effort.
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = true; // trims leading dashes
    for ch in input.chars() {
        let c = fold_char(ch);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("item");
    }
    out
}

fn fold_char(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        other => other,
    }
}

async fn unique_slug<F, Fut>(base: &str, mut exists: F) -> DomainResult<String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = DomainResult<bool>>,
{
    if !exists(base.to_string()).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let cand = format!("{base}-{n}");
        if !exists(cand.clone()).await? {
            return Ok(cand);
        }
    }
    Err(DomainError::Conflict(
        "no se pudo generar un slug único".into(),
    ))
}

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

// --- categories ------------------------------------------------------------

pub async fn create_category(
    db: &Db,
    tenant: &Thing,
    input: NewCategory,
) -> DomainResult<CategoryDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    let base = input
        .slug
        .as_deref()
        .map(slugify)
        .unwrap_or_else(|| slugify(&input.name));
    let slug = unique_slug(&base, |s| {
        let db = db.clone();
        let tenant = tenant.clone();
        async move { repo::category_slug_exists(&db, &tenant, &s).await }
    })
    .await?;
    repo::create_category(db, tenant, &slug, &input).await
}

pub async fn update_category(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateCategory,
) -> DomainResult<CategoryDto> {
    let id = parse_thing(id)?;
    repo::update_category(db, tenant, &id, &patch).await
}

pub async fn delete_category(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_category(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

pub async fn get_category(db: &Db, tenant: &Thing, id: &str) -> DomainResult<CategoryDto> {
    let id = parse_thing(id)?;
    repo::get_category(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

// --- products --------------------------------------------------------------

async fn resolve_category(db: &Db, tenant: &Thing, raw: &str) -> DomainResult<Thing> {
    let t = parse_thing(raw)?;
    if t.tb != "category" {
        return Err(DomainError::Invalid(
            "category debe ser un id de categoría".into(),
        ));
    }
    if !repo::category_belongs(db, tenant, &t).await? {
        return Err(DomainError::Invalid(
            "la categoría no existe en este tenant".into(),
        ));
    }
    Ok(t)
}

pub async fn create_product(
    db: &Db,
    tenant: &Thing,
    input: NewProduct,
) -> DomainResult<ProductDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    if input.price < Decimal::ZERO {
        return Err(DomainError::Invalid("precio no puede ser negativo".into()));
    }
    let category = match input.category.as_deref() {
        Some(s) if !s.is_empty() => Some(resolve_category(db, tenant, s).await?),
        _ => None,
    };
    let base = input
        .slug
        .as_deref()
        .map(slugify)
        .unwrap_or_else(|| slugify(&input.name));
    let slug = unique_slug(&base, |s| {
        let db = db.clone();
        let tenant = tenant.clone();
        async move { repo::product_slug_exists(&db, &tenant, &s).await }
    })
    .await?;
    repo::create_product(db, tenant, &slug, &input, category).await
}

pub async fn list_products(
    db: &Db,
    tenant: &Thing,
    filters: ProductFilters,
) -> DomainResult<Vec<ProductDto>> {
    // Hides child variants by default (retail padres + SKUs planos).
    let rows = repo::list_products(db, tenant, &filters).await?;
    enrich_list_variants_stock(db, tenant, rows).await
}

/// Same as [`list_products`] but can include variant children (`parent_id` set).
pub async fn list_products_with_variants(
    db: &Db,
    tenant: &Thing,
    filters: ProductFilters,
    include_variants: bool,
) -> DomainResult<Vec<ProductDto>> {
    let rows = repo::list_products_opts(db, tenant, &filters, include_variants).await?;
    enrich_list_variants_stock(db, tenant, rows).await
}

/// Attach `variants_stock` on multi-SKU parents in a list page (batch query).
/// Presence of the field is the client POS/list parent flag (C contract).
async fn enrich_list_variants_stock(
    db: &Db,
    tenant: &Thing,
    mut rows: Vec<ProductDto>,
) -> DomainResult<Vec<ProductDto>> {
    let parents: Vec<Thing> = rows
        .iter()
        .filter(|p| p.parent_id.is_none())
        .filter_map(|p| parse_thing(&p.id).ok())
        .collect();
    if parents.is_empty() {
        return Ok(rows);
    }
    let aggs = repo::children_agg_by_parents(db, tenant, &parents).await?;
    for p in &mut rows {
        if p.parent_id.is_none() {
            if let Some(&(sum, count)) = aggs.get(&p.id) {
                // Key present ⇒ ≥1 active child (even if sum stock is 0).
                p.variants_stock = Some(sum);
                p.variant_count = Some(count);
            }
        }
    }
    Ok(rows)
}

pub async fn get_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<ProductDto> {
    let id = parse_thing(id)?;
    let p = repo::get_product(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)?;
    enrich_product_dto(db, tenant, p).await
}

pub async fn update_product(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateProduct,
) -> DomainResult<ProductDto> {
    let pid = parse_thing(id)?;
    if let Some(p) = patch.price {
        if p < Decimal::ZERO {
            return Err(DomainError::Invalid("precio no puede ser negativo".into()));
        }
    }
    // tri-state category: absent skip, "" clear, id set+validate.
    let category: Option<Option<Thing>> = match patch.category.as_deref() {
        None => None,
        Some("") => Some(None),
        Some(s) => Some(Some(resolve_category(db, tenant, s).await?)),
    };
    // Barcode handled after row patch so a conflict leaves product fields
    // already updated only if barcode succeeds — apply barcode first when set.
    if let Some(ref raw) = patch.barcode {
        apply_barcode_patch(db, tenant, &pid, raw).await?;
    }
    let mut dto = repo::update_product(db, tenant, &pid, &patch, category).await?;
    // Re-enrich barcode after optional remapping.
    if dto.barcode.is_none() {
        dto.barcode = repo::barcode_of_product(db, tenant, &pid).await?;
    }
    Ok(dto)
}

/// Soft-delete a product. For sellable SKUs (plano o variante) also frees the
/// `product_barcode` mapping so the EAN can be reassigned. Stock and
/// `stock_movement` rows stay (ledger invariant + history).
pub async fn delete_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    // Refuse soft-delete of a multi-SKU parent while it still has active children
    // (would orphan live variants under an inactive shell).
    if repo::has_active_variants(db, tenant, &id).await? {
        return Err(DomainError::Invalid(
            "no se puede eliminar un producto con variantes activas; \
             elimine o desactive las variantes primero"
                .into(),
        ));
    }
    if repo::soft_delete_product(db, tenant, &id).await? {
        repo::delete_barcodes_of_product(db, tenant, &id).await?;
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

/// Soft-delete a sellable variant under `parent_id`.
///
/// Invariants:
/// - `variant_id` must be a child of `parent_id` (same tenant)
/// - soft-delete only (`active = false`); row + stock + movements preserved
/// - barcode mapping removed so the EAN is free for a new variant
/// - does **not** invent stock_movements (stock stays on inactive row; not in
///   `variants_stock` / list because those filter `active = true`)
pub async fn delete_variant(
    db: &Db,
    tenant: &Thing,
    parent_id: &str,
    variant_id: &str,
) -> DomainResult<()> {
    let parent_thing = parse_thing(parent_id)?;
    let child_thing = parse_thing(variant_id)?;
    if parent_thing.tb != "product" || child_thing.tb != "product" {
        return Err(DomainError::Invalid(
            "parent_id y variant_id deben ser ids de producto".into(),
        ));
    }
    let _ = repo::get_product(db, tenant, &parent_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    let child = repo::get_product(db, tenant, &child_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    match child.parent_id.as_deref() {
        Some(p) if p == parent_thing.to_string() || p == parent_id => {}
        Some(_) => {
            return Err(DomainError::Invalid(
                "la variante no pertenece a este producto padre".into(),
            ));
        }
        None => {
            return Err(DomainError::Invalid(
                "el producto no es una variante (sin parent_id)".into(),
            ));
        }
    }
    if !child.active {
        // Idempotent: already soft-deleted — still free any leftover barcode.
        repo::delete_barcodes_of_product(db, tenant, &child_thing).await?;
        return Ok(());
    }
    if !repo::soft_delete_product(db, tenant, &child_thing).await? {
        return Err(DomainError::NotFound);
    }
    repo::delete_barcodes_of_product(db, tenant, &child_thing).await?;
    Ok(())
}

/// Apply barcode remapping for `update_product`. Empty string clears.
async fn apply_barcode_patch(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    raw: &str,
) -> DomainResult<()> {
    let code = raw.trim();
    if code.is_empty() {
        repo::delete_barcodes_of_product(db, tenant, product).await?;
        return Ok(());
    }
    if let Some(owner) = repo::product_id_by_barcode(db, tenant, code).await? {
        if owner != *product {
            return Err(DomainError::Conflict(format!(
                "el código de barras '{code}' ya está asignado a {owner}"
            )));
        }
        // Same product already owns this code — no-op.
        return Ok(());
    }
    // Free any previous mapping for this product, then claim the new code.
    repo::delete_barcodes_of_product(db, tenant, product).await?;
    repo::create_barcode(db, tenant, product, code).await?;
    Ok(())
}

/// Manual adjustment from `POST /products/{id}/stock`. Fase 3 retrofit:
/// emits a `stock_movement` (`reason = "manual_adjust"`, `admin = JWT sub`)
/// via [`crate::inventory::service::add_movement`] in the same SurrealQL tx
/// that updates `product.stock`. `admin` is the JWT `sub` (record id) when
/// available; `None` is accepted for callers without a user context.
pub async fn adjust_stock(
    db: &Db,
    tenant: &Thing,
    id: &str,
    adj: StockAdjust,
    admin: Option<&str>,
) -> DomainResult<ProductDto> {
    let pid = parse_thing(id)?;
    let current = repo::get_product(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    let delta = match (adj.set, adj.delta) {
        (Some(s), None) => s - current.stock,
        (None, Some(d)) => d,
        _ => {
            return Err(DomainError::Invalid(
                "indique exactamente uno de `set` o `delta`".into(),
            ))
        }
    };
    if delta == 0 {
        return Ok(current);
    }
    let reason = adj
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("manual_adjust");
    let (_movement, product) =
        crate::inventory::service::add_movement(db, tenant, id, delta, reason, admin, None).await?;
    Ok(product)
}

pub async fn stats(db: &Db, tenant: &Thing) -> DomainResult<ProductStats> {
    repo::stats(db, tenant, LOW_STOCK_DEFAULT).await
}

pub async fn bulk_price(db: &Db, tenant: &Thing, req: BulkPrice) -> DomainResult<usize> {
    let v = req.value;
    // Translate the API-layer mode into a type-safe `PriceOp`. The repo
    // binds the numeric operand — no SQL string is built from user input.
    let op = match req.mode {
        BulkPriceMode::Percent => {
            if v <= Decimal::from(-100) {
                return Err(DomainError::Invalid(
                    "el porcentaje dejaría el precio en cero o negativo".into(),
                ));
            }
            // `Percent(v)` semantics: new = price * (100 + v) / 100
            //                       = price * ((100 + v) / 100)
            // Precompute the multiplier here so the repo only sees a
            // bound Decimal.
            let multiplier = (Decimal::from(100) + v) / Decimal::from(100);
            repo::PriceOp::MultiplyPct(multiplier)
        }
        BulkPriceMode::Amount => repo::PriceOp::DeltaAbs(v),
    };
    let category = match req.category.as_deref() {
        Some(s) if !s.is_empty() => Some(resolve_category(db, tenant, s).await?),
        _ => None,
    };
    let update = repo::PriceUpdate {
        op,
        floor_at_zero: true,
        round: req.round,
    };
    repo::bulk_update_price_typed(db, tenant, update, category).await
}

pub async fn etiquetas(db: &Db, tenant: &Thing, q: &str) -> DomainResult<EtiquetaResults> {
    repo::etiquetas(db, tenant, q).await
}

// --- variants (multi-SKU retail, migración 0034 / Opción A) -----------------

/// Build a display name from parent + attrs when the caller omits `name`.
fn variant_display_name(parent_name: &str, attrs: &Option<serde_json::Value>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(serde_json::Value::Object(map)) = attrs {
        for key in ["talla", "color", "sku"] {
            if let Some(serde_json::Value::String(s)) = map.get(key) {
                let t = s.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }
    }
    if parts.is_empty() {
        format!("{parent_name} (variante)")
    } else {
        format!("{parent_name} — {}", parts.join(" / "))
    }
}

/// Create a sellable variant under `parent_id`. The child is a full `product`
/// row (own stock, own barcode, own price) so POS/FEFO/compras reuse the
/// existing product path. Nested variants are rejected.
pub async fn create_variant(
    db: &Db,
    tenant: &Thing,
    parent_id: &str,
    input: NewVariant,
) -> DomainResult<ProductDto> {
    let parent_thing = parse_thing(parent_id)?;
    if parent_thing.tb != "product" {
        return Err(DomainError::Invalid(
            "parent_id debe ser un id de producto".into(),
        ));
    }
    let parent = repo::get_product(db, tenant, &parent_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    if parent.parent_id.is_some() {
        return Err(DomainError::Invalid(
            "no se pueden anidar variantes (el padre ya es una variante)".into(),
        ));
    }
    if input.stock < 0 {
        return Err(DomainError::Invalid(
            "stock de variante no puede ser negativo".into(),
        ));
    }
    if let Some(p) = input.price {
        if p < Decimal::ZERO {
            return Err(DomainError::Invalid("precio no puede ser negativo".into()));
        }
    }
    let barcode = input
        .barcode
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if let Some(ref code) = barcode {
        if let Some(existing) = repo::product_id_by_barcode(db, tenant, code).await? {
            return Err(DomainError::Conflict(format!(
                "el código de barras '{code}' ya está asignado a {}",
                existing
            )));
        }
    }

    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| variant_display_name(&parent.name, &input.attrs));
    let price = input.price.unwrap_or(parent.price);
    let cost_price = input.cost_price.or(parent.cost_price);
    let category = parent
        .category
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());

    let base = input
        .slug
        .as_deref()
        .map(slugify)
        .unwrap_or_else(|| slugify(&name));
    let slug = unique_slug(&base, |s| {
        let db = db.clone();
        let tenant = tenant.clone();
        async move { repo::product_slug_exists(&db, &tenant, &s).await }
    })
    .await?;

    let mut created = repo::create_variant_product(
        db,
        tenant,
        &parent_thing,
        &slug,
        &name,
        price,
        cost_price,
        input.stock,
        input.attrs,
        input.external_id,
        input.image_url,
        category,
        parent.physical_stock,
    )
    .await?;

    if let Some(code) = barcode {
        let child = parse_thing(&created.id)?;
        // CREATE (not UPSERT): never steal another product's barcode under race.
        if let Err(e) = repo::create_barcode(db, tenant, &child, &code).await {
            // Hard-delete just-created orphan (no movements yet) so list/POS
            // never see a half-claimed variant after a barcode race.
            let _ = repo::hard_delete_product(db, tenant, &child).await;
            // Concurrent unique-index races often surface as raw DB_ERROR from
            // Surreal before our pre-check sees the winner — normalize to
            // CONFLICT so the POS/admin client gets a stable ES contract.
            if repo::product_id_by_barcode(db, tenant, &code)
                .await?
                .is_some()
            {
                return Err(DomainError::Conflict(format!(
                    "el código de barras '{code}' ya está asignado"
                )));
            }
            return Err(e);
        }
        created.barcode = Some(code);
    }

    Ok(created)
}

/// List variants of a parent product (empty if the product has none).
/// Ordered by name; each DTO carries own `stock` and optional `barcode`.
pub async fn list_variants(
    db: &Db,
    tenant: &Thing,
    parent_id: &str,
) -> DomainResult<Vec<ProductDto>> {
    let parent_thing = parse_thing(parent_id)?;
    // 404 if parent not in tenant (don't leak existence across tenants).
    let _ = repo::get_product(db, tenant, &parent_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    let mut kids = repo::list_variants(db, tenant, &parent_thing).await?;
    let pids: Vec<Thing> = kids
        .iter()
        .filter_map(|k| parse_thing(&k.id).ok())
        .collect();
    let map = repo::barcodes_of_products(db, tenant, &pids).await?;
    for kid in &mut kids {
        if let Some(code) = map.get(&kid.id) {
            kid.barcode = Some(code.clone());
        }
    }
    Ok(kids)
}

/// Resolve a tenant barcode to the sellable product (variant or plain SKU).
///
/// If the mapped product is a multi-SKU **parent** (has active children),
/// returns [`DomainError::Invalid`] with the same stable ES fragments as the
/// POS parent-sale guard — the scan path must never hand the cashier a
/// non-sellable shell (client POS adds by-barcode hits without a second check).
/// Soft-deleted (`active = false`) products are treated as not found so POS
/// never sells a deactivated SKU (barcode is also freed on delete).
pub async fn find_by_barcode(db: &Db, tenant: &Thing, barcode: &str) -> DomainResult<ProductDto> {
    let code = barcode.trim();
    if code.is_empty() {
        return Err(DomainError::Invalid("código de barras vacío".into()));
    }
    let pid = repo::product_id_by_barcode(db, tenant, code)
        .await?
        .ok_or(DomainError::NotFound)?;
    let mut p = repo::get_product(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    if !p.active {
        return Err(DomainError::NotFound);
    }
    if repo::has_active_variants(db, tenant, &pid).await? {
        return Err(DomainError::Invalid(format!(
            "el producto '{}' tiene variantes; venda por talla/SKU o \
             escanee el código de barras de la variante",
            p.name
        )));
    }
    p.barcode = Some(code.to_string());
    Ok(p)
}

/// `true` when the product has at least one active child variant.
pub async fn has_active_variants(db: &Db, tenant: &Thing, product_id: &str) -> DomainResult<bool> {
    let pid = parse_thing(product_id)?;
    repo::has_active_variants(db, tenant, &pid).await
}

/// Read-side sum of active children stock. Does **not** write `parent.stock`
/// (stock ledger + plain farmacia SKUs live on the product row itself).
pub async fn variants_stock_sum(db: &Db, tenant: &Thing, parent_id: &str) -> DomainResult<i64> {
    let parent_thing = parse_thing(parent_id)?;
    let _ = repo::get_product(db, tenant, &parent_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    repo::sum_children_stock(db, tenant, &parent_thing).await
}

/// Enrich a product DTO with barcode + variants_stock when applicable.
pub async fn enrich_product_dto(
    db: &Db,
    tenant: &Thing,
    mut p: ProductDto,
) -> DomainResult<ProductDto> {
    if let Ok(pid) = parse_thing(&p.id) {
        if p.barcode.is_none() {
            p.barcode = repo::barcode_of_product(db, tenant, &pid).await?;
        }
        // Only parents (no parent_id) get variants_stock / variant_count.
        if p.parent_id.is_none() {
            if let Some(&(sum, count)) =
                repo::children_agg_by_parents(db, tenant, std::slice::from_ref(&pid))
                    .await?
                    .get(&p.id)
            {
                p.variants_stock = Some(sum);
                p.variant_count = Some(count);
            }
        }
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basics() {
        assert_eq!(slugify("Paracetamol 500mg"), "paracetamol-500mg");
        assert_eq!(slugify("  Ibuprofeno   Forte  "), "ibuprofeno-forte");
        assert_eq!(slugify("Ácido Acetilsalicílico"), "acido-acetilsalicilico");
        assert_eq!(slugify("Niño/Ñandú"), "nino-nandu");
        assert_eq!(slugify("***"), "item");
    }

    #[test]
    fn variant_name_from_attrs() {
        let attrs = Some(serde_json::json!({"talla": "M", "color": "Negro"}));
        assert_eq!(
            variant_display_name("Polera básica", &attrs),
            "Polera básica — M / Negro"
        );
        assert_eq!(
            variant_display_name("Polera básica", &None),
            "Polera básica (variante)"
        );
    }
}
