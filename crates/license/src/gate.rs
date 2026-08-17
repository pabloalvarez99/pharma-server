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
///
/// A feature whose minimum tier is Free is included for **everyone**, whatever
/// the license file happens to list. Free is the floor of the product, not an
/// entitlement that a stale, hand-edited or older-issued file can revoke: a
/// business that signed up before a feature moved down to Free would otherwise
/// keep being denied something the product now advertises as included, and
/// nobody would ever find out. Anything above Free is granted only by the file.
pub fn entitled(license: &License, feature: &str) -> bool {
    tier_required_for(feature) == Tier::Free || license.features.iter().any(|f| f == feature)
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
        // Free — los reportes sobre los datos del propio negocio.
        //
        // Se cobra lo que cuesta plata operar o carga responsabilidad
        // (integraciones con el SII, federación entre tenants, sync en la nube,
        // respaldo a S3, asientos extra, SLA). No se cobra por entender los
        // datos propios: un reporte es una consulta sobre las filas del propio
        // negocio en su propia instancia, su costo marginal es una query, y
        // cobrarlo es lo que hacía que el producto se sintiera demo.
        //
        // `near_expiry` es el caso que menos se sostenía: en comida y en
        // remedios, avisar que algo se vence es seguridad, no analítica.
        "reports.sales_daily"
        | "reports.margins_daily"
        | "reports.top_products"
        | "reports.stock_rotation"
        | "reports.near_expiry"
        | "federation.receive_cards" => Tier::Free,
        // Pro
        "integrations.sii_dte_auto"
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
