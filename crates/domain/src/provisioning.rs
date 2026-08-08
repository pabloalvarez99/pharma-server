//! Tenant provisioning — shared logic behind `pharma tenant-create` /
//! `pharma user-create` (CLI) and `POST /admin/v1/tenants` (SaaS signup via
//! license-server). One implementation so CLI and API can never drift.
//!
//! The caller hashes the password (this crate never depends on `auth`);
//! everything else — slug derivation, RUT/vertical validation, duplicate
//! detection, tenant + admin user + `admin_setting` writes — lives here.

use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// `admin_setting` keys written on provisioning. `business.vertical` is the
/// same key first-run onboarding (`/api/v1/setup`) stores.
pub const SETTING_VERTICAL: &str = "business.vertical";
pub const SETTING_BUSINESS_NAME: &str = "business.name";
pub const SETTING_RUT: &str = "business.rut";

#[derive(Debug, serde::Deserialize)]
struct IdRow {
    id: Thing,
}

/// Input for [`provision_tenant`]. `rut` and `vertical` must already be
/// validated/normalized via [`validate_rut`] / [`validate_vertical`].
#[derive(Debug)]
pub struct ProvisionInput {
    /// Explicit slug; `None` derives one from `business_name`.
    pub slug: Option<String>,
    pub business_name: String,
    /// Normalized RUT (no dots/dash, uppercase K), e.g. `765432103`.
    pub rut: String,
    /// Rubro key from the catalog (`crate::rubro`), e.g. `minimarket`.
    pub vertical: String,
    pub admin_email: String,
    /// argon2id hash — never the plaintext password.
    pub admin_password_hash: String,
}

#[derive(Debug)]
pub struct ProvisionedTenant {
    /// Record id, e.g. `tenant:abc123`.
    pub tenant_id: String,
    pub slug: String,
    /// Admin user record id, e.g. `user:xyz`.
    pub user_id: String,
}

/// Normalize a RUT: strip dots/dashes/spaces, uppercase the DV.
pub fn normalize_rut(raw: &str) -> String {
    raw.trim().replace([' ', '.', '-'], "").to_uppercase()
}

/// Validate a Chilean RUT (módulo 11) and return it normalized.
pub fn validate_rut(raw: &str) -> DomainResult<String> {
    let rut = normalize_rut(raw);
    if rut.len() < 2 {
        return Err(DomainError::Invalid(
            "RUT inválido: demasiado corto (ej: 76.543.210-3).".into(),
        ));
    }
    let (body, dv) = rut.split_at(rut.len() - 1);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit()) {
        return Err(DomainError::Invalid(
            "RUT inválido: el cuerpo debe ser numérico (ej: 76.543.210-3).".into(),
        ));
    }
    let mut sum: u32 = 0;
    let mut factor: u32 = 2;
    for c in body.chars().rev() {
        sum += c.to_digit(10).expect("digit checked above") * factor;
        factor = if factor == 7 { 2 } else { factor + 1 };
    }
    let expected = match 11 - (sum % 11) {
        11 => '0',
        10 => 'K',
        n => char::from_digit(n, 10).expect("0-9"),
    };
    if !dv.starts_with(expected) {
        return Err(DomainError::Invalid(
            "RUT inválido: dígito verificador no coincide.".into(),
        ));
    }
    Ok(rut)
}

/// Validate a vertical against the rubro catalog and return it lowercased.
pub fn validate_vertical(raw: &str) -> DomainResult<String> {
    let v = raw.trim().to_lowercase();
    if crate::rubro::all_rubros().contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(DomainError::Invalid(format!(
            "Rubro '{raw}' fuera de catálogo. Válidos: {}.",
            crate::rubro::all_rubros().join(", ")
        )))
    }
}

/// Create a tenant. `Conflict` when the slug is already taken.
pub async fn create_tenant(db: &Db, name: &str, slug: &str) -> DomainResult<Thing> {
    let mut q = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?;
    let existing: Option<IdRow> = q.take(0)?;
    if existing.is_some() {
        return Err(DomainError::Conflict(format!(
            "Ya existe un negocio con la sucursal '{slug}'."
        )));
    }
    let mut res = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", name.to_string()))
        .bind(("slug", slug.to_string()))
        .await?;
    let row: Option<IdRow> = res.take(0)?;
    row.map(|r| r.id)
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("CREATE tenant returned no row")))
}

