//! Sucursales + cajas persistence. Pure async fns over `&Db`. Every query is
//! tenant-scoped: callers pass the tenant `Thing` extracted from the JWT.
//!
//! `register.branch` is a `record<branch>` link (migration 0032), so it is
//! bound as a `Thing` (never a JSON string) — a SCHEMAFULL `TYPE record<branch>`
//! field rejects a plain string. The service validates branch ownership for the
//! tenant before any link is written here.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BranchRow {
    id: Thing,
    name: String,
    code: Option<String>,
    address: Option<String>,
    comuna: Option<String>,
    phone: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<BranchRow> for BranchDto {
    fn from(r: BranchRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            code: r.code,
            address: r.address,
            comuna: r.comuna,
            phone: r.phone,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegisterRow {
    id: Thing,
    branch: Option<Thing>,
    name: String,
    code: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<RegisterRow> for RegisterDto {
    fn from(r: RegisterRow) -> Self {
        Self {
            id: r.id.to_string(),
            branch: r.branch.map(|t| t.to_string()),
            name: r.name,
            code: r.code,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// --- branch (sucursal) -----------------------------------------------------

pub async fn create_branch(db: &Db, tenant: &Thing, input: &NewBranch) -> DomainResult<BranchDto> {
    let mut r = db
        .query(
            "CREATE branch SET tenant = $t, name = $name, code = $code, \
             address = $address, comuna = $comuna, phone = $phone RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", input.name.clone()))
        .bind(("code", input.code.clone()))
        .bind(("address", input.address.clone()))
        .bind(("comuna", input.comuna.clone()))
        .bind(("phone", input.phone.clone()))
        .await?;
    let row: Option<BranchRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_branches(
    db: &Db,
    tenant: &Thing,
    f: &BranchFilters,
) -> DomainResult<Vec<BranchDto>> {
    // Active filter is ALWAYS applied (default `true`): a soft-deleted (archived)
    // branch must drop out of the default list, otherwise soft-delete is a no-op
    // from the UI's view. `active=false` reveals the archived ones.
    let mut r = db
        .query("SELECT * FROM branch WHERE tenant = $t AND active = $active ORDER BY name")
        .bind(("t", tenant.clone()))
        .bind(("active", f.active.unwrap_or(true)))
        .await?;
    let rows: Vec<BranchRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_branch(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<Option<BranchDto>> {
    let mut r = db
        .query("SELECT * FROM branch WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<BranchRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

/// Cheap existence check used to validate a `register.branch` link belongs to
/// the tenant before it is written.
pub async fn branch_exists(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM branch WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn update_branch(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateBranch,
) -> DomainResult<Option<BranchDto>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.name.is_some() {
        sets.push("name = $name");
    }
    if patch.code.is_some() {
        sets.push("code = $code");
    }
    if patch.address.is_some() {
        sets.push("address = $address");
    }
    if patch.comuna.is_some() {
        sets.push("comuna = $comuna");
    }
    if patch.phone.is_some() {
        sets.push("phone = $phone");
    }
    if patch.active.is_some() {
        sets.push("active = $active");
    }
    if sets.is_empty() {
        return get_branch(db, tenant, id).await;
    }
    let q = format!(
        "UPDATE branch SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", ")
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("name", patch.name.clone()))
        .bind(("code", patch.code.clone()))
        .bind(("address", patch.address.clone()))
        .bind(("comuna", patch.comuna.clone()))
        .bind(("phone", patch.phone.clone()))
        .bind(("active", patch.active))
        .await?;
    let row: Option<BranchRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn soft_delete_branch(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE branch SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- register (caja) -------------------------------------------------------

pub async fn create_register(
    db: &Db,
    tenant: &Thing,
    name: &str,
    branch: Option<&Thing>,
    code: Option<&str>,
) -> DomainResult<RegisterDto> {
    let mut r = db
        .query(
            "CREATE register SET tenant = $t, name = $name, branch = $branch, \
             code = $code RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("branch", branch.cloned()))
        .bind(("code", code.map(str::to_string)))
        .await?;
    let row: Option<RegisterRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_registers(
    db: &Db,
    tenant: &Thing,
    f: &RegisterFilters,
    branch: Option<&Thing>,
) -> DomainResult<Vec<RegisterDto>> {
    // Active filter ALWAYS applied (default `true`) so archived cajas drop out
    // of the default list; `active=false` reveals them. Branch filter is
    // optional (None = cajas of every sucursal).
    let mut conds = vec!["tenant = $t".to_string(), "active = $active".to_string()];
    if branch.is_some() {
        conds.push("branch = $branch".to_string());
    }
    let q = format!(
        "SELECT * FROM register WHERE {} ORDER BY name",
        conds.join(" AND ")
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("active", f.active.unwrap_or(true)))
        .bind(("branch", branch.cloned()))
        .await?;
    let rows: Vec<RegisterRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_register(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<RegisterDto>> {
    let mut r = db
        .query("SELECT * FROM register WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<RegisterRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_register(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    name: Option<&str>,
    branch: Option<&Thing>,
    code: Option<&str>,
    active: Option<bool>,
) -> DomainResult<Option<RegisterDto>> {
    let mut sets: Vec<&str> = Vec::new();
    if name.is_some() {
        sets.push("name = $name");
    }
    if branch.is_some() {
        sets.push("branch = $branch");
    }
    if code.is_some() {
        sets.push("code = $code");
    }
    if active.is_some() {
        sets.push("active = $active");
    }
    if sets.is_empty() {
        return get_register(db, tenant, id).await;
    }
    let q = format!(
        "UPDATE register SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", ")
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("name", name.map(str::to_string)))
        .bind(("branch", branch.cloned()))
        .bind(("code", code.map(str::to_string)))
        .bind(("active", active))
        .await?;
    let row: Option<RegisterRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn soft_delete_register(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE register SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}
