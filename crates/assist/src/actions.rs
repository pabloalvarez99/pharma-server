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
    /// Register a new customer — reuses `domain::customers::create_customer`
    /// (which validates the name and app-level RUT uniqueness per tenant).
    CrearCliente {
        name: String,
        rut: Option<String>,
        phone: Option<String>,
        email: Option<String>,
    },
    /// Quick-create a product with just a name and price — reuses
    /// `domain::catalog::create_product` (slug auto-generated, no category, no
    /// cost). The owner refines the rest in the catalog screen later.
    CrearProductoRapido {
        name: String,
        price: Decimal,
        stock: i64,
    },
    /// Reprice a single existing product — reuses
    /// `domain::catalog::update_product`. `product_id` + `old_price` are
    /// resolved server-side at propose time (see [`build`]); `old_price` is for
    /// the confirmation prose only.
    AjustarPrecio {
        product_id: String,
        product_name: String,
        old_price: Decimal,
        new_price: Decimal,
    },
    /// Close the open cash drawer with the counted cash — reuses
    /// `domain::cash_register::close_session`. The open session + `expected`
    /// cash are resolved server-side at propose time (see [`build`]); `expected`
    /// is for the confirmation prose only.
    CerrarCaja {
        session_id: String,
        register_name: String,
        expected: Decimal,
        counted: Decimal,
    },
}

impl Action {
    /// Stable machine label echoed to the client and written to the audit log.
    pub fn label(&self) -> &'static str {
        match self {
            Action::RegistrarGasto { .. } => "registrar_gasto",
            Action::CrearOrdenCompraDraft { .. } => "crear_orden_compra_draft",
            Action::CrearCliente { .. } => "crear_cliente",
            Action::CrearProductoRapido { .. } => "crear_producto_rapido",
            Action::AjustarPrecio { .. } => "ajustar_precio",
            Action::CerrarCaja { .. } => "cerrar_caja",
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
            Action::CrearCliente { name, rut, .. } => match rut {
                Some(r) => format!("Registrar al cliente «{name}» (RUT {r})."),
                None => format!("Registrar al cliente «{name}»."),
            },
            Action::CrearProductoRapido { name, price, stock } => format!(
                "Crear el producto «{}» a {} ({} {} de stock inicial).",
                name,
                clp(*price),
                stock,
                if *stock == 1 { "unidad" } else { "unidades" },
            ),
            Action::AjustarPrecio {
                product_name,
                old_price,
                new_price,
                ..
            } => format!(
                "Cambiar el precio de {} de {} a {}.",
                product_name,
                clp(*old_price),
                clp(*new_price),
            ),
            Action::CerrarCaja {
                register_name,
                expected,
                counted,
                ..
            } => format!(
                "Cerrar la caja «{}» con {} contados (esperado {}, {}).",
                register_name,
                clp(*counted),
                clp(*expected),
                diff_text(*counted - *expected),
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
            Action::CrearCliente {
                name,
                rut,
                phone,
                email,
            } => serde_json::json!({
                "name": name,
                "rut": rut,
                "phone": phone,
                "email": email,
            }),
            Action::CrearProductoRapido { name, price, stock } => serde_json::json!({
                "name": name,
                "price": price.to_string(),
                "stock": stock,
            }),
            Action::AjustarPrecio {
                product_id,
                product_name,
                old_price,
                new_price,
            } => serde_json::json!({
                "product_id": product_id,
                "product_name": product_name,
                "old_price": old_price.to_string(),
                "new_price": new_price.to_string(),
            }),
            Action::CerrarCaja {
                session_id,
                register_name,
                expected,
                counted,
            } => serde_json::json!({
                "session_id": session_id,
                "register_name": register_name,
                "expected": expected.to_string(),
                "counted": counted.to_string(),
            }),
        }
    }
}

