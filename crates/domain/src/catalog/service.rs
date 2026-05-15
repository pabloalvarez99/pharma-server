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
    repo::list_products(db, tenant, &filters).await
}

pub async fn get_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<ProductDto> {
    let id = parse_thing(id)?;
    repo::get_product(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
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
    repo::update_product(db, tenant, &pid, &patch, category).await
}

pub async fn delete_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_product(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

pub async fn adjust_stock(
    db: &Db,
    tenant: &Thing,
    id: &str,
    adj: StockAdjust,
) -> DomainResult<ProductDto> {
    let pid = parse_thing(id)?;
    let current = repo::get_product(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    let new_stock = match (adj.set, adj.delta) {
        (Some(s), None) => s,
        (None, Some(d)) => current.stock + d,
        _ => {
            return Err(DomainError::Invalid(
                "indique exactamente uno de `set` o `delta`".into(),
            ))
        }
    };
    if new_stock < 0 {
        return Err(DomainError::Invalid("stock resultante negativo".into()));
    }
    // NOTE: full audited movement (stock_movement) lands in Fase 3.
    repo::set_stock(db, tenant, &pid, new_stock).await
}

pub async fn stats(db: &Db, tenant: &Thing) -> DomainResult<ProductStats> {
    repo::stats(db, tenant, LOW_STOCK_DEFAULT).await
}

pub async fn bulk_price(db: &Db, tenant: &Thing, req: BulkPrice) -> DomainResult<usize> {
    let v = req.value;
    let inner = match req.mode {
        BulkPriceMode::Percent => {
            if v <= Decimal::from(-100) {
                return Err(DomainError::Invalid(
                    "el porcentaje dejaría el precio en cero o negativo".into(),
                ));
            }
            format!("price * (100dec + {v}dec) / 100dec")
        }
        BulkPriceMode::Amount => format!("price + {v}dec"),
    };
    let guarded = format!("math::max([0dec, {inner}])");
    let expr = if req.round {
        format!("math::round({guarded})")
    } else {
        guarded
    };
    let category = match req.category.as_deref() {
        Some(s) if !s.is_empty() => Some(resolve_category(db, tenant, s).await?),
        _ => None,
    };
    repo::bulk_update_price(db, tenant, &expr, category).await
}

pub async fn etiquetas(db: &Db, tenant: &Thing, q: &str) -> DomainResult<EtiquetaResults> {
    repo::etiquetas(db, tenant, q).await
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
}
