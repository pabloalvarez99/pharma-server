//! Feature gate API. Spec: `docs/strategy/license-architecture.md` §7.

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

use crate::schema::{License, Tier};

#[derive(Debug, Error)]
#[error("feature '{feature}' requiere tier '{tier_required}'")]
pub struct GateError {
    pub feature: String,
    pub tier_required: &'static str,
}

/// Non-fallible check. Use in UI ("¿muestro este botón?").
pub fn entitled(license: &License, feature: &str) -> bool {
    license.features.iter().any(|f| f == feature)
}

/// Fallible variant. Use in API handlers: map `GateError` → HTTP 402.
pub fn require(license: &License, feature: &str) -> Result<(), GateError> {
    if entitled(license, feature) {
        Ok(())
    } else {
        Err(GateError {
            feature: feature.to_string(),
            tier_required: tier_required_for(feature).as_str(),
        })
    }
}

/// Hard expiry check: `now > expires_at + grace`. `tier=Free` with
/// `expires_at=None` never expires.
pub fn is_expired(license: &License, now: DateTime<Utc>, grace: Duration) -> bool {
    match license.expires_at {
        None => false,
        Some(exp) => now > exp + grace,
    }
}

/// In grace period: `expires_at < now <= expires_at + grace`.
pub fn is_in_grace(license: &License, now: DateTime<Utc>, grace: Duration) -> bool {
    match license.expires_at {
        None => false,
        Some(exp) => now > exp && now <= exp + grace,
    }
}

/// Minimum tier required to obtain a given feature. Catalog from
/// `docs/strategy/license-architecture.md` §9. Falls back to `Enterprise`
/// for unknown keys (conservative — fail closed).
fn tier_required_for(feature: &str) -> Tier {
    match feature {
        // Free
        "reports.sales_daily" | "federation.receive_cards" => Tier::Free,
        // Pro
        "reports.margins_daily"
        | "reports.top_products"
        | "reports.stock_rotation"
        | "reports.near_expiry"
        | "integrations.sii_dte_auto"
        | "integrations.isp_controlados_auto"
        | "integrations.telegram_bot"
        | "federation.quote_request"
        | "backup.local_30d"
        | "support.email_48h" => Tier::Pro,
        // Business
        "integrations.webhook_outbound"
        | "federation.po_create"
        | "federation.online_sync"
        | "backup.local_90d"
        | "support.email_24h_chat" => Tier::Business,
        // Enterprise
        "reports.custom_queries"
        | "branding.white_label"
        | "federation.multi_cluster"
        | "backup.s3_compat"
        | "support.sla_4h" => Tier::Enterprise,
        // Microtx-only (no minimum tier — purchased as addons). Report as Pro.
        "branding.custom_logo"
        | "branding.themes"
        | "seats.extra_cashier"
        | "support.premium_credits" => Tier::Pro,
        _ => Tier::Enterprise,
    }
}