/// es-CL phrasing of a drawer difference (`counted − expected`): exact, surplus,
/// or short. Shared by the proposal summary and the executed outcome.
fn diff_text(diff: Decimal) -> String {
    if diff.is_zero() {
        "calza exacto".to_string()
    } else if diff.is_sign_positive() {
        format!("sobran {}", clp(diff))
    } else {
        format!("faltan {}", clp(diff.abs()))
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
    /// A "crear cliente" request with the text-derivable fields.
    Cliente {
        name: String,
        rut: Option<String>,
        phone: Option<String>,
        email: Option<String>,
    },
    /// A "crear producto" request (name + price, optional initial stock).
    Producto {
        name: String,
        price: Decimal,
        stock: i64,
    },
    /// An "ajustar precio" request; `product_name` still needs DB resolution
    /// into a record id + current price (see [`build`]).
    AjustePrecio {
        product_name: String,
        new_price: Decimal,
    },
    /// A "cerrar caja" request: the operator counted `counted` pesos. The open
    /// session + expected cash are resolved server-side (see [`build`]).
    CierreCaja { counted: Decimal },
}

/// Imperative verbs that introduce a CREATE request (gasto, cliente, producto,
/// orden de compra).
const CREATE_VERBS: &[&str] = &[
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

/// Imperative verbs that introduce a price CHANGE on an existing product.
/// Kept distinct from [`CREATE_VERBS`] so "crea un producto X precio 1000" is a
/// create (price is an attribute) while "cambia el precio de X a 1000" is an
/// adjust.
const PRICE_VERBS: &[&str] = &[
    "cambia ",
    "cambiar ",
    "ajusta ",
    "ajustar ",
    "actualiza ",
    "actualizar ",
    "modifica ",
    "modificar ",
    "sube ",
    "subir ",
    "baja ",
    "bajar ",
    "pon ",
    "poner ",
    "fija ",
    "fijar ",
    "deja ",
    "dejar ",
];

/// Parse a question into a write [`ActionParse`]. Purely textual and
/// conservative: anything it cannot confidently turn into a whitelisted action
/// stays [`ActionParse::NotAnAction`] (→ read path) or becomes
/// [`ActionParse::Incomplete`] (→ friendly nudge). It NEVER guesses an action.
pub fn parse_action(question: &str) -> ActionParse {
    let q = normalize(question);
    let has_create = CREATE_VERBS.iter().any(|v| q.contains(v));
    let has_price_verb = PRICE_VERBS.iter().any(|v| q.contains(v));

    // Purchase order before everything else: "orden de compra" is unambiguous.
    if q.contains("orden de compra") || q.contains(" oc ") || q.starts_with("oc ") {
        if !has_create {
            return ActionParse::NotAnAction;
        }
        return parse_oc(&q);
    }

    // Cash-drawer close: "cierra la caja con $50.000 contados". A distinct
    // imperative (none of the CREATE/PRICE verbs) so it never collides with the
    // read intent CajaActual ("cuánto hay en caja"), which carries no close verb.
    if (q.contains("caja") || q.contains("arqueo"))
        && ["cierra", "cerrar", "cuadra", "cuadrar", "arquea", "arquear"]
            .iter()
            .any(|v| q.contains(v))
    {
        return parse_cierre(&q);
    }

    // Price adjustment BEFORE the create branches: a price verb + "precio" is a
    // reprice of an existing product, even when the text also says "producto".
    if has_price_verb && q.contains("precio") {
        return parse_ajuste(&q);
    }

    // Expense: needs a create verb AND the word "gasto" so we don't steal the
    // read intent ("gastos del mes", "cuánto gasté").
    if has_create && q.contains("gasto") {
        return parse_gasto(&q);
    }

    // New customer: create verb + "cliente". Read intents ("mejores clientes",
    // "cuántos clientes tengo") carry no create verb → fall through to the
    // read agent.
    if has_create && q.contains("cliente") {
        return parse_cliente(&q);
    }

    // Quick product: create verb + "producto".
    if has_create && q.contains("producto") {
        return parse_producto(&q);
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

/// Fields a "crear cliente" text may carry after the customer name.
const CLIENTE_FIELD_MARKERS: &[&str] = &[
    " rut ",
    " telefono ",
    " fono ",
    " celular ",
    " cel ",
    " email ",
    " correo ",
    " mail ",
    " con ",
];

fn parse_cliente(q: &str) -> ActionParse {
    let hint = ActionParse::Incomplete(
        "Para crear un cliente dime su nombre. Por ejemplo: «crea un cliente Juan Pérez \
         rut 12.345.678-9»."
            .into(),
    );
    let Some((_, after)) = q.split_once("cliente") else {
        return hint;
    };
    let mut rest = after.trim();
    // Strip filler that can sit between "cliente" and the actual name.
    for lead in [
        "llamado ",
        "llamada ",
        "de nombre ",
        "nombre ",
        "nuevo ",
        "nueva ",
        ": ",
    ] {
        if let Some(s) = rest.strip_prefix(lead) {
            rest = s.trim();
        }
    }
    // Pad with a leading space so a marker like " rut " is also found when the
    // field word sits at the very start ("cliente rut 11..." → no name).
    let scan = format!(" {rest}");
    // The name runs until the first field marker (rut/phone/email/…).
    let mut name_end = scan.len();
    for m in CLIENTE_FIELD_MARKERS {
        if let Some(p) = scan.find(m) {
            name_end = name_end.min(p);
        }
    }
    let name = titlecase(strip_trailing_punct(scan[..name_end].trim()).trim());
    if name.is_empty() {
        return hint;
    }
    ActionParse::Cliente {
        name,
        rut: token_after(&scan, &[" rut "]).map(|s| s.to_uppercase()),
        phone: token_after(&scan, &[" telefono ", " fono ", " celular ", " cel "]),
        email: token_after(&scan, &[" email ", " correo ", " mail ", " e-mail "]),
    }
}

fn parse_producto(q: &str) -> ActionParse {
    let hint = ActionParse::Incomplete(
        "Para crear un producto dime nombre y precio. Por ejemplo: «crea un producto \
         Aspirina a $1000»."
            .into(),
    );
    let Some((_, after)) = q.split_once("producto") else {
        return hint;
    };
    let mut rest = after.trim();
    for lead in ["llamado ", "llamada ", "nuevo ", "nueva ", ": "] {
        if let Some(s) = rest.strip_prefix(lead) {
            rest = s.trim();
        }
    }
    // Optional initial stock ("... stock 20").
    let stock = token_after(rest, &[" stock ", " unidades "])
        .and_then(|t| t.parse::<i64>().ok())
        .filter(|n| *n >= 0)
        .unwrap_or(0);
    // Price: prefer a `$`-prefixed figure, else the one after "precio".
    let (name_raw, price) = if let Some(p) = rest.find('$') {
        (&rest[..p], parse_money(&digits_after(&rest[p + 1..])))
    } else if let Some(p) = rest.find(" precio ") {
        let tail = rest[p + " precio ".len()..].trim_start();
        (&rest[..p], parse_money(&digits_after(tail)))
    } else {
        return hint;
    };
    let Some(price) = price else {
        return hint;
    };
    let name = titlecase(&trim_price_connectors(name_raw));
    if name.is_empty() {
        return hint;
    }
    ActionParse::Producto { name, price, stock }
}

fn parse_cierre(q: &str) -> ActionParse {
    match extract_amount(q) {
        Some(counted) => ActionParse::CierreCaja { counted },
        None => ActionParse::Incomplete(
            "Para cerrar la caja dime cuánto contaste. Por ejemplo: «cierra la caja con \
             $50.000»."
                .into(),
        ),
    }
}

fn parse_ajuste(q: &str) -> ActionParse {
    let hint = ActionParse::Incomplete(
        "Para cambiar un precio dime el producto y el nuevo precio. Por ejemplo: \
         «cambia el precio de paracetamol a $1500»."
            .into(),
    );
    let after = q
        .split_once("precio de ")
        .or_else(|| q.split_once("precio del "))
        .map(|(_, r)| r);
    let Some(rest) = after else {
        return hint;
    };
    let mut rest = rest.trim();
    // "precio del producto X" → drop the redundant "producto" lead.
    for lead in ["producto ", "el producto ", "del producto "] {
        if let Some(s) = rest.strip_prefix(lead) {
            rest = s.trim();
        }
    }
    let (name_raw, price) = if let Some(p) = rest.find('$') {
        (&rest[..p], parse_money(&digits_after(&rest[p + 1..])))
    } else if let Some(p) = rest.rfind(" a ") {
        (
            &rest[..p],
            parse_money(&digits_after(rest[p + 3..].trim_start())),
        )
    } else {
        return hint;
    };
    let Some(new_price) = price else {
        return hint;
    };
    let product_name = trim_price_connectors(name_raw);
    if product_name.is_empty() {
        return hint;
    }
    ActionParse::AjustePrecio {
        product_name,
        new_price,
    }
}

/// Leading run of money digits (`0-9` and CL thousands `.`) of `s`.
fn digits_after(s: &str) -> String {
    s.trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

/// Trim price-introducing connectors off the tail of a captured product name.
fn trim_price_connectors(name_raw: &str) -> String {
    let mut head = name_raw.trim();
    for cut in [
        " a $", " a", " por $", " por", " precio", " en", " vale", " cuesta",
    ] {
        if let Some(stripped) = head.strip_suffix(cut) {
            head = stripped.trim();
            break;
        }
    }
    strip_trailing_punct(head).trim().to_string()
}

/// First whitespace-delimited token following any of `cues`, punctuation
/// stripped. Used to pull optional RUT/phone/email out of a "crear cliente"
/// text. Returns `None` when no cue matches or the token is empty.
fn token_after(hay: &str, cues: &[&str]) -> Option<String> {
    for c in cues {
        if let Some(p) = hay.find(c) {
            let tail = hay[p + c.len()..].trim_start();
            let tok: String = tail.chars().take_while(|ch| !ch.is_whitespace()).collect();
            let t = strip_trailing_punct(&tok);
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
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

/// Title-case a parsed proper noun for display. `normalize` lower-cases the
/// whole question for keyword matching, so a captured customer/product name
/// arrives all-lowercase; this restores a presentable `Juan Perez` / `Coca
/// Cola`. (Accents folded by `normalize` are not restored — a minor cosmetic
/// loss the owner can fix in the detail screen.)
fn titlecase(s: &str) -> String {
    s.split_whitespace()
        .map(capitalize_first)
        .collect::<Vec<_>>()
        .join(" ")
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
        ActionParse::Cliente {
            name,
            rut,
            phone,
            email,
        } => Ok(BuildOutcome::Ready(Action::CrearCliente {
            name,
            rut,
            phone,
            email,
        })),
        ActionParse::Producto { name, price, stock } => {
            Ok(BuildOutcome::Ready(Action::CrearProductoRapido {
                name,
                price,
                stock,
            }))
        }
        ActionParse::AjustePrecio {
            product_name,
            new_price,
        } => {
            use domain::catalog::model::ProductFilters;
            use domain::catalog::service as catalog;
            let products = catalog::list_products(
                db,
                tenant,
                ProductFilters {
                    search: Some(product_name.clone()),
                    limit: Some(5),
                    ..Default::default()
                },
            )
            .await?;
            let Some(p) = pick_product(&products, &product_name) else {
                return Ok(BuildOutcome::Reject(format!(
                    "No encontré ningún producto que coincida con «{product_name}». \
                     Revisa el nombre o créalo primero."
                )));
            };
            Ok(BuildOutcome::Ready(Action::AjustarPrecio {
                product_id: p.id.clone(),
                product_name: p.name.clone(),
                old_price: p.price,
                new_price,
            }))
        }
        ActionParse::CierreCaja { counted } => {
            use domain::cash_register::model::SessionFilters;
            use domain::cash_register::service as caja;
            let sessions = caja::list_sessions(
                db,
                tenant,
                SessionFilters {
                    status: Some("open".into()),
                    user: None,
                    limit: Some(1),
                    offset: None,
                },
            )
            .await?;
            let Some(session) = sessions.into_iter().next() else {
                return Ok(BuildOutcome::Reject(
                    "No hay ninguna caja abierta para cerrar.".into(),
                ));
            };
            let (s, _cash, _in, _out, expected) =
                caja::compute_summary(db, tenant, &session.id).await?;
            Ok(BuildOutcome::Ready(Action::CerrarCaja {
                session_id: s.id,
                register_name: s.register_name,
                expected,
                counted,
            }))
        }
    }
}

/// Prefer a case-insensitive exact name match, else the first hit.
fn pick_product<'a>(
    products: &'a [domain::catalog::model::ProductDto],
    name: &str,
) -> Option<&'a domain::catalog::model::ProductDto> {
    let want = name.to_lowercase();
    products
        .iter()
        .find(|p| p.name.to_lowercase() == want)
        .or_else(|| products.first())
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
        Action::CrearCliente {
            name,
            rut,
            phone,
            email,
        } => {
            use domain::customers::model::NewCustomer;
            use domain::customers::service as customers;
            let dto = customers::create_customer(
                db,
                tenant,
                NewCustomer {
                    name,
                    rut,
                    phone,
                    email,
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!("Cliente «{}» registrado.", dto.name),
                data: serde_json::json!({
                    "id": dto.id,
                    "name": dto.name,
                    "rut": dto.rut,
                }),
            }
        }
        Action::CrearProductoRapido { name, price, stock } => {
            use domain::catalog::model::NewProduct;
            use domain::catalog::service as catalog;
            let dto = catalog::create_product(
                db,
                tenant,
                NewProduct {
                    name,
                    slug: None,
                    description: None,
                    price,
                    cost_price: None,
                    stock,
                    category: None,
                    image_url: None,
                    external_id: None,
                    laboratory: None,
                    therapeutic_action: None,
                    active_ingredient: None,
                    prescription_type: None,
                    presentation: None,
                    discount_percent: None,
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!("Producto «{}» creado a {}.", dto.name, clp(dto.price)),
                data: serde_json::json!({
                    "id": dto.id,
                    "name": dto.name,
                    "price": dto.price.to_string(),
                    "stock": dto.stock,
                }),
            }
        }
        Action::AjustarPrecio {
            product_id,
            product_name: _,
            old_price,
            new_price,
        } => {
            use domain::catalog::model::UpdateProduct;
            use domain::catalog::service as catalog;
            let dto = catalog::update_product(
                db,
                tenant,
                &product_id,
                UpdateProduct {
                    price: Some(new_price),
                    ..Default::default()
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!(
                    "Precio de {} actualizado: de {} a {}.",
                    dto.name,
                    clp(old_price),
                    clp(dto.price),
                ),
                data: serde_json::json!({
                    "id": dto.id,
                    "name": dto.name,
                    "old_price": old_price.to_string(),
                    "new_price": dto.price.to_string(),
                }),
            }
        }
        Action::CerrarCaja {
            session_id,
            register_name,
            expected,
            counted,
        } => {
            use domain::cash_register::model::CloseSessionInput;
            use domain::cash_register::service as caja;
            let summary = caja::close_session(
                db,
                tenant,
                &session_id,
                CloseSessionInput {
                    closing_cash_counted: counted,
                    notes: Some("Cierre registrado por el agente".into()),
                },
            )
            .await?;
            let diff = counted - expected;
            ActionOutcome {
                action: label,
                text: format!(
                    "Caja «{}» cerrada: contaste {}, esperado {}; {}.",
                    register_name,
                    clp(counted),
                    clp(expected),
                    diff_text(diff),
                ),
                data: serde_json::json!({
                    "id": summary.session.id,
                    "register_name": register_name,
                    "counted": counted.to_string(),
                    "expected": expected.to_string(),
                    "discrepancia": diff.to_string(),
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
    fn parse_cierre_extracts_counted() {
        for q in [
            "cierra la caja con 50000",
            "cierra la caja con $50.000",
            "cerrar caja, conté 50000",
            "cuadra la caja con 50000",
        ] {
            match parse_action(q) {
                ActionParse::CierreCaja { counted } => assert_eq!(counted, dec("50000"), "q={q}"),
                other => panic!("expected CierreCaja for {q:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_cierre_without_amount_is_incomplete() {
        assert!(matches!(
            parse_action("cierra la caja"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn read_cash_question_is_not_an_action() {
        // "cuánto hay en caja" has no close verb → read intent CajaActual, not a
        // write. The write path must never steal it.
        assert_eq!(parse_action("cuánto hay en caja"), ActionParse::NotAnAction);
        assert_eq!(parse_action("efectivo en caja"), ActionParse::NotAnAction);
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
    fn parse_cliente_name_only() {
        match parse_action("crea un cliente Juan Pérez") {
            ActionParse::Cliente {
                name, rut, phone, ..
            } => {
                assert_eq!(name, "Juan Perez");
                assert_eq!(rut, None);
                assert_eq!(phone, None);
            }
            other => panic!("expected Cliente, got {other:?}"),
        }
    }

    #[test]
    fn parse_cliente_with_rut_and_phone() {
        match parse_action("registra al cliente Maria Soto rut 12.345.678-9 telefono 987654321") {
            ActionParse::Cliente {
                name, rut, phone, ..
            } => {
                assert_eq!(name, "Maria Soto");
                assert_eq!(rut.as_deref(), Some("12.345.678-9"));
                assert_eq!(phone.as_deref(), Some("987654321"));
            }
            other => panic!("expected Cliente, got {other:?}"),
        }
    }

    #[test]
    fn parse_cliente_without_name_is_incomplete() {
        assert!(matches!(
            parse_action("crea un cliente rut 11.111.111-1"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn read_customer_questions_are_not_actions() {
        // These belong to the read agent, not the write path.
        assert_eq!(parse_action("mejores clientes"), ActionParse::NotAnAction);
        assert_eq!(
            parse_action("cuántos clientes tengo"),
            ActionParse::NotAnAction
        );
    }

    #[test]
    fn parse_producto_with_dollar_price() {
        match parse_action("crea un producto Aspirina a $1000") {
            ActionParse::Producto { name, price, stock } => {
                assert_eq!(name, "Aspirina");
                assert_eq!(price, dec("1000"));
                assert_eq!(stock, 0);
            }
            other => panic!("expected Producto, got {other:?}"),
        }
    }

    #[test]
    fn parse_producto_with_precio_word_and_stock() {
        match parse_action("agrega producto Coca Cola precio 1500 stock 20") {
            ActionParse::Producto { name, price, stock } => {
                assert_eq!(name, "Coca Cola");
                assert_eq!(price, dec("1500"));
                assert_eq!(stock, 20);
            }
            other => panic!("expected Producto, got {other:?}"),
        }
    }

    #[test]
    fn parse_producto_without_price_is_incomplete() {
        assert!(matches!(
            parse_action("crea un producto Aspirina"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn parse_ajuste_precio_dollar() {
        match parse_action("cambia el precio de paracetamol a $1500") {
            ActionParse::AjustePrecio {
                product_name,
                new_price,
            } => {
                assert_eq!(product_name, "paracetamol");
                assert_eq!(new_price, dec("1500"));
            }
            other => panic!("expected AjustePrecio, got {other:?}"),
        }
    }

    #[test]
    fn parse_ajuste_precio_no_dollar() {
        match parse_action("ajusta el precio de coca cola a 2000") {
            ActionParse::AjustePrecio {
                product_name,
                new_price,
            } => {
                assert_eq!(product_name, "coca cola");
                assert_eq!(new_price, dec("2000"));
            }
            other => panic!("expected AjustePrecio, got {other:?}"),
        }
    }

    #[test]
    fn create_product_with_precio_word_is_not_an_adjust() {
        // "crea ... precio N" is a create (no price verb), not a reprice.
        assert!(matches!(
            parse_action("crea un producto Aspirina precio 1000"),
            ActionParse::Producto { .. }
        ));
    }

    #[test]
    fn read_price_question_is_not_an_action() {
        // No price verb → falls through to the read agent (PrecioProducto).
        assert_eq!(
            parse_action("precio de paracetamol"),
            ActionParse::NotAnAction
        );
        assert_eq!(
            parse_action("cuánto cuesta el ibuprofeno"),
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
