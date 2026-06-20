//! Write-action framework for the business agent (ADR-0016, Wave 3 — "el
//! agente actúa").
//!
//! The read-only agent answers questions; this module lets it *act* — but
//! under a deliberately tight safety envelope, because letting an agent write
//! to the ERP is the riskiest thing it can do:
//!
//! 1. **Two-step propose → confirm.** A write question never executes on the
//!    spot. The agent returns an [`ActionProposal`] — `{name, summary, params,
//!    confirm_token, expires_at}` — and writes NOTHING. Execution happens only
//!    when the server later receives that exact `confirm_token` back. The
//!    params are frozen server-side at propose time and never travel back from
//!    the client, so they can't be tampered with between the two steps.
//! 2. **Closed whitelist.** Only the variants of [`Action`] can ever run. There
//!    is no arbitrary-write path. v1 ships two: `registrar_gasto` and
//!    `crear_orden_compra_draft`. Each one reuses the existing `domain` write
//!    service — nothing is reimplemented here.
//! 3. **Single-use, expiring, tenant-bound tokens.** A token is consumed
//!    atomically on the first valid confirm (replay → rejected), expires after
//!    [`TOKEN_TTL_SECS`], and is scoped to the tenant that proposed it (a token
//!    minted for tenant A can never execute against tenant B).
//! 4. **Audit.** Every execution writes an `audit_log` row (the existing table,
//!    no new schema) so the owner can always see what the agent did on their
//!    behalf.
//!
//! Role gating (admin/owner only) is enforced at the HTTP edge — see
//! `crates/api/src/v1/assist.rs`. This module is offline-first and
//! deterministic: parsing is keyword-based, execution is a local domain write,
//! no network anywhere (ADR-0005).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use surrealdb::sql::Thing;

use db::Db;
use domain::DomainResult;

use crate::deterministic::clp;
use crate::intent::normalize;

/// How long a `confirm_token` stays valid after it is issued. Short by design:
/// a proposal is a "do you want me to do X?" prompt, not a durable grant.
pub const TOKEN_TTL_SECS: i64 = 180;

// ---- the whitelist -----------------------------------------------------------

/// The CLOSED set of write actions the agent may perform. Adding a capability
/// to the agent's hands means adding a variant here *and* a `match` arm in
/// [`execute`] — there is no generic write path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Record a business expense — reuses `domain::expenses::create_expense`.
    RegistrarGasto {
        category: String,
        description: String,
        amount: Decimal,
        payment_method: String,
    },
    /// Create a *draft* purchase order — reuses
    /// `domain::purchasing::create_purchase_order` (which leaves the PO in
    /// `draft`; issuing/receiving stays a separate, human step).
    CrearOrdenCompraDraft {
        supplier_id: String,
        supplier_name: String,
        product_name: String,
        quantity: i64,
        unit_cost: Decimal,
    },
}

impl Action {
    /// Stable machine label echoed to the client and written to the audit log.
    pub fn label(&self) -> &'static str {
        match self {
            Action::RegistrarGasto { .. } => "registrar_gasto",
            Action::CrearOrdenCompraDraft { .. } => "crear_orden_compra_draft",
        }
    }

    /// One-line es-CL summary the UI shows before the owner confirms.
    pub fn summary(&self) -> String {
        match self {
            Action::RegistrarGasto {
                description,
                amount,
                ..
            } => format!(
                "Registrar un gasto de {} por «{}».",
                clp(*amount),
                description
            ),
            Action::CrearOrdenCompraDraft {
                supplier_name,
                product_name,
                quantity,
                unit_cost,
                ..
            } => format!(
                "Crear una orden de compra (borrador) a {}: {} × {} a {} c/u.",
                supplier_name,
                quantity,
                product_name,
                clp(*unit_cost),
            ),
        }
    }

    /// Structured echo of the parameters, so the UI can render a confirmation
    /// card without re-parsing the summary.
    pub fn params(&self) -> serde_json::Value {
        match self {
            Action::RegistrarGasto {
                category,
                description,
                amount,
                payment_method,
            } => serde_json::json!({
                "category": category,
                "description": description,
                "amount": amount.to_string(),
                "payment_method": payment_method,
            }),
            Action::CrearOrdenCompraDraft {
                supplier_id,
                supplier_name,
                product_name,
                quantity,
                unit_cost,
            } => serde_json::json!({
                "supplier_id": supplier_id,
                "supplier_name": supplier_name,
                "product_name": product_name,
                "quantity": quantity,
                "unit_cost": unit_cost.to_string(),
            }),
        }
    }
}

