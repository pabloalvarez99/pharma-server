//! Customers business logic: name/RUT validation, app-level RUT uniqueness
//! per tenant, soft delete. Thin orchestration over [`super::repo`].

use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn normalize_rut(raw: &str) -> String {
    raw.trim()
        .replace([' ', '.'], "")
        .replace('-', "")
        .to_uppercase()
}

fn clean_optional(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub async fn create_customer(
    db: &Db,
    tenant: &Thing,
    mut input: NewCustomer,
) -> DomainResult<CustomerDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    input.name = input.name.trim().to_string();
    input.phone = clean_optional(input.phone);
    input.email = clean_optional(input.email);
    input.rut = clean_optional(input.rut).map(|r| normalize_rut(&r));
    if let Some(rut) = input.rut.as_deref() {
        if repo::rut_exists(db, tenant, rut, None).await? {
            return Err(DomainError::Conflict(
                "ya existe un cliente con ese RUT".into(),
            ));
        }
    }
    repo::create_customer(db, tenant, &input).await
}

pub async fn list_customers(
    db: &Db,
    tenant: &Thing,
    filters: CustomerFilters,
) -> DomainResult<Vec<CustomerDto>> {
    repo::list_customers(db, tenant, &filters).await
}

pub async fn get_customer(db: &Db, tenant: &Thing, id: &str) -> DomainResult<CustomerDto> {
    let id = parse_thing(id)?;
    repo::get_customer(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_customer(
    db: &Db,
    tenant: &Thing,
    id: &str,
    mut patch: UpdateCustomer,
) -> DomainResult<CustomerDto> {
    let pid = parse_thing(id)?;
    patch.name = patch
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    patch.phone = clean_optional(patch.phone);
    patch.email = clean_optional(patch.email);
    patch.rut = clean_optional(patch.rut).map(|r| normalize_rut(&r));
    if let Some(rut) = patch.rut.as_deref() {
        if repo::rut_exists(db, tenant, rut, Some(&pid)).await? {
            return Err(DomainError::Conflict(
                "ya existe un cliente con ese RUT".into(),
            ));
        }
    }
    repo::update_customer(db, tenant, &pid, &patch).await
}

pub async fn delete_customer(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_customer(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

// --- loyalty ---------------------------------------------------------------

pub async fn list_loyalty(
    db: &Db,
    tenant: &Thing,
    filters: LoyaltyFilters,
) -> DomainResult<Vec<LoyaltyTxDto>> {
    repo::list_loyalty(db, tenant, &filters).await
}

pub async fn loyalty_stats(db: &Db, tenant: &Thing) -> DomainResult<LoyaltyStats> {
    repo::loyalty_stats(db, tenant).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rut_normalizes() {
        assert_eq!(normalize_rut(" 12.345.678-9 "), "123456789");
        assert_eq!(normalize_rut("11111111k"), "11111111K");
    }
}
