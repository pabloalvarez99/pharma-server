//! Sucursales + cajas business logic. Thin orchestration over [`super::repo`]:
//! trims/validates names, resolves + ownership-checks the optional
//! `register.branch` link, maps "not found" to [`DomainError::NotFound`].

use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn clean(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Parse + tenant-ownership-check a `branch:<key>` link supplied by the client.
/// Returns `Err` if the id is malformed or the branch isn't this tenant's, so a
/// register can never link to another tenant's sucursal.
async fn resolve_branch(db: &Db, tenant: &Thing, raw: &str) -> DomainResult<Thing> {
    let bid = parse_thing(raw)?;
    if bid.tb != "branch" {
        return Err(DomainError::Invalid(format!(
            "branch debe ser un id 'branch:<key>', recibido '{raw}'"
        )));
    }
    if !repo::branch_exists(db, tenant, &bid).await? {
        return Err(DomainError::NotFound);
    }
    Ok(bid)
}

// --- branch (sucursal) -----------------------------------------------------

pub async fn create_branch(
    db: &Db,
    tenant: &Thing,
    mut input: NewBranch,
) -> DomainResult<BranchDto> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(DomainError::Invalid("nombre de sucursal requerido".into()));
    }
    input.code = clean(input.code);
    input.address = clean(input.address);
    input.comuna = clean(input.comuna);
    input.phone = clean(input.phone);
    repo::create_branch(db, tenant, &input).await
}

pub async fn list_branches(
    db: &Db,
    tenant: &Thing,
    filters: BranchFilters,
) -> DomainResult<Vec<BranchDto>> {
    repo::list_branches(db, tenant, &filters).await
}

pub async fn get_branch(db: &Db, tenant: &Thing, id: &str) -> DomainResult<BranchDto> {
    let id = parse_thing(id)?;
    repo::get_branch(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_branch(
    db: &Db,
    tenant: &Thing,
    id: &str,
    mut patch: UpdateBranch,
) -> DomainResult<BranchDto> {
    let bid = parse_thing(id)?;
    patch.name = patch
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    patch.code = clean(patch.code);
    patch.address = clean(patch.address);
    patch.comuna = clean(patch.comuna);
    patch.phone = clean(patch.phone);
    repo::update_branch(db, tenant, &bid, &patch)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn delete_branch(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_branch(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

// --- register (caja) -------------------------------------------------------

pub async fn create_register(
    db: &Db,
    tenant: &Thing,
    mut input: NewRegister,
) -> DomainResult<RegisterDto> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(DomainError::Invalid("nombre de caja requerido".into()));
    }
    input.code = clean(input.code);
    let branch = match clean(input.branch) {
        Some(raw) => Some(resolve_branch(db, tenant, &raw).await?),
        None => None,
    };
    repo::create_register(
        db,
        tenant,
        &input.name,
        branch.as_ref(),
        input.code.as_deref(),
    )
    .await
}

pub async fn list_registers(
    db: &Db,
    tenant: &Thing,
    filters: RegisterFilters,
) -> DomainResult<Vec<RegisterDto>> {
    let branch = match filters
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(parse_thing(raw)?),
        None => None,
    };
    repo::list_registers(db, tenant, &filters, branch.as_ref()).await
}

pub async fn get_register(db: &Db, tenant: &Thing, id: &str) -> DomainResult<RegisterDto> {
    let id = parse_thing(id)?;
    repo::get_register(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_register(
    db: &Db,
    tenant: &Thing,
    id: &str,
    mut patch: UpdateRegister,
) -> DomainResult<RegisterDto> {
    let rid = parse_thing(id)?;
    patch.name = patch
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    patch.code = clean(patch.code);
    let branch = match clean(patch.branch) {
        Some(raw) => Some(resolve_branch(db, tenant, &raw).await?),
        None => None,
    };
    repo::update_register(
        db,
        tenant,
        &rid,
        patch.name.as_deref(),
        branch.as_ref(),
        patch.code.as_deref(),
        patch.active,
    )
    .await?
    .ok_or(DomainError::NotFound)
}

pub async fn delete_register(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_register(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}