// ---- the proposal (frozen client contract) -----------------------------------

/// What `POST /assist/ask` returns for a write request. `confirm_token` is the
/// only field the client sends back to `POST /assist/act`; everything else is
/// for display. FROZEN contract — do not reshape without bumping ye's UI.
#[derive(Debug, Clone, Serialize)]
pub struct ActionProposal {
    /// Machine label (see [`Action::label`]).
    pub name: &'static str,
    /// es-CL one-liner for the confirmation prompt.
    pub summary: String,
    /// Structured parameters backing the summary.
    pub params: serde_json::Value,
    /// Opaque, single-use server-issued token. Send it back verbatim to execute.
    pub confirm_token: String,
    /// When the token stops being valid.
    pub expires_at: DateTime<Utc>,
}

/// What `POST /assist/act` returns once a confirmed action runs.
#[derive(Debug, Clone, Serialize)]
pub struct ActionOutcome {
    /// Machine label of the action that ran.
    pub action: &'static str,
    /// es-CL confirmation of what happened.
    pub text: String,
    /// Structured result (created record id, amounts, …).
    pub data: serde_json::Value,
}

// ---- the token store ---------------------------------------------------------

/// Why a `consume` failed. Both map to a single, non-revealing client message
/// (we never tell the caller whether a token was wrong, used, or for another
/// tenant — that would be a guessing oracle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActError {
    /// No such pending token (never issued, already consumed, or wrong tenant).
    NotFound,
    /// The token existed but its TTL elapsed.
    Expired,
}

impl ActError {
    pub fn message(&self) -> &'static str {
        match self {
            ActError::NotFound => "El token de confirmación es inválido o ya fue usado.",
            ActError::Expired => "El token de confirmación expiró. Vuelve a pedir la acción.",
        }
    }
}

struct Pending {
    action: Action,
    tenant: Thing,
    expires_at: DateTime<Utc>,
}

/// In-memory, process-local store of pending (proposed but not yet confirmed)
/// actions. Offline-first: no DB, no network — the two-step handshake is
/// stateful only for the few minutes a token lives. A process restart drops
/// pending proposals, which is the safe failure mode (the owner just re-asks).
#[derive(Default)]
pub struct ActionStore {
    inner: Mutex<HashMap<String, Pending>>,
}

impl ActionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issue a token for `action`, scoped to `tenant`, with the default TTL.
    pub fn propose(&self, action: Action, tenant: &Thing) -> ActionProposal {
        self.propose_with_ttl(action, tenant, TOKEN_TTL_SECS)
    }

    /// Issue a token with an explicit TTL (seconds). A non-positive `ttl_secs`
    /// produces an already-expired token — used by tests to exercise the expiry
    /// path deterministically.
    pub fn propose_with_ttl(
        &self,
        action: Action,
        tenant: &Thing,
        ttl_secs: i64,
    ) -> ActionProposal {
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(ttl_secs);
        let proposal = ActionProposal {
            name: action.label(),
            summary: action.summary(),
            params: action.params(),
            confirm_token: token.clone(),
            expires_at,
        };
        self.inner.lock().unwrap().insert(
            token,
            Pending {
                action,
                tenant: tenant.clone(),
                expires_at,
            },
        );
        proposal
    }

    /// Atomically consume a token. Single-use: a token that maps to a pending
    /// action for `tenant` is *removed* and its action returned. Replays, wrong
    /// tenants, and unknown tokens all fail with [`ActError::NotFound`]; a
    /// known-but-stale token is removed and fails with [`ActError::Expired`].
    pub fn consume(&self, token: &str, tenant: &Thing) -> Result<Action, ActError> {
        let mut g = self.inner.lock().unwrap();
        match g.get(token) {
            None => Err(ActError::NotFound),
            // Wrong tenant: do NOT consume — it belongs to someone else.
            Some(p) if &p.tenant != tenant => Err(ActError::NotFound),
            Some(p) if p.expires_at < Utc::now() => {
                g.remove(token);
                Err(ActError::Expired)
            }
            Some(_) => Ok(g.remove(token).expect("present under lock").action),
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

/// The process-wide store backing the HTTP endpoints.
static STORE: LazyLock<ActionStore> = LazyLock::new(ActionStore::new);

/// Accessor for the global [`ActionStore`] used by the API layer.
pub fn store() -> &'static ActionStore {
    &STORE
}

// ---- parsing (deterministic, es-CL) ------------------------------------------

/// Result of scanning a question for a WRITE request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionParse {
    /// Not a write request — the caller should fall through to the read agent.
    NotAnAction,
    /// A write request was recognised but the text is missing a required field;
    /// the string is an es-CL nudge telling the owner what to add.
    Incomplete(String),
    /// A "registrar gasto" request with all text-derivable fields.
    Gasto {
        category: String,
        description: String,
        amount: Decimal,
        payment_method: String,
    },
    /// A "crear orden de compra" request; `supplier_name` still needs DB
    /// resolution into a record id (see [`build`]).
    Oc {
        supplier_name: String,
        product_name: String,
        quantity: i64,
        unit_cost: Decimal,
    },
}

