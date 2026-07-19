//! OPT-IN LLM provider (ADR-0016 "Camino LLM opt-in" + ADR-0017 BYO-AI).
//!
//! The seam in [`crate::provider`] was always built to accept an LLM provider
//! later, with the owner's own key, default OFF. This is that provider.
//!
//! ## Grounding, not hallucination
//!
//! The LLM never invents tenant figures. For every question we FIRST run the
//! offline [`crate::Deterministic`] provider over the owner's real data, then
//! hand the question + that grounded answer to the model and ask it only to
//! *rephrase / enrich* in warm es-CL. So the numbers always come from a real
//! `domain` read; the model improves the prose and can field fuzzier phrasings
//! the keyword parser classified as `Unknown`.
//!
//! ## Offline-first is preserved (ADR-0005 inv. 1, 2, 6)
//!
//! On ANY failure — no key, no network, no credit, timeout, bad response — the
//! provider returns the deterministic answer verbatim. The agent therefore
//! NEVER errors in the owner's face just because the LLM is unreachable, and a
//! node with the toggle off makes zero network calls. Sending the grounded
//! context to the model is the explicit, opt-in action the owner enabled
//! (telemetry-style consent, ADR-0005 inv. 3).
//!
//! ## Provider-agnostic
//!
//! The HTTP call sits behind the [`ChatBackend`] trait. Today the only impl is
//! [`AnthropicBackend`] (Claude Messages API, raw HTTP — there is no official
//! Rust SDK). A second backend (OpenAI, a local Ollama, a managed proxy) plugs
//! in without touching [`LlmProvider`]. Tests inject a mock backend so the
//! suite makes no network calls.

use async_trait::async_trait;

use domain::DomainResult;

use crate::deterministic::Deterministic;
use crate::provider::{Answer, AssistProvider, AssistQuery};

/// Model id for the Claude backend. Anthropic's most capable model
/// (`shared/models.md`); the owner pays for their own tokens (BYO-key).
pub const DEFAULT_MODEL: &str = "claude-opus-4-8";
/// Anthropic Messages API endpoint.
pub const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
/// Hard timeout so a slow model never stalls the agent — we fall back to the
/// (already-computed) deterministic answer instead.
const TIMEOUT_SECS: u64 = 30;
/// Cap on model output; the rephrase is a short paragraph, not an essay.
const MAX_TOKENS: u32 = 700;

/// A minimal chat completion seam: one Spanish-system + user prompt in, one
/// text answer out. Keeps the LLM provider testable (mock impl) and
/// provider-agnostic (swap Anthropic for any other backend).
#[async_trait]
pub trait ChatBackend: Send + Sync {
    /// Run one completion. `Err` on any transport/credit/parse failure — the
    /// caller degrades to the deterministic answer.
    async fn complete(&self, system: &str, user: &str) -> Result<String, String>;
}

/// The opt-in LLM provider. Generic over the backend so tests don't touch the
/// network. Holds an inner [`Deterministic`] for grounding + fallback.
pub struct LlmProvider<B: ChatBackend> {
    backend: B,
    inner: Deterministic,
}

impl<B: ChatBackend> LlmProvider<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            inner: Deterministic,
        }
    }
}

/// es-CL system prompt. Pins the model to *rephrasing grounded data* and bans
/// inventing figures — the offline answer is the single source of truth.
const SYSTEM_PROMPT: &str = "Eres el agente de RutBusiness, el asistente del dueño de un \
    negocio chileno. Hablas español de Chile, cercano y claro, como hablaría el dueño. \
    Recibes la pregunta del dueño y una RESPUESTA BASE ya calculada con los datos reales \
    de su negocio. Tu trabajo es reformular esa respuesta base de forma natural y útil. \
    REGLAS ESTRICTAS: (1) NUNCA inventes cifras, fechas, nombres ni datos que no estén en \
    la respuesta base — si la base no tiene un dato, no lo afirmes. (2) Mantén exactos \
    todos los montos en pesos, porcentajes y cantidades de la base. (3) Sé breve: un \
    párrafo corto. (4) No agregues despedidas ni ofertas de ayuda genéricas. (5) Si la \
    respuesta base dice que no entendió o que faltan datos, ayúdalo a reformular su \
    pregunta con un ejemplo concreto.";

/// Build the user-turn prompt: the original question + the grounded answer.
fn build_user_prompt(question: &str, grounded: &Answer) -> String {
    let mut p = format!(
        "Pregunta del dueño:\n{question}\n\nRespuesta base (datos reales del negocio):\n{}",
        grounded.text
    );
    if let Some(data) = &grounded.data {
        // The structured figures back the prose; give them to the model too so
        // it can surface a detail the prose omitted — still grounded data.
        if let Ok(j) = serde_json::to_string(data) {
            p.push_str("\n\nDatos estructurados (JSON):\n");
            p.push_str(&j);
        }
    }
    p
}

#[async_trait]
impl<B: ChatBackend> AssistProvider for LlmProvider<B> {
    async fn answer(&self, q: &AssistQuery<'_>) -> DomainResult<Answer> {
        // 1. Always ground first on the owner's real data (offline).
        let grounded = self.inner.answer(q).await?;

        // 2. Ask the model to rephrase. On ANY failure, return the grounded
        //    answer unchanged — the agent never errors because the LLM is down.
        let user = build_user_prompt(q.question, &grounded);
        match self.backend.complete(SYSTEM_PROMPT, &user).await {
            Ok(text) => {
                let text = text.trim();
                if text.is_empty() {
                    // Empty completion → keep the grounded answer.
                    return Ok(grounded);
                }
                // Preserve intent label + structured `data` (and any `action`);
                // only the prose is upgraded.
                Ok(Answer {
                    text: text.to_string(),
                    ..grounded
                })
            }
            Err(e) => {
                tracing::warn!(error = %e, "assist: llm backend failed; serving deterministic answer");
                Ok(grounded)
            }
        }
    }
}

