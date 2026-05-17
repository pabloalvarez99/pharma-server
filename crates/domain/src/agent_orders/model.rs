use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AgentOrderDto {
    pub id: String,
    pub peer_did: String,
    pub status: String,
    pub total: f64,
    pub currency: String,
    pub price_adjusted: bool,
    pub buyer_note: Option<String>,
    /// `lines_json` decoded back into a JSON array for the operator UI. Falls
    /// back to `null` if the stored string is not valid JSON.
    pub lines: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct AgentOrderFilters {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