const GASTO_VERBS: &[&str] = &[
    "registra ",
    "registrar ",
    "anota ",
    "anotar ",
    "agrega ",
    "agregar ",
    "carga ",
    "cargar ",
    "ingresa ",
    "ingresar ",
    "crea ",
    "crear ",
    "nuevo ",
    "nueva ",
    "guarda ",
    "guardar ",
];

/// Parse a question into a write [`ActionParse`]. Purely textual and
/// conservative: anything it cannot confidently turn into a whitelisted action
/// stays [`ActionParse::NotAnAction`] (→ read path) or becomes
/// [`ActionParse::Incomplete`] (→ friendly nudge). It NEVER guesses an action.
pub fn parse_action(question: &str) -> ActionParse {
    let q = normalize(question);
    let has_verb = GASTO_VERBS.iter().any(|v| q.contains(v));

    // Purchase order before expense: "orden de compra" is unambiguous.
    if q.contains("orden de compra") || q.contains(" oc ") || q.starts_with("oc ") {
        if !has_verb {
            return ActionParse::NotAnAction;
        }
        return parse_oc(&q);
    }

    // Expense: needs an imperative verb AND the word "gasto" so we don't steal
    // the read intent ("gastos del mes", "cuánto gasté").
    if has_verb && q.contains("gasto") {
        return parse_gasto(&q);
    }

    ActionParse::NotAnAction
}

fn parse_gasto(q: &str) -> ActionParse {
    let Some(amount) = extract_amount(q) else {
        return ActionParse::Incomplete(
            "¿De cuánto es el gasto? Por ejemplo: «registra un gasto de 5000 en arriendo».".into(),
        );
    };
    let category = capture_category(q).unwrap_or_else(|| "otros".to_string());
    let description = capitalize_first(&category);
    ActionParse::Gasto {
        category,
        description,
        amount,
        payment_method: "cash".into(),
    }
}

