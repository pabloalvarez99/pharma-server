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

/// Customer detail + lifetime purchase aggregates. 404 when not found for the
/// tenant.
pub async fn get_customer_detail(
    db: &Db,
    tenant: &Thing,
    id: &str,
) -> DomainResult<CustomerDetailDto> {
    let id = parse_thing(id)?;
    repo::get_customer_detail(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

/// A customer's realized-purchase history, newest first. 404 when the customer
/// doesn't exist for the tenant (so callers don't confuse "no orders" with
/// "wrong customer/tenant").
pub async fn customer_history(
    db: &Db,
    tenant: &Thing,
    id: &str,
    filters: CustomerHistoryFilters,
) -> DomainResult<Vec<CustomerOrderDto>> {
    let cid = parse_thing(id)?;
    if repo::get_customer(db, tenant, &cid).await?.is_none() {
        return Err(DomainError::NotFound);
    }
    repo::customer_history(db, tenant, &cid, filters.limit.unwrap_or(50)).await
}

/// Fuzzy customer search (name/rut/phone). Empty/whitespace `q` yields `[]`.
pub async fn search_customers(
    db: &Db,
    tenant: &Thing,
    query: CustomerSearchQuery,
) -> DomainResult<Vec<CustomerDto>> {
    match query.q.as_deref() {
        Some(q) => repo::search_customers(db, tenant, q).await,
        None => Ok(Vec::new()),
    }
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

// --- bulk import (Supabase migration) -------------------------------------

/// Minimum viable email check — non-empty, contains `@`, has chars on both
/// sides. We don't run a full RFC 5322 regex (overkill for a bulk import
/// from a curated source like Supabase) but we do reject the obvious junk
/// so a single bad row in the CSV doesn't pollute the customer table.
fn is_email_shaped(s: &str) -> bool {
    let s = s.trim();
    let Some(at) = s.find('@') else {
        return false;
    };
    let (local, rest) = s.split_at(at);
    let domain = &rest[1..];
    !local.is_empty() && !domain.is_empty() && domain.contains('.')
}

/// Idempotent bulk upsert. One row's failure (invalid email, conflicting
/// RUT, transient DB error) only aborts that row — others continue and the
/// caller gets a per-index error list. Capped by
/// [`IMPORT_CUSTOMERS_MAX_BATCH`] at the API layer.
pub async fn import_customers(
    db: &Db,
    tenant: &Thing,
    req: ImportCustomersRequest,
) -> DomainResult<ImportCustomersResponse> {
    let mut created = 0u32;
    let mut updated = 0u32;
    let mut errors: Vec<ImportError> = Vec::new();

    for (idx, row) in req.customers.into_iter().enumerate() {
        let email = row.email.trim();
        if email.is_empty() {
            errors.push(ImportError {
                idx,
                message: "email requerido".into(),
            });
            continue;
        }
        if !is_email_shaped(email) {
            errors.push(ImportError {
                idx,
                message: format!("email inválido: {email}"),
            });
            continue;
        }
        let name = row.name.trim();
        if name.is_empty() {
            errors.push(ImportError {
                idx,
                message: "nombre requerido".into(),
            });
            continue;
        }
        let input = NewCustomer {
            name: name.to_string(),
            rut: clean_optional(row.rut).map(|r| normalize_rut(&r)),
            phone: clean_optional(row.phone),
            email: Some(email.to_lowercase()),
        };
        match repo::upsert_customer_by_email(db, tenant, &input).await {
            Ok(repo::UpsertOutcome::Created) => created += 1,
            Ok(repo::UpsertOutcome::Updated) => updated += 1,
            Err(e) => errors.push(ImportError {
                idx,
                message: e.to_string(),
            }),
        }
    }

    Ok(ImportCustomersResponse {
        created,
        updated,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rut_normalizes() {
        assert_eq!(normalize_rut(" 12.345.678-9 "), "123456789");
        assert_eq!(normalize_rut("11111111k"), "11111111K");
    }

    #[test]
    fn email_shape_check_filters_obvious_junk() {
        assert!(is_email_shaped("a@b.cl"));
        assert!(is_email_shaped("  user@host.com  "));
        assert!(!is_email_shaped(""));
        assert!(!is_email_shaped("plainstring"));
        assert!(!is_email_shaped("@host.com"));
        assert!(!is_email_shaped("user@"));
        assert!(!is_email_shaped("user@host")); // no TLD dot
    }
}