/// Create a user for a tenant. `password_hash` is a ready argon2id hash.
pub async fn create_user(
    db: &Db,
    tenant: &Thing,
    email: &str,
    password_hash: &str,
    roles: &[String],
) -> DomainResult<Thing> {
    let mut res = db
        .query(
            "CREATE user SET tenant = $tenant, email = $email, \
             password = $password, roles = $roles RETURN AFTER",
        )
        .bind(("tenant", tenant.clone()))
        .bind(("email", email.to_string()))
        .bind(("password", password_hash.to_string()))
        .bind(("roles", roles.to_vec()))
        .await?;
    let row: Option<IdRow> = res.take(0)?;
    row.map(|r| r.id)
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("CREATE user returned no row")))
}

/// `true` when some tenant already registered this (normalized) RUT.
pub async fn rut_taken(db: &Db, rut: &str) -> DomainResult<bool> {
    let mut q = db
        .query("SELECT id FROM admin_setting WHERE key = $key AND value = $rut LIMIT 1")
        .bind(("key", SETTING_RUT.to_string()))
        .bind(("rut", rut.to_string()))
        .await?;
    let row: Option<IdRow> = q.take(0)?;
    Ok(row.is_some())
}

/// Provision a full tenant: tenant row + admin/owner user + business settings
/// (`business.vertical`, `business.name`, `business.rut`).
///
/// `Conflict` on duplicate slug or RUT. Input is assumed pre-validated
/// ([`validate_rut`], [`validate_vertical`]); `business_name`/`admin_email`
/// emptiness is re-checked here as a last line of defense.
pub async fn provision_tenant(db: &Db, input: ProvisionInput) -> DomainResult<ProvisionedTenant> {
    let business_name = input.business_name.trim();
    if business_name.is_empty() {
        return Err(DomainError::Invalid("Indica el nombre del negocio.".into()));
    }
    let email = input.admin_email.trim().to_lowercase();
    if email.is_empty() {
        return Err(DomainError::Invalid("Indica el correo del admin.".into()));
    }

    let raw_slug = input
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(crate::catalog::service::slugify)
        .unwrap_or_else(|| crate::catalog::service::slugify(business_name));
    let slug = if raw_slug.is_empty() {
        return Err(DomainError::Invalid(
            "No se pudo derivar una sucursal (slug) del nombre; indica una.".into(),
        ));
    } else {
        raw_slug
    };

    if rut_taken(db, &input.rut).await? {
        return Err(DomainError::Conflict(format!(
            "Ya existe un negocio registrado con el RUT {}.",
            input.rut
        )));
    }

    let tenant = create_tenant(db, business_name, &slug).await?;
    let roles = vec!["owner".to_string(), "admin".to_string()];
    let user = create_user(db, &tenant, &email, &input.admin_password_hash, &roles).await?;

    crate::sales::service::set_setting(db, &tenant, SETTING_VERTICAL, &input.vertical).await?;
    crate::sales::service::set_setting(db, &tenant, SETTING_BUSINESS_NAME, business_name).await?;
    crate::sales::service::set_setting(db, &tenant, SETTING_RUT, &input.rut).await?;

    Ok(ProvisionedTenant {
        tenant_id: tenant.to_string(),
        slug,
        user_id: user.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rut_valid_cases() {
        assert_eq!(validate_rut("76.543.210-3").unwrap(), "765432103");
        assert_eq!(validate_rut("11.111.111-1").unwrap(), "111111111");
        // Lowercase k normalizes up.
        assert_eq!(
            validate_rut("10.994.906-k").unwrap(),
            validate_rut("10994906K").unwrap()
        );
    }

    #[test]
    fn rut_invalid_cases() {
        assert!(validate_rut("12.345.678-0").is_err(), "DV incorrecto");
        assert!(validate_rut("").is_err());
        assert!(validate_rut("K").is_err());
        assert!(validate_rut("ABC-3").is_err());
    }

    #[test]
    fn vertical_catalog_gate() {
        assert_eq!(validate_vertical("Minimarket").unwrap(), "minimarket");
        assert_eq!(validate_vertical("farmacia").unwrap(), "farmacia");
        assert!(validate_vertical("astronave").is_err());
    }
}