fn parse_oc(q: &str) -> ActionParse {
    // Canonical: "<verb> ... orden de compra de <qty> <product> a <supplier> a $<cost>".
    let hint = ActionParse::Incomplete(
        "Para crear una orden de compra dime cantidad, producto, proveedor y costo. \
         Por ejemplo: «crea una orden de compra de 10 paracetamol a Farmaltda a $500»."
            .into(),
    );
    let Some(after) = q.split_once("compra de ").map(|(_, r)| r) else {
        return hint;
    };
    // Quantity: leading integer.
    let qty_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    let Ok(quantity) = qty_str.parse::<i64>() else {
        return hint;
    };
    if quantity <= 0 {
        return hint;
    }
    let rest = after[qty_str.len()..].trim();

    // Unit cost: the $-prefixed amount, introduced by " a $" / " por $" / "$".
    let Some(dollar_pos) = rest.find('$') else {
        return hint;
    };
    let cost_str: String = rest[dollar_pos + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let Some(unit_cost) = parse_money(&cost_str) else {
        return hint;
    };

    // Everything before the cost marker holds "<product> a <supplier>". Trim the
    // cost connector words off the tail.
    let mut head = rest[..dollar_pos].trim();
    for tail_cut in [" a $", " por $", " a", " por"] {
        if let Some(stripped) = head.strip_suffix(tail_cut) {
            head = stripped.trim();
            break;
        }
    }

    // Split product vs supplier on the supplier connector.
    let (product_name, supplier_name) = match split_supplier(head) {
        Ok(pair) => pair,
        Err(incomplete) => return incomplete,
    };
    if product_name.is_empty() || supplier_name.is_empty() {
        return hint;
    }
    ActionParse::Oc {
        supplier_name,
        product_name,
        quantity,
        unit_cost,
    }
}

/// Split "<product> a <supplier>" / "… para <supplier>" / "… proveedor <s>".
/// Returns `(product, supplier)` or the parse hint when no connector is found.
fn split_supplier(head: &str) -> Result<(String, String), ActionParse> {
    const CONN: &[&str] = &[
        " al proveedor ",
        " a proveedor ",
        " proveedor ",
        " para ",
        " a ",
    ];
    for c in CONN {
        if let Some(pos) = head.find(c) {
            let product = head[..pos].trim().to_string();
            let supplier = head[pos + c.len()..].trim().to_string();
            return Ok((product, supplier));
        }
    }
    Err(ActionParse::Incomplete(
        "¿A qué proveedor? Por ejemplo: «… a Farmaltda a $500».".into(),
    ))
}

/// Pull the spending figure out of an expense question. Returns the first
/// run of digits (with `.` thousands separators) found, as whole pesos.
fn extract_amount(q: &str) -> Option<Decimal> {
    let bytes = q.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            return parse_money(&q[start..i]);
        }
        i += 1;
    }
    None
}

/// Parse a CL money token like `5000`, `5.000`, or `1.234.567` into whole
/// pesos. `.` is treated as a thousands separator (CL convention).
fn parse_money(s: &str) -> Option<Decimal> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<Decimal>().ok()
}

/// Capture an expense category from connectors like "en arriendo", "para luz",
/// "por sueldos". Skips connectors whose tail starts with a digit (that's the
/// amount, not a category). Returns up to 3 words.
fn capture_category(q: &str) -> Option<String> {
    const CONN: &[&str] = &[" en ", " para ", " por ", " de ", " categoria ", " rubro "];
    for c in CONN {
        let mut from = 0;
        while let Some(rel) = q[from..].find(c) {
            let pos = from + rel;
            let tail = q[pos + c.len()..].trim_start();
            let first = tail.split_whitespace().next().unwrap_or("");
            if !first.is_empty() && !first.chars().next().unwrap().is_ascii_digit() {
                let words: Vec<&str> = tail.split_whitespace().take(3).collect();
                let joined = words.join(" ");
                let cat = strip_trailing_punct(&joined);
                if !cat.is_empty() {
                    return Some(cat.to_string());
                }
            }
            from = pos + c.len();
        }
    }
    None
}

fn strip_trailing_punct(s: &str) -> &str {
    s.trim_matches(|c: char| c == '?' || c == '!' || c == '.' || c == ',' || c.is_whitespace())
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ---- build (resolve to a ready Action) ---------------------------------------

/// Outcome of turning a parsed request into a ready-to-store [`Action`].
pub enum BuildOutcome {
    /// Not a write request — read path.
    NotAnAction,
    /// Could not proceed; the string is a friendly es-CL explanation (missing
    /// field, unknown supplier, …). No token is issued.
    Reject(String),
    /// Fully resolved and validated — ready to propose.
    Ready(Action),
}

/// Resolve a [`ActionParse`] into a [`BuildOutcome`]. The only DB work here is
/// resolving an OC supplier *name* into a record id (a read); expense building
/// is pure (validation is deferred to the domain service at execute time).
pub async fn build(db: &Db, tenant: &Thing, parsed: ActionParse) -> DomainResult<BuildOutcome> {
    match parsed {
        ActionParse::NotAnAction => Ok(BuildOutcome::NotAnAction),
        ActionParse::Incomplete(hint) => Ok(BuildOutcome::Reject(hint)),
        ActionParse::Gasto {
            category,
            description,
            amount,
            payment_method,
        } => Ok(BuildOutcome::Ready(Action::RegistrarGasto {
            category,
            description,
            amount,
            payment_method,
        })),
        ActionParse::Oc {
            supplier_name,
            product_name,
            quantity,
            unit_cost,
        } => {
            use domain::purchasing::model::SupplierFilters;
            use domain::purchasing::service as purchasing;
            let suppliers = purchasing::list_suppliers(
                db,
                tenant,
                SupplierFilters {
                    search: Some(supplier_name.clone()),
                    active: Some(true),
                    limit: Some(5),
                    offset: None,
                },
            )
            .await?;
            let Some(sup) = pick_supplier(&suppliers, &supplier_name) else {
                return Ok(BuildOutcome::Reject(format!(
                    "No encontré al proveedor «{supplier_name}». Créalo primero en Compras y \
                     vuelve a pedírmelo."
                )));
            };
            Ok(BuildOutcome::Ready(Action::CrearOrdenCompraDraft {
                supplier_id: sup.id.clone(),
                supplier_name: sup.name.clone(),
                product_name,
                quantity,
                unit_cost,
            }))
        }
    }
}

/// Prefer a case-insensitive exact name match, else the first hit.
fn pick_supplier<'a>(
    suppliers: &'a [domain::purchasing::model::SupplierDto],
    name: &str,
) -> Option<&'a domain::purchasing::model::SupplierDto> {
    let want = name.to_lowercase();
    suppliers
        .iter()
        .find(|s| s.name.to_lowercase() == want)
        .or_else(|| suppliers.first())
}

