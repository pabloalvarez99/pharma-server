//! Inbound federated purchase orders (`agent_order`) — the supplier-operator
//! side. A remote signed peer places an order via `POST /agent/inbox`
//! (`po.create`); this context lets the *human operator* of the supplier
//! tenant list those orders and decide: `accepted` or `rejected`. The buyer
//! learns the decision by polling the federated `po.status` topic.
//!
//! `agent_order` rows are written by the federated handler (not tenant/JWT
//! scoped at creation — authenticity is the Ed25519 signature). Here every
//! read/write IS tenant-scoped via the operator's JWT `tenant_id`, so one
//! tenant can never see or act on another's inbound orders.

pub mod model;
pub mod service;

pub use model::*;
