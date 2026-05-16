//! Purchasing business logic: validation, compare best supplier, mapping.
//! Thin orchestration over [`super::repo`].

use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};
use crate::money::CURRENCY_CLP;

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn parse_typed(s: &str, table: &str) -> DomainResult<Thing> {
    let t = parse_thing(s)?;
    if t.tb != table {
        return Err(DomainError::Invalid(format!(
            "id debe pertenecer a `{table}`"
        )));
    }
    Ok(t)
}

// --- suppliers -------------------------------------------------------------

pub async fn create_supplier(
    db: &Db,
    tenant: &Thing,
    input: NewSupplier,
) -> DomainResult<SupplierDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    repo::create_supplier(db, tenant, &input).await
}

pub async fn list_suppliers(
    db: &Db,
    tenant: &Thing,
    filters: SupplierFilters,
) -> DomainResult<Vec<SupplierDto>> {
    repo::list_suppliers(db, tenant, &filters).await
}

pub async fn get_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<SupplierDto> {
    let id = parse_typed(id, "supplier")?;
    repo::get_supplier(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_supplier(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateSupplier,
) -> DomainResult<SupplierDto> {
    let id = parse_typed(id, "supplier")?;
    repo::update_supplier(db, tenant, &id, &patch).await
}

pub async fn delete_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_typed(id, "supplier")?;
    if repo::soft_delete_supplier(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

// --- mapping ---------------------------------------------------------------

async fn resolve_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<Thing> {
    let t = parse_typed(id, "supplier")?;
    if !repo::supplier_belongs(db, tenant, &t).await? {
        return Err(DomainError::Invalid(
            "el proveedor no existe en este tenant".into(),
        ));
    }
    Ok(t)
}

async fn resolve_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<Thing> {
    let t = parse_typed(id, "product")?;
    if !repo::product_belongs(db, tenant, &t).await? {
        return Err(DomainError::Invalid(
            "el producto no existe en este tenant".into(),
        ));
    }
    Ok(t)
}

pub async fn map_product(
    db: &Db,
    tenant: &Thing,
    supplier_id: &str,
    input: MapSupplierProduct,
) -> DomainResult<SupplierProductMappingDto> {
    if input.supplier_code.trim().is_empty() {
        return Err(DomainError::Invalid("supplier_code requerido".into()));
    }
    let supplier = resolve_supplier(db, tenant, supplier_id).await?;
    let product = resolve_product(db, tenant, &input.product).await?;
    repo::create_mapping(db, tenant, &supplier, &product, input.supplier_code.trim())
        .await
        .map_err(|e| match e {
            // Unique-index violation surfaces as a generic Db error; map to
            // CONFLICT so the caller gets a 409 instead of 500.
            DomainError::Db(boxed) => {
                let msg = boxed.to_string().to_lowercase();
                if msg.contains("already") || msg.contains("index") || msg.contains("unique") {
                    DomainError::Conflict(
                        "ya existe un mapping para este (proveedor, supplier_code)".into(),
                    )
                } else {
                    DomainError::Db(boxed)
                }
            }
            other => other,
        })
}

// --- prices ----------------------------------------------------------------

pub async fn create_price(
    db: &Db,
    tenant: &Thing,
    input: NewSupplierPrice,
) -> DomainResult<SupplierPriceDto> {
    if input.unit_cost < Decimal::ZERO {
        return Err(DomainError::Invalid(
            "unit_cost no puede ser negativo".into(),
        ));
    }
    let supplier = resolve_supplier(db, tenant, &input.supplier).await?;
    let product = match input.product.as_deref() {
        Some(s) if !s.is_empty() => Some(resolve_product(db, tenant, s).await?),
        _ => None,
    };
    let currency = input.currency.as_deref().unwrap_or(CURRENCY_CLP);
    repo::create_price(
        db,
        tenant,
        &supplier,
        product,
        input.supplier_code.as_deref(),
        input.description.as_deref(),
        input.unit_cost,
        currency,
        input.valid_from,
    )
    .await
}

pub async fn list_prices(
    db: &Db,
    tenant: &Thing,
    filters: SupplierPriceFilters,
) -> DomainResult<Vec<SupplierPriceDto>> {
    repo::list_prices(db, tenant, &filters).await
}

/// Pick cheapest supplier for each requested item. If `product` is given,
/// `savings = product.cost_price − best.unit_cost`. If only `supplier_code`
/// is given, savings is `None`.
pub async fn compare(
    db: &Db,
    tenant: &Thing,
    req: CompareRequest,
) -> DomainResult<CompareResponse> {
    let mut out = Vec::with_capacity(req.items.len());
    for item in req.items {
        let (best, current_cost) = if let Some(pid) = item.product.as_deref() {
            let p = resolve_product(db, tenant, pid).await?;
            let best = repo::best_price_for_product(db, tenant, &p).await?;
            let cost = repo::product_cost_price(db, tenant, &p).await?;
            (best, cost)
        } else if let Some(code) = item.supplier_code.as_deref() {
            (repo::best_price_for_code(db, tenant, code).await?, None)
        } else {
            return Err(DomainError::Invalid(
                "cada item requiere `product` o `supplier_code`".into(),
            ));
        };
        let best = best.map(|(dto, supplier_name)| CompareBest {
            supplier: dto.supplier.clone(),
            supplier_name,
            unit_cost: dto.unit_cost,
            price_id: dto.id,
            valid_from: dto.valid_from,
        });
        let savings = match (&best, current_cost) {
            (Some(b), Some(c)) => Some(c - b.unit_cost),
            _ => None,
        };
        out.push(CompareResult {
            product: item.product,
            supplier_code: item.supplier_code,
            best,
            current_cost,
            savings,
        });
    }
    Ok(CompareResponse { items: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_typed_rejects_wrong_table() {
        let err = parse_typed("product:abc", "supplier").unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
    }
}