/// Real backend: Claude Messages API over raw HTTP (no official Rust SDK).
/// Holds the owner's key + the model id. Never logs the key.
pub struct AnthropicBackend {
    api_key: String,
    model: String,
    url: String,
    client: reqwest::Client,
}

impl AnthropicBackend {
    /// Build with the owner's key. `model`/`url` default to Claude Opus 4.8 on
    /// the public endpoint; overridable for a proxy or a pinned model.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: DEFAULT_MODEL.to_string(),
            url: ANTHROPIC_URL.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
                .build()
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_url(mut self, url: String) -> Self {
        self.url = url;
        self
    }
}

#[async_trait]
impl ChatBackend for AnthropicBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        // Anthropic Messages API request. `system` is a top-level field; the
        // question + grounded data go in a single user turn.
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": system,
            "messages": [{ "role": "user", "content": user }],
        });

        let resp = self
            .client
            .post(&self.url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("anthropic request failed: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("anthropic body read failed: {e}"))?;

        if !status.is_success() {
            // Surface the API error type (e.g. credit too low) without the key.
            return Err(format!(
                "anthropic http {status}: {}",
                first_chars(&text, 300)
            ));
        }

        // Response shape: { "content": [ { "type": "text", "text": "..." }, ... ] }
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("anthropic json parse: {e}"))?;
        let out = v
            .get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        if out.is_empty() {
            return Err("anthropic response had no text content".to_string());
        }
        Ok(out)
    }
}

fn first_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::Intent;
    use crate::provider::AssistQuery;
    use std::sync::Arc;
    use surrealdb::engine::local::Db as LocalDb;
    use surrealdb::Surreal;

    /// A backend that returns a canned string or a canned error — no network.
    struct MockBackend {
        result: Result<String, String>,
        called: Arc<std::sync::atomic::AtomicBool>,
    }
    #[async_trait]
    impl ChatBackend for MockBackend {
        async fn complete(&self, _system: &str, _user: &str) -> Result<String, String> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            self.result.clone()
        }
    }

    async fn memdb() -> (Arc<db::Db>, Thing) {
        let handle: Surreal<LocalDb> = Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("mem db");
        handle.use_ns("t").use_db("t").await.unwrap();
        // Run migrations so the deterministic reads have schema.
        db::run_migrations(&handle, "../../migrations")
            .await
            .expect("migrations");
        let mut r = handle
            .query("CREATE tenant SET name='T', slug='t' RETURN AFTER")
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct Row {
            id: Thing,
        }
        let row: Option<Row> = r.take(0).unwrap();
        (Arc::new(handle), row.unwrap().id)
    }

    use surrealdb::sql::Thing;

    fn query<'a>(db: &'a db::Db, tenant: &'a Thing) -> AssistQuery<'a> {
        AssistQuery {
            question: "¿cuánto vendí hoy?",
            intent: Intent::VentasHoy,
            db,
            tenant,
        }
    }

    #[tokio::test]
    async fn rephrases_on_success() {
        let (db, tenant) = memdb().await;
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let provider = LlmProvider::new(MockBackend {
            result: Ok("Hoy llevas $0 en ventas, aún sin movimientos.".into()),
            called: called.clone(),
        });
        let ans = provider.answer(&query(&db, &tenant)).await.unwrap();
        assert!(
            called.load(std::sync::atomic::Ordering::SeqCst),
            "backend called"
        );
        assert_eq!(ans.text, "Hoy llevas $0 en ventas, aún sin movimientos.");
        // Intent label preserved from the grounded answer.
        assert_eq!(ans.intent, Intent::VentasHoy.label());
    }

    #[tokio::test]
    async fn falls_back_to_deterministic_on_error() {
        let (db, tenant) = memdb().await;
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let provider = LlmProvider::new(MockBackend {
            result: Err("anthropic http 400: credit too low".into()),
            called: called.clone(),
        });
        let ans = provider.answer(&query(&db, &tenant)).await.unwrap();
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
        // On failure the deterministic prose stands — non-empty, grounded.
        assert!(!ans.text.is_empty());
        assert_eq!(ans.intent, Intent::VentasHoy.label());
    }

    #[tokio::test]
    async fn empty_completion_keeps_deterministic() {
        let (db, tenant) = memdb().await;
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let det = Deterministic
            .answer(&query(&db, &tenant))
            .await
            .unwrap()
            .text;
        let provider = LlmProvider::new(MockBackend {
            result: Ok("   ".into()),
            called,
        });
        let ans = provider.answer(&query(&db, &tenant)).await.unwrap();
        assert_eq!(ans.text, det, "blank completion → deterministic prose");
    }

    #[test]
    fn user_prompt_includes_question_and_grounding() {
        let grounded = Answer::new(&Intent::VentasHoy, "Hoy: $1.000")
            .with_data(serde_json::json!({"total": "1000"}));
        let p = build_user_prompt("¿cuánto vendí?", &grounded);
        assert!(p.contains("¿cuánto vendí?"));
        assert!(p.contains("Hoy: $1.000"));
        assert!(p.contains("total"));
    }
}
