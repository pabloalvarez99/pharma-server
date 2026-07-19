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
    /// Per-line FEFO breakdown recorded at fulfillment (BACKLOG #2 remainder,
    /// migration `0014`). `None` until the order is fulfilled and on
    /// fulfillments of non-batch-tracked products. Sibling of the sales-path
    /// `order_item.batches_json` (migration `0013`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulfillment_batches: Option<Vec<AgentOrderFulfillmentLine>>,
    pub created_at: DateTime<Utc>,
}

/// One line of the fulfillment breakdown. `allocations` is in FEFO consumption
/// order (earliest expiry first), with per-allocation `qty` summing to the
/// fulfilled line quantity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct AgentOrderFulfillmentLine {
    pub product: String,
    pub allocations: Vec<AgentOrderFulfillmentAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct AgentOrderFulfillmentAllocation {
    pub batch: String,
    pub qty: i64,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct AgentOrderFilters {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
