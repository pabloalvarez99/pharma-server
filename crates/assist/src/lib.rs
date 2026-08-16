//! `assist` — the business agent's brain (ADR-0016, "Pregúntale a tu negocio").
//!
//! North-star differentiator: 1 RUT = 1 negocio = 1 agente IA. This crate turns
//! a Spanish question from the owner into a grounded answer over their own
//! tenant data, 100% offline-first (ADR-0005): a deterministic es-CL intent
//! parser + read-only executors over the existing `domain` services.
//!
//! The [`AssistProvider`] seam keeps the door open for an OPT-IN LLM provider
//! later (owner-supplied key, default OFF) without rewriting any of this. The
//! MVP ships only the [`Deterministic`] provider.

pub mod actions;
pub mod deterministic;
pub mod feria_caja;
pub mod intent;
pub mod llm;
pub mod money;
pub mod provider;

pub use actions::{
    build, execute, parse_action, store, Action, ActionOutcome, ActionParse, ActionProposal,
    ActionStore, BuildOutcome, VentaLinea, VentaLineaParse,
};
pub use deterministic::Deterministic;
pub use intent::{parse, Intent};
pub use llm::{AnthropicBackend, ChatBackend, LlmProvider};
pub use money::Money;
pub use provider::{select_provider, Answer, AssistConfig, AssistProvider, AssistQuery};
