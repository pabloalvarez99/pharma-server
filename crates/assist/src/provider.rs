//! The provider seam (ADR-0016).
//!
//! [`AssistProvider`] is the single abstraction the API depends on. The MVP
//! ships exactly one implementation — [`crate::deterministic::Deterministic`]
//! — that answers from local domain reads with NO network. A future
//! OPT-IN LLM provider (owner-supplied key, default OFF, telemetry-style
//! consent) plugs in behind the same trait without touching the endpoint or
//! the intent parser: it receives the same [`AssistQuery`] (raw question +
//! parsed intent + read-only db handle) and can either honour the deterministic
//! intent or run its own tool-calling over the db.

use async_trait::async_trait;
use db::Db;
use serde::Serialize;
use surrealdb::sql::Thing;

use crate::intent::Intent;

/// A fully-formed answer for the owner. `text` is Spanish prose grounded in
/// real tenant data; `data` carries the structured figures the UI can render
/// (charts, tables) without re-querying.
#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    /// Machine label of the resolved intent (see [`Intent::label`]).
    pub intent: String,
    /// Spanish, user-facing answer.
    pub text: String,
    /// Optional structured payload backing the prose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Answer {
    pub fn new(intent: &Intent, text: impl Into<String>) -> Self {
        Self {
            intent: intent.label().to_string(),
            text: text.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Everything a provider needs to answer one question. Read-only: the `db`
/// handle is shared and providers MUST NOT mutate (enforced by convention +
/// the fact that only read services are reachable here).
pub struct AssistQuery<'a> {
    /// The owner's raw, unmodified question (LLM providers want the original).
    pub question: &'a str,
    /// Deterministically parsed intent.
    pub intent: Intent,
    /// Read-only database handle.
    pub db: &'a Db,
    /// Tenant scope (from the JWT `tenant_id`). Every read is filtered by it.
    pub tenant: &'a Thing,
}

/// The seam. One method, async, returns a grounded [`Answer`] or a domain
/// error. Implementations are `Send + Sync` so the API can hold a trait object
/// in shared state.
#[async_trait]
pub trait AssistProvider: Send + Sync {
    async fn answer(&self, q: &AssistQuery<'_>) -> domain::DomainResult<Answer>;
}