// ---- execute -----------------------------------------------------------------

/// Run a confirmed [`Action`] by delegating to the existing domain write
/// service, then append an `audit_log` row. Tenant-scoped; `actor` is the
/// authenticated user (for `created_by` + audit attribution).
pub async fn execute(
    db: &Db,
    tenant: &Thing,
    actor: Option<&Thing>,
    action: Action,
) -> DomainResult<ActionOutcome> {
    let label = action.label();
    let outcome = match action {
        Action::RegistrarGasto {
            category,
            description,
            amount,
            payment_method,
        } => {
            use domain::expenses::model::NewExpense;
            use domain::expenses::service as expenses;
            let dto = expenses::create_expense(
                db,
                tenant,
                actor,
                NewExpense {
                    category,
                    description,
                    amount,
                    payment_method,
                    cash_session: None,
                    supplier: None,
                    note: Some("Registrado por el agente".into()),
                    incurred_at: None,
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!(
                    "Gasto registrado: «{}» por {}.",
                    dto.description,
                    clp(dto.amount)
                ),
                data: serde_json::json!({
                    "id": dto.id,
                    "category": dto.category,
                    "amount": dto.amount.to_string(),
                }),
            }
        }
        Action::CrearOrdenCompraDraft {
            supplier_id,
            supplier_name,
            product_name,
            quantity,
            unit_cost,
        } => {
            use domain::purchasing::model::{NewPurchaseOrder, NewPurchaseOrderItem};
            use domain::purchasing::service as purchasing;
            let dto = purchasing::create_purchase_order(
                db,
                tenant,
                NewPurchaseOrder {
                    supplier: supplier_id,
                    currency: None,
                    notes: Some("Borrador creado por el agente".into()),
                    external_ref: None,
                    items: vec![NewPurchaseOrderItem {
                        product: None,
                        product_name: product_name.clone(),
                        quantity,
                        unit_cost,
                    }],
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!(
                    "Orden de compra borrador creada para {}: {} × {} ({}).",
                    supplier_name,
                    quantity,
                    product_name,
                    clp(dto.total),
                ),
                data: serde_json::json!({
                    "id": dto.id,
                    "status": dto.status,
                    "supplier": supplier_name,
                    "total": dto.total.to_string(),
                }),
            }
        }
    };

    write_audit(db, tenant, actor, label).await?;
    Ok(outcome)
}

/// Append an append-only `audit_log` row for an executed agent action. Reuses
/// the existing table (migration `0002_audit_log.surql`) — no new schema. The
/// generic HTTP audit middleware also records the `/assist/act` POST; this row
/// adds the *which action* granularity, queryable via the audit endpoint.
async fn write_audit(
    db: &Db,
    tenant: &Thing,
    actor: Option<&Thing>,
    label: &str,
) -> DomainResult<()> {
    db.query(
        "CREATE audit_log SET tenant = $tenant, user = $user, method = 'ACTION', \
         path = $path, status = 200",
    )
    .bind(("tenant", tenant.clone()))
    .bind(("user", actor.cloned()))
    .bind(("path", format!("assist/act/{label}")))
    .await?
    .check()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn tenant() -> Thing {
        Thing::from(("tenant", "a"))
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn parse_gasto_extracts_amount_and_category() {
        match parse_action("registra un gasto de 5000 en arriendo") {
            ActionParse::Gasto {
                amount,
                category,
                payment_method,
                ..
            } => {
                assert_eq!(amount, dec("5000"));
                assert_eq!(category, "arriendo");
                assert_eq!(payment_method, "cash");
            }
            other => panic!("expected Gasto, got {other:?}"),
        }
    }

    #[test]
    fn parse_gasto_thousands_separator() {
        match parse_action("anota un gasto de $1.250.000 en sueldos") {
            ActionParse::Gasto {
                amount, category, ..
            } => {
                assert_eq!(amount, dec("1250000"));
                assert_eq!(category, "sueldos");
            }
            other => panic!("expected Gasto, got {other:?}"),
        }
    }

    #[test]
    fn parse_gasto_without_amount_is_incomplete() {
        assert!(matches!(
            parse_action("registra un gasto en arriendo"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn read_expense_question_is_not_an_action() {
        // These belong to the read agent (GastosMes), not the write path.
        assert_eq!(parse_action("gastos del mes"), ActionParse::NotAnAction);
        assert_eq!(
            parse_action("cuánto gasté este mes"),
            ActionParse::NotAnAction
        );
    }

    #[test]
    fn parse_oc_canonical() {
        match parse_action("crea una orden de compra de 10 paracetamol a Farmaltda a $500") {
            ActionParse::Oc {
                supplier_name,
                product_name,
                quantity,
                unit_cost,
            } => {
                assert_eq!(quantity, 10);
                assert_eq!(product_name, "paracetamol");
                assert_eq!(supplier_name, "farmaltda");
                assert_eq!(unit_cost, dec("500"));
            }
            other => panic!("expected Oc, got {other:?}"),
        }
    }

    #[test]
    fn parse_oc_missing_cost_is_incomplete() {
        assert!(matches!(
            parse_action("crea una orden de compra de 10 paracetamol a Farmaltda"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn non_write_question_is_not_an_action() {
        assert_eq!(parse_action("cuánto vendí hoy"), ActionParse::NotAnAction);
        assert_eq!(
            parse_action("stock de paracetamol"),
            ActionParse::NotAnAction
        );
    }

    #[test]
    fn token_is_single_use() {
        let store = ActionStore::new();
        let action = Action::RegistrarGasto {
            category: "luz".into(),
            description: "Luz".into(),
            amount: dec("1000"),
            payment_method: "cash".into(),
        };
        let p = store.propose(action.clone(), &tenant());
        assert_eq!(store.len(), 1);
        assert_eq!(store.consume(&p.confirm_token, &tenant()), Ok(action));
        assert_eq!(store.len(), 0, "token removed on consume");
        // Replay rejected.
        assert_eq!(
            store.consume(&p.confirm_token, &tenant()),
            Err(ActError::NotFound)
        );
    }

    #[test]
    fn token_is_tenant_bound() {
        let store = ActionStore::new();
        let action = Action::RegistrarGasto {
            category: "luz".into(),
            description: "Luz".into(),
            amount: dec("1000"),
            payment_method: "cash".into(),
        };
        let p = store.propose(action, &tenant());
        let other = Thing::from(("tenant", "b"));
        // Wrong tenant cannot consume, and does NOT burn the token.
        assert_eq!(
            store.consume(&p.confirm_token, &other),
            Err(ActError::NotFound)
        );
        assert_eq!(store.len(), 1, "other tenant must not consume the token");
        // Rightful tenant still can.
        assert!(store.consume(&p.confirm_token, &tenant()).is_ok());
    }

    #[test]
    fn expired_token_is_rejected() {
        let store = ActionStore::new();
        let action = Action::RegistrarGasto {
            category: "luz".into(),
            description: "Luz".into(),
            amount: dec("1000"),
            payment_method: "cash".into(),
        };
        let p = store.propose_with_ttl(action, &tenant(), -1);
        assert_eq!(
            store.consume(&p.confirm_token, &tenant()),
            Err(ActError::Expired)
        );
        assert_eq!(store.len(), 0, "expired token is purged on consume");
    }

    #[test]
    fn unknown_token_is_not_found() {
        let store = ActionStore::new();
        assert_eq!(store.consume("nope", &tenant()), Err(ActError::NotFound));
    }
}
