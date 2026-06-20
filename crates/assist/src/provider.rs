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
    /// Present only when the question is a WRITE request: a two-step action
    /// proposal the client confirms via `POST /assist/act` (ADR-0016, Wave 3).
    /// Omitted for every read answer, so the read contract is unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<crate::actions::ActionProposal>,
}

impl Answer {
    pub fn new(intent: &Intent, text: impl Into<String>) -> Self {
        Self {
            intent: intent.label().to_string(),
            text: text.into(),
            data: None,
            action: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Build a write-action proposal answer. `intent` label is a fixed
    /// `accion_propuesta`; the structured proposal rides in `action`.
    pub fn proposal(proposal: crate::actions::ActionProposal) -> Self {
        Self {
            intent: "accion_propuesta".to_string(),
            text: format!("{} Confírmala para que la ejecute.", proposal.summary),
            data: None,
            action: Some(proposal),
        }
    }

    /// A plain es-CL note carrying no proposal (e.g. missing data, no
    /// permission, unknown supplier). `intent` label is `accion`.
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            intent: "accion".to_string(),
            text: text.into(),
            data: None,
            action: None,
        }
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

/// Runtime configuration for which provider answers (ADR-0016 "Camino LLM
/// opt-in"). The default is **fully offline**: the deterministic provider, no
/// network, no model, no tenant data ever leaving the machine (ADR-0005
/// invariants 1, 2, 3).
///
/// The LLM path is **OPT-IN and not yet implemented**. The fields below are the
/// plumbing the founder flips on later: even with `llm_enabled = true` this
/// build still answers with [`crate::Deterministic`] and makes **zero** network
/// calls — there is no `LlmProvider` compiled in. See [`select_provider`].
#[derive(Debug, Clone, Default)]
pub struct AssistConfig {
    /// Owner opt-in toggle for the future LLM provider. **Default OFF** (the
    /// derived `bool` default is `false`).
    pub llm_enabled: bool,
    /// Owner-supplied LLM API key, stored locally (same consent model as
    /// telemetry, ADR-0005 inv. 3). Never transmitted in this build. `None` =
    /// no key configured.
    pub llm_api_key: Option<String>,
}

impl AssistConfig {
    /// True only when the owner explicitly opted in AND supplied a key. Even
    /// then, no LLM provider exists yet — see [`select_provider`].
    pub fn llm_active(&self) -> bool {
        self.llm_enabled && self.llm_api_key.is_some()
    }
}

/// Pick the provider for a request given the owner's [`AssistConfig`].
///
/// Today this **always** returns the offline [`crate::Deterministic`] provider.
/// When `llm_active()` is set we log a one-line notice and still fall back to
/// deterministic — wiring an `LlmProvider` here is the single, isolated change
/// the founder makes to flip the opt-in path on (ADR-0016), without touching
/// the endpoint, the parser, or any executor.
pub fn select_provider(cfg: &AssistConfig) -> Box<dyn AssistProvider> {
    if cfg.llm_active() {
        tracing::warn!(
            "assist: llm opt-in is set but no LlmProvider is compiled in; \
             answering offline with the deterministic provider"
        );
    }
    Box::new(crate::Deterministic)
}
