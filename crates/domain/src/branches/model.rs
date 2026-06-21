//! Sucursales (branch) + cajas (register) DTOs and inputs — config center.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// --- branch (sucursal) -----------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BranchDto {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub comuna: Option<String>,
    pub phone: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewBranch {
    pub name: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub comuna: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateBranch {
    pub name: Option<String>,
    pub code: Option<String>,
    pub address: Option<String>,
    pub comuna: Option<String>,
    pub phone: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct BranchFilters {
    /// `None` lists all; `Some(true|false)` filters by activo.
    pub active: Option<bool>,
}

// --- register (caja) -------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterDto {
    pub id: String,
    /// Sucursal a la que pertenece la caja (`branch:<key>`), si está asignada.
    pub branch: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewRegister {
    pub name: String,
    /// Sucursal opcional (`branch:<key>`). Debe pertenecer al mismo tenant.
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateRegister {
    pub name: Option<String>,
    /// Reasigna la sucursal (`branch:<key>`); debe pertenecer al tenant. No hay
    /// forma de desasignar en v1 (se reasigna, no se limpia).
    pub branch: Option<String>,
    pub code: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct RegisterFilters {
    pub active: Option<bool>,
    /// Filtra cajas de una sucursal (`branch:<key>`).
    pub branch: Option<String>,
}
