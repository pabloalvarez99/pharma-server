//! License document schema. Mirrors `docs/strategy/license-architecture.md` §2.1.
//!
//! Forward-compat rule: parsers reject `schema_version > SCHEMA_VERSION` with
//! a clear error (binary is stale). Unknown optional fields are tolerated by
//! serde's default behavior.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current supported schema version. Bump = new ADR + migration plan.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Business,
    Enterprise,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Pro => "pro",
            Tier::Business => "business",
            Tier::Enterprise => "enterprise",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoughtAddon {
    pub addon_id: String,
    pub feature_keys: Vec<String>,
    pub purchased_at: DateTime<Utc>,
    pub order_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_renewal_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_sla_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_label: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub schema_version: u32,
    pub license_id: String,
    pub tenant_id: Uuid,
    pub tier: Tier,
    pub features: Vec<String>,
    #[serde(default)]
    pub bought_addons: Vec<BoughtAddon>,
    pub seat_count: u32,
    pub issued_at: DateTime<Utc>,
    /// `null` legal only when `tier == Free` (perpetuo).
    pub expires_at: Option<DateTime<Utc>>,
    pub issuer_did: String,
    /// Identifies which licenser key signed this license (ADR-0007 rotation).
    /// Absent/empty → legacy license, verified against `keys::LEGACY_KEY_ID`
    /// so a key rotation never invalidates licenses minted before this field
    /// existed.
    #[serde(default)]
    pub key_id: String,
    /// Base64 (standard, padded) of the 64-byte Ed25519 signature over the
    /// canonical-JSON encoding of this struct with `signature` removed.
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl License {
    /// Default Free-tier license used when no file is present on disk.
    /// Entitles the features documented as included in Free
    /// (license-architecture.md §9): todos los reportes sobre los datos del
    /// propio negocio, más `federation.receive_cards`.
    ///
    /// La lista se mantiene explícita aunque `gate::entitled` ya conceda
    /// cualquier feature de tier Free sin mirarla: este struct es lo que
    /// contesta `GET /api/v1/license`, y un resumen que no nombre lo que el
    /// negocio tiene incluido no le sirve a nadie para saber qué puede usar.
    pub fn free_default(tenant_id: Uuid) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            license_id: format!("lic_free_{tenant_id}"),
            tenant_id,
            tier: Tier::Free,
            features: vec![
                "reports.sales_daily".to_string(),
                "reports.margins_daily".to_string(),
                "reports.top_products".to_string(),
                "reports.stock_rotation".to_string(),
                "reports.near_expiry".to_string(),
                "federation.receive_cards".to_string(),
            ],
            bought_addons: Vec::new(),
            seat_count: 1,
            issued_at: chrono::Utc::now(),
            expires_at: None,
            issuer_did: String::new(),
            key_id: String::new(),
            signature: String::new(),
            metadata: None,
        }
    }
}
