//! Agent identity + signed-envelope foundation (Fase 11 ecosystem).
//!
//! Each pharma-server install is a sovereign **agent** in a federated mesh of
//! ERP nodes (pharmacies, suppliers, distributors, labs) where real humans
//! operate each node and trade via a common protocol. This crate provides the
//! cryptographic primitives that make a node addressable and its messages
//! tamper-evident:
//!
//! * [`identity::Identity`] — an Ed25519 keypair persisted at install time.
//!   The public half yields a stable DID: `did:pharma:<bs58(pubkey)>`.
//! * [`card::AgentCard`] — self-signed public metadata for discovery
//!   (name, kind, region, endpoint).
//! * [`envelope::Envelope`] — a topic-tagged JSON message, signed over a
//!   canonical (sorted-key) byte encoding so any peer can verify authenticity
//!   without a central authority.
//!
//! No networking here — transport (HTTP push / optional relay / NATS) lives in
//! a future `sync`/transport crate. This crate is offline-pure and unit-tested.

pub mod card;
pub mod envelope;
pub mod identity;

mod canonical;

pub use card::{AgentCard, AgentKind};
pub use envelope::Envelope;
pub use identity::Identity;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("clave inválida: {0}")]
    Key(String),
    #[error("firma inválida")]
    BadSignature,
    #[error("DID inválido: {0}")]
    BadDid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialización: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;
