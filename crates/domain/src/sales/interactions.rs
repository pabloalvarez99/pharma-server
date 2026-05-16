//! Drug-interaction checker — Beers criteria + Vademécum CL.
//!
//! Reference port of Tu Farmacia `apps/web/src/lib/drug-interactions.ts` (≈370
//! LoC of curated pair rules). Full ruleset port lives in [`rules`] (TODO,
//! tracked next-session in Fase 4 service slice). This module exposes the
//! public [`check`] entry point and types so [`super::service::post_sale`]
//! can already wire the call (returning empty `Vec` until rules ship).
//!
//! Caveat (mirrored from source): rules are referential, NEVER replace
//! pharmacist judgement. POS surfaces warnings; never blocks dispensing.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critica,
    Mayor,
    Moderada,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InteractionDetail {
    pub severity: Severity,
    /// Canonical alphabetical pair.
    pub drugs: (String, String),
    pub effect: String,
    pub recommendation: String,
}

/// Check active-ingredient pairs in a cart for known critical interactions.
///
/// `active_ingredients`: deduplicated lowercase tokens of each cart item's
/// `product.active_ingredient` (tokenization mirrors Tu Farmacia drug-info).
///
/// Returns matches ordered by severity descending (`Critica` first).
///
/// Currently returns empty `Vec` — full ruleset port pending (tracked Fase 4
/// service slice). The signature is stable so `service::post_sale` can wire
/// the call now.
pub fn check(_active_ingredients: &[String]) -> Vec<InteractionDetail> {
    Vec::new()
}
