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
//! * [`relay`] — a store-and-forward queue (tenant-scoped `agent_relay` table)
//!   that holds signed envelopes for a temporarily-offline peer and drains them
//!   with bounded-backoff retry. Delivery itself is abstracted behind the
//!   [`relay::PeerTransport`] trait, keeping the wire transport out of here.
//!
//! Networking still lives elsewhere (HTTP push / NATS, wired in `crates/api`);
//! this crate only provides the crypto primitives plus the persistence + retry
//! state machine, and is unit-tested against an in-memory SurrealDB.

pub mod canonical;
pub mod card;
pub mod envelope;
pub mod error;
pub mod identity;
pub mod relay;

pub use card::{AgentCard, AgentKind};
pub use envelope::Envelope;
pub use error::{AgentError, Result};
pub use identity::Identity;
