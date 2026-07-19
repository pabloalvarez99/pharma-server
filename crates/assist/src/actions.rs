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
        /// Catalogued product record id when the line name matches a product
        /// exactly (so a later receipt moves stock); `None` for an off-catalog
        /// (free-text) line. Resolved server-side at propose time (see [`build`]).
        product_id: Option<String>,
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
    /// Adjust a product's stock — reuses `domain::inventory::adjust` (which
    /// emits an audited `stock_movement`). Exactly one of `set`/`delta` is
    /// `Some`; `product_id` + `old_stock` are resolved server-side at propose
    /// time (see [`build`]). `old_stock` is for the confirmation prose only.
    AjustarStock {
        product_id: String,
        product_name: String,
        old_stock: i64,
        set: Option<i64>,
        delta: Option<i64>,
    },
    /// Receive a `draft` purchase order — reuses
    /// `domain::purchasing::receive_purchase_order` (one-shot: bumps stock +
    /// recomputes WAC cost for catalogued lines, flips the PO to `received`).
    /// The PO is resolved server-side at propose time (see [`build`]);
    /// `items`/`total` are for the confirmation prose only.
    RecibirOrdenCompra {
        po_id: String,
        items: i64,
        total: Decimal,
    },
    /// Cancel a `draft` purchase order — reuses
    /// `domain::purchasing::cancel_purchase_order` (draft → cancelled). The PO is
    /// resolved server-side at propose time (see [`build`]); `total` is for the
    /// confirmation prose only.
    CancelarOrdenCompra { po_id: String, total: Decimal },
    /// Open the cash drawer with a starting float — reuses
    /// `domain::cash_register::open_session` (per-cashier; rejects if the user
    /// already has an open drawer). Pairs with [`Action::CerrarCaja`].
    AbrirCaja { opening_cash: Decimal },
    /// Register a new supplier — reuses `domain::purchasing::create_supplier`.
    /// Unblocks the OC flow (`CrearOrdenCompraDraft` rejects unknown suppliers).
    CrearProveedor {
        name: String,
        rut: Option<String>,
        phone: Option<String>,
        email: Option<String>,
    },
    /// Dispense a (non-controlled) prescription — reuses
    /// `domain::prescriptions::create_prescription` (Health, Ley 20.000 ledger).
    /// `product_id` is resolved server-side at propose time (see [`build`]).
    /// Controlled prescriptions are intentionally NOT created here (they require
    /// doctor identification by law — routed to the Recetas screen).
    DispensarReceta {
        patient_name: String,
        patient_rut: String,
        product_id: Option<String>,
        product_name: Option<String>,
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
            Action::AjustarStock { .. } => "ajustar_stock",
            Action::RecibirOrdenCompra { .. } => "recibir_orden_compra",
            Action::CancelarOrdenCompra { .. } => "cancelar_orden_compra",
            Action::AbrirCaja { .. } => "abrir_caja",
            Action::CrearProveedor { .. } => "crear_proveedor",
            Action::DispensarReceta { .. } => "dispensar_receta",
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
            Action::AjustarStock {
                product_name,
                old_stock,
                set,
                delta,
                ..
            } => format!(
                "Ajustar el stock de {} de {} a {} unidades.",
                product_name,
                old_stock,
                stock_target(*old_stock, *set, *delta),
            ),
            Action::RecibirOrdenCompra { items, total, .. } => format!(
                "Recibir una orden de compra ({} {}) por {}: sube el stock y la deja como recibida.",
                items,
                if *items == 1 { "producto" } else { "productos" },
                clp(*total),
            ),
            Action::CancelarOrdenCompra { total, .. } => format!(
                "Cancelar una orden de compra (borrador) por {}.",
                clp(*total),
            ),
            Action::AbrirCaja { opening_cash } => format!(
                "Abrir la caja con un fondo inicial de {}.",
                clp(*opening_cash),
            ),
            Action::CrearProveedor { name, rut, .. } => match rut {
                Some(r) => format!("Registrar al proveedor «{name}» (RUT {r})."),
                None => format!("Registrar al proveedor «{name}»."),
            },
            Action::DispensarReceta {
                patient_name,
                patient_rut,
                product_name,
                ..
            } => match product_name {
                Some(p) => format!(
                    "Registrar la dispensación de {p} a {patient_name} (RUT {patient_rut})."
                ),
                None => format!(
                    "Registrar una receta a {patient_name} (RUT {patient_rut})."
                ),
            },
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
                product_id,
                product_name,
                quantity,
                unit_cost,
            } => serde_json::json!({
                "supplier_id": supplier_id,
                "supplier_name": supplier_name,
                "product_id": product_id,
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
            Action::AjustarStock {
                product_id,
                product_name,
                old_stock,
                set,
                delta,
            } => serde_json::json!({
                "product_id": product_id,
                "product_name": product_name,
                "old_stock": old_stock,
                "set": set,
                "delta": delta,
            }),
            Action::RecibirOrdenCompra {
                po_id,
                items,
                total,
            } => serde_json::json!({
                "po_id": po_id,
                "items": items,
                "total": total.to_string(),
            }),
            Action::CancelarOrdenCompra { po_id, total } => serde_json::json!({
                "po_id": po_id,
                "total": total.to_string(),
            }),
            Action::AbrirCaja { opening_cash } => serde_json::json!({
                "opening_cash": opening_cash.to_string(),
            }),
            Action::CrearProveedor {
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
            Action::DispensarReceta {
                patient_name,
                patient_rut,
                product_id,
                product_name,
            } => serde_json::json!({
                "patient_name": patient_name,
                "patient_rut": patient_rut,
                "product_id": product_id,
                "product_name": product_name,
            }),
        }
    }
}

/// Resulting stock after applying a `set`/`delta` adjustment to `old`. Exactly
/// one of `set`/`delta` is expected `Some`; if neither is, stock is unchanged.
fn stock_target(old: i64, set: Option<i64>, delta: Option<i64>) -> i64 {
    match (set, delta) {
        (Some(s), _) => s,
        (None, Some(d)) => old + d,
        (None, None) => old,
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
    /// An "ajustar stock" request; `product_name` still needs DB resolution into
    /// a record id + current stock (see [`build`]). Exactly one of `set`/`delta`
    /// is `Some`.
    AjusteStock {
        product_name: String,
        set: Option<i64>,
        delta: Option<i64>,
    },
    /// A "recibir orden de compra" request. The concrete draft PO (optionally
    /// scoped to `supplier_name`) is resolved server-side (see [`build`]).
    RecibirOc { supplier_name: Option<String> },
    /// A "cancelar orden de compra" request; the draft PO (optionally scoped to
    /// `supplier_name`) is resolved server-side (see [`build`]).
    CancelarOc { supplier_name: Option<String> },
    /// An "abrir caja" request with the starting float. No DB resolution needed.
    AperturaCaja { opening_cash: Decimal },
    /// A "crear proveedor" request with the text-derivable fields.
    Proveedor {
        name: String,
        rut: Option<String>,
        phone: Option<String>,
        email: Option<String>,
    },
    /// A "dispensar receta" request (non-controlled). `product_name` still needs
    /// DB resolution into a record id (see [`build`]).
    Receta {
        patient_name: String,
        patient_rut: String,
        product_name: Option<String>,
    },
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

    // Goods receipt of a draft PO. BEFORE the OC create branch, which also
    // matches "orden de compra": "recibe la orden de compra" is a receive, not a
    // create. Gated on a receive verb + a PO noun, never on a question.
    if ["recib", "recepcion"].iter().any(|v| q.contains(v))
        && (q.contains("orden")
            || q.contains(" oc")
            || q.contains("compra")
            || q.contains("mercaderia"))
        && !q.starts_with("que ")
        && !q.starts_with("cuanto")
        && !q.starts_with("cual")
    {
        return parse_recibe(&q);
    }

    // Cancel a draft PO. BEFORE the OC create branch (also matches "orden de
    // compra"): "cancela la orden de compra" is a cancel, not a create. Gated on
    // a cancel verb + a PO noun, never on a question.
    if ["cancela", "cancelar", "anula", "anular"]
        .iter()
        .any(|v| q.contains(v))
        && (q.contains("orden") || q.contains(" oc") || q.contains("compra"))
        && !q.starts_with("que ")
        && !q.starts_with("cuanto")
        && !q.starts_with("cual")
    {
        return parse_cancela(&q);
    }

    // Purchase order before everything else: "orden de compra" is unambiguous.
    if q.contains("orden de compra") || q.contains(" oc ") || q.starts_with("oc ") {
        if !has_create {
            return ActionParse::NotAnAction;
        }
        return parse_oc(&q);
    }

    // Cash-drawer OPEN: "abre la caja con $50.000 de fondo". Distinct open verbs
    // so it never collides with the close branch below or the read CajaActual.
    if q.contains("caja")
        && ["abre", "abrir", "apertura", "aperturar"]
            .iter()
            .any(|v| q.contains(v))
    {
        return parse_apertura(&q);
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

    // Stock adjustment: "repón 40 de paracetamol", "ajusta el stock de X a 100".
    // `parse_stock` returns `Some` only when it confidently sees a stock-adjust
    // command (verb + product + amount); otherwise `None`, so the read intents
    // StockProducto ("stock de X") and StockBajo ("qué tengo que reponer") are
    // left untouched. Before the price branch ("baja el stock" vs "baja el
    // precio") since it is gated on the word "stock".
    if let Some(parsed) = parse_stock(&q) {
        return parsed;
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

    // Dispense a prescription (Health): a create/dispense verb + "receta". The
    // read intents RecetasMes/Controlados carry no such verb → they fall through
    // to the read agent. "controlada" is rejected here (needs doctor by law).
    if q.contains("receta") && (has_create || q.contains("dispensa")) {
        return parse_receta(&q);
    }

    // New supplier: create verb + "proveedor". Before "cliente" is irrelevant
    // (distinct word). The OC branch above already claimed "orden de compra …
    // proveedor X", so this only fires for a bare "crea el proveedor X".
    if has_create && q.contains("proveedor") {
        return parse_proveedor(&q);
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

/// Parse a (non-controlled) prescription dispensing: patient name + RUT
/// (required) and an optional product. Form: "registra una receta a <patient>
/// rut <prut> [de <product>]". Controlled prescriptions are refused here — they
/// require doctor identification (Ley 20.000), routed to the Recetas screen.
fn parse_receta(q: &str) -> ActionParse {
    if q.contains("controlad") {
        return ActionParse::Incomplete(
            "Las recetas controladas requieren registrar al médico (Ley 20.000); hazlo en \
             la pantalla de Recetas."
                .into(),
        );
    }
    let hint = ActionParse::Incomplete(
        "Para registrar una receta dime el paciente y su RUT. Por ejemplo: «registra una \
         receta a Juan Pérez rut 12.345.678-9 de paracetamol»."
            .into(),
    );
    let Some((_, after)) = q.split_once("receta") else {
        return hint;
    };
    let mut rest = after.trim();
    for lead in [
        "a ",
        "para ",
        "al paciente ",
        "del paciente ",
        "de la paciente ",
        "paciente ",
        "de nombre ",
        ": ",
    ] {
        if let Some(s) = rest.strip_prefix(lead) {
            rest = s.trim();
            break;
        }
    }
    let scan = format!(" {rest}");
    // The patient RUT is the anchor: the name runs up to " rut ".
    let Some(rut_pos) = scan.find(" rut ") else {
        return ActionParse::Incomplete(
            "¿Cuál es el RUT del paciente? Por ejemplo: «… rut 12.345.678-9».".into(),
        );
    };
    let patient_name = titlecase(strip_trailing_punct(scan[..rut_pos].trim()).trim());
    if patient_name.is_empty() {
        return hint;
    }
    let after_rut = scan[rut_pos + 5..].trim();
    // First token after "rut" is the RUT itself.
    let patient_rut = after_rut
        .split_whitespace()
        .next()
        .map(|t| strip_trailing_punct(t).to_uppercase())
        .unwrap_or_default();
    if patient_rut.is_empty() {
        return ActionParse::Incomplete(
            "¿Cuál es el RUT del paciente? Por ejemplo: «… rut 12.345.678-9».".into(),
        );
    }
    // Optional product after a " de " that follows the RUT.
    let product_name = after_rut.find(" de ").map(|p| {
        strip_trailing_punct(after_rut[p + 4..].trim())
            .trim()
            .to_string()
    });
    let product_name = product_name.filter(|s| !s.is_empty());
    ActionParse::Receta {
        patient_name,
        patient_rut,
        product_name,
    }
}

/// Mirror of [`parse_cliente`] for suppliers: captures the name after
/// "proveedor", then optional rut/phone/email by the same field markers.
fn parse_proveedor(q: &str) -> ActionParse {
    let hint = ActionParse::Incomplete(
        "Para crear un proveedor dime su nombre. Por ejemplo: «crea el proveedor Farmaltda \
         rut 76.123.456-7»."
            .into(),
    );
    let Some((_, after)) = q.split_once("proveedor") else {
        return hint;
    };
    let mut rest = after.trim();
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
    let scan = format!(" {rest}");
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
    ActionParse::Proveedor {
        name,
        rut: token_after(&scan, &[" rut "]).map(|s| s.to_uppercase()),
        phone: token_after(&scan, &[" telefono ", " fono ", " celular ", " cel "]),
        email: token_after(&scan, &[" email ", " correo ", " mail ", " e-mail "]),
    }
}

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

/// Try to read a stock-adjust command. Returns `None` (→ read path) unless the
/// text confidently carries a stock-adjust verb; `Some(Incomplete)` when the
/// verb is present but the product/amount is missing; `Some(AjusteStock)` when
/// complete. Forms covered: "repón 40 de <p>", "descuenta 5 de <p>", "ajusta el
/// stock de <p> a 100", "deja el stock de <p> en 100".
fn parse_stock(q: &str) -> Option<ActionParse> {
    // Reposición-family verbs imply a stock add on their own (no "stock" word
    // needed). Note "repon" is NOT a substring of "reposicion" (s ≠ n), so the
    // read noun "reposición" never trips this.
    const REPO: &[&str] = &["repon", "repón", "repone", "reponer", "repongo", "repuse"];
    // Generic add/sub/set verbs need a stock cue to avoid colliding with the
    // create ("agrega un cliente") and price ("ajusta el precio") branches.
    const ADD: &[&str] = &["suma", "sumar", "agrega", "agregar", "ingresa", "ingresar"];
    const SUB: &[&str] = &[
        "descuenta",
        "descontar",
        "resta",
        "restar",
        "quita",
        "quitar",
        "merma",
        "mermar",
    ];
    const SET: &[&str] = &[
        "ajusta",
        "ajustar",
        "corrige",
        "corregir",
        "deja",
        "dejar",
        "fija",
        "fijar",
        "establece",
        "establecer",
    ];
    // A create request ("agrega/ingresa un producto/cliente …") overlaps the
    // generic add verbs — it is a create, never a stock adjust. Bail so the
    // create branches downstream own it.
    if CREATE_VERBS.iter().any(|v| q.contains(v))
        && (q.contains("producto") || q.contains("cliente") || q.contains("gasto"))
    {
        return None;
    }
    let stock_word = q.contains("stock") || q.contains("inventario");
    let has_repo = REPO.iter().any(|v| q.contains(v));
    let has_add = has_repo || (ADD.iter().any(|v| q.contains(v)) && stock_word);
    let has_sub = SUB.iter().any(|v| q.contains(v)) && (stock_word || q.contains(" de "));
    let has_set = SET.iter().any(|v| q.contains(v)) && stock_word;
    if !(has_add || has_sub || has_set) {
        return None;
    }
    // Never steal a read question ("qué tengo que reponer", "cuánto stock…").
    if q.starts_with("que ")
        || q.starts_with("cuanto")
        || q.starts_with("cual")
        || q.contains("tengo que repon")
        || q.contains("hay que repon")
        || q.contains("que repon")
        || q.contains("stock bajo")
        || q.contains("bajo stock")
    {
        return None;
    }
    let hint = ActionParse::Incomplete(
        "Para ajustar stock dime el producto y la cantidad. Por ejemplo: «repón 40 de \
         paracetamol» o «ajusta el stock de paracetamol a 100»."
            .into(),
    );
    let Some(n) = extract_count(q) else {
        return Some(hint);
    };
    let product = capture_stock_product(q);
    if product.is_empty() {
        return Some(hint);
    }
    // A set verb paired with an "a/en N" target sets the absolute level; an
    // add/sub verb applies a signed delta.
    if has_set && (q.contains(" a ") || q.contains(" en ")) {
        Some(ActionParse::AjusteStock {
            product_name: product,
            set: Some(n),
            delta: None,
        })
    } else if has_sub {
        Some(ActionParse::AjusteStock {
            product_name: product,
            set: None,
            delta: Some(-n),
        })
    } else {
        Some(ActionParse::AjusteStock {
            product_name: product,
            set: None,
            delta: Some(n),
        })
    }
}

/// First run of digits (CL thousands `.` allowed) in `q`, as a non-negative
/// count. Sign is carried by the verb, not the number.
fn extract_count(q: &str) -> Option<i64> {
    let bytes = q.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let s: String = q[start..i].chars().filter(|c| c.is_ascii_digit()).collect();
            return s.parse::<i64>().ok();
        }
        i += 1;
    }
    None
}

/// Capture the product name from a stock-adjust command. Prefers an explicit
/// "stock de <product>" anchor, else the first "<…> de <product>" tail. Strips a
/// leading article and cuts at the first digit or stock connector.
fn capture_stock_product(q: &str) -> String {
    for a in ["stock de ", "stock del ", "stock a ", "stock para "] {
        if let Some(p) = q.find(a) {
            return clean_stock_name(&q[p + a.len()..]);
        }
    }
    if let Some(p) = q.find(" de ") {
        let name = clean_stock_name(&q[p + 4..]);
        if !name.is_empty() {
            return name;
        }
    }
    String::new()
}

/// Strip a leading article and cut at the first digit or amount connector.
fn clean_stock_name(s: &str) -> String {
    let mut t = s.trim();
    for art in ["el ", "la ", "los ", "las ", "un ", "una ", "producto "] {
        if let Some(x) = t.strip_prefix(art) {
            t = x.trim();
        }
    }
    let cut = t.find(|c: char| c.is_ascii_digit()).unwrap_or(t.len());
    let mut head = t[..cut].trim();
    for conn in [" a", " en", " a $", " por", " con", " hasta"] {
        if let Some(x) = head.strip_suffix(conn) {
            head = x.trim();
            break;
        }
    }
    strip_trailing_punct(head).trim().to_string()
}

/// Parse a goods-receipt request. Captures an optional supplier after
/// "proveedor " or a trailing " de <name>", rejecting PO-noun noise words so
/// "recibe la orden de compra" resolves to "the latest draft" (no supplier).
fn parse_recibe(q: &str) -> ActionParse {
    const NOISE: &[&str] = &[
        "compra",
        "compras",
        "orden",
        "la orden",
        "oc",
        "la oc",
        "mercaderia",
        "borrador",
        "ultima orden",
    ];
    let raw = if let Some((_, r)) = q.split_once("proveedor ") {
        Some(r)
    } else {
        q.rfind(" de ").map(|p| &q[p + 4..])
    };
    let supplier_name = raw
        .map(|s| strip_trailing_punct(s.trim()).trim().to_string())
        .filter(|s| !s.is_empty() && !NOISE.contains(&s.as_str()));
    ActionParse::RecibirOc { supplier_name }
}

/// Parse a PO-cancel request, capturing an optional supplier the same way
/// [`parse_recibe`] does.
fn parse_cancela(q: &str) -> ActionParse {
    const NOISE: &[&str] = &[
        "compra",
        "compras",
        "orden",
        "la orden",
        "oc",
        "la oc",
        "borrador",
        "ultima orden",
    ];
    let raw = if let Some((_, r)) = q.split_once("proveedor ") {
        Some(r)
    } else {
        q.rfind(" de ").map(|p| &q[p + 4..])
    };
    let supplier_name = raw
        .map(|s| strip_trailing_punct(s.trim()).trim().to_string())
        .filter(|s| !s.is_empty() && !NOISE.contains(&s.as_str()));
    ActionParse::CancelarOc { supplier_name }
}

fn parse_apertura(q: &str) -> ActionParse {
    match extract_amount(q) {
        Some(opening_cash) => ActionParse::AperturaCaja { opening_cash },
        None => ActionParse::Incomplete(
            "Para abrir la caja dime con cuánto fondo partes. Por ejemplo: «abre la caja \
             con $50.000»."
                .into(),
        ),
    }
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
            // Link the line to a catalogued product only on an EXACT (case-
            // insensitive) name match, so a later receipt bumps the right
            // product's stock. Anything else stays a free-text line (off-catalog
            // buy) — never guess a fuzzy product and move the wrong stock.
            let product_id = {
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
                let want = product_name.to_lowercase();
                products
                    .iter()
                    .find(|p| p.name.to_lowercase() == want)
                    .map(|p| p.id.clone())
            };
            Ok(BuildOutcome::Ready(Action::CrearOrdenCompraDraft {
                supplier_id: sup.id.clone(),
                supplier_name: sup.name.clone(),
                product_id,
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
        ActionParse::AjusteStock {
            product_name,
            set,
            delta,
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
            let new_stock = stock_target(p.stock, set, delta);
            if new_stock == p.stock {
                return Ok(BuildOutcome::Reject(format!(
                    "El stock de {} ya está en {} unidades; no hay nada que ajustar.",
                    p.name, p.stock
                )));
            }
            if new_stock < 0 {
                return Ok(BuildOutcome::Reject(format!(
                    "No puedo dejar el stock de {} en {}: quedaría negativo (hoy tiene {}).",
                    p.name, new_stock, p.stock
                )));
            }
            Ok(BuildOutcome::Ready(Action::AjustarStock {
                product_id: p.id.clone(),
                product_name: p.name.clone(),
                old_stock: p.stock,
                set,
                delta,
            }))
        }
        ActionParse::RecibirOc { supplier_name } => {
            use domain::purchasing::model::{PurchaseOrderFilters, SupplierFilters};
            use domain::purchasing::service as purchasing;
            let supplier_id = match &supplier_name {
                Some(name) => {
                    let sups = purchasing::list_suppliers(
                        db,
                        tenant,
                        SupplierFilters {
                            search: Some(name.clone()),
                            active: Some(true),
                            limit: Some(5),
                            offset: None,
                        },
                    )
                    .await?;
                    match pick_supplier(&sups, name) {
                        Some(s) => Some(s.id.clone()),
                        None => {
                            return Ok(BuildOutcome::Reject(format!(
                                "No encontré al proveedor «{name}». Revisa el nombre."
                            )));
                        }
                    }
                }
                None => None,
            };
            let drafts = purchasing::list_purchase_orders(
                db,
                tenant,
                PurchaseOrderFilters {
                    supplier: supplier_id,
                    status: Some("draft".into()),
                    limit: Some(50),
                    offset: None,
                },
            )
            .await?;
            let Some(hdr) = drafts.into_iter().max_by_key(|p| p.created_at) else {
                let scope = supplier_name
                    .as_deref()
                    .map(|s| format!(" de «{s}»"))
                    .unwrap_or_default();
                return Ok(BuildOutcome::Reject(format!(
                    "No tienes órdenes de compra en borrador{scope} para recibir."
                )));
            };
            let po = purchasing::get_purchase_order(db, tenant, &hdr.id).await?;
            let items = po.items.iter().filter(|l| l.product.is_some()).count() as i64;
            Ok(BuildOutcome::Ready(Action::RecibirOrdenCompra {
                po_id: po.id,
                items,
                total: po.total,
            }))
        }
        ActionParse::CancelarOc { supplier_name } => {
            use domain::purchasing::model::{PurchaseOrderFilters, SupplierFilters};
            use domain::purchasing::service as purchasing;
            let supplier_id = match &supplier_name {
                Some(name) => {
                    let sups = purchasing::list_suppliers(
                        db,
                        tenant,
                        SupplierFilters {
                            search: Some(name.clone()),
                            active: Some(true),
                            limit: Some(5),
                            offset: None,
                        },
                    )
                    .await?;
                    match pick_supplier(&sups, name) {
                        Some(s) => Some(s.id.clone()),
                        None => {
                            return Ok(BuildOutcome::Reject(format!(
                                "No encontré al proveedor «{name}». Revisa el nombre."
                            )));
                        }
                    }
                }
                None => None,
            };
            let drafts = purchasing::list_purchase_orders(
                db,
                tenant,
                PurchaseOrderFilters {
                    supplier: supplier_id,
                    status: Some("draft".into()),
                    limit: Some(50),
                    offset: None,
                },
            )
            .await?;
            let Some(hdr) = drafts.into_iter().max_by_key(|p| p.created_at) else {
                let scope = supplier_name
                    .as_deref()
                    .map(|s| format!(" de «{s}»"))
                    .unwrap_or_default();
                return Ok(BuildOutcome::Reject(format!(
                    "No tienes órdenes de compra en borrador{scope} para cancelar."
                )));
            };
            Ok(BuildOutcome::Ready(Action::CancelarOrdenCompra {
                po_id: hdr.id,
                total: hdr.total,
            }))
        }
        ActionParse::AperturaCaja { opening_cash } => {
            Ok(BuildOutcome::Ready(Action::AbrirCaja { opening_cash }))
        }
        ActionParse::Proveedor {
            name,
            rut,
            phone,
            email,
        } => Ok(BuildOutcome::Ready(Action::CrearProveedor {
            name,
            rut,
            phone,
            email,
        })),
        ActionParse::Receta {
            patient_name,
            patient_rut,
            product_name,
        } => {
            // Resolve the product by EXACT (case-insensitive) name only, so a
            // dispensing links the right product; otherwise leave it product-less
            // (a prescription without a catalogued product is valid).
            let product_id = match &product_name {
                Some(name) => {
                    use domain::catalog::model::ProductFilters;
                    use domain::catalog::service as catalog;
                    let products = catalog::list_products(
                        db,
                        tenant,
                        ProductFilters {
                            search: Some(name.clone()),
                            limit: Some(5),
                            ..Default::default()
                        },
                    )
                    .await?;
                    let want = name.to_lowercase();
                    products
                        .iter()
                        .find(|p| p.name.to_lowercase() == want)
                        .map(|p| p.id.clone())
                }
                None => None,
            };
            Ok(BuildOutcome::Ready(Action::DispensarReceta {
                patient_name,
                patient_rut,
                product_id,
                product_name,
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
            product_id,
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
                        product: product_id,
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
                    attrs: None,
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
        Action::AjustarStock {
            product_id,
            product_name: _,
            old_stock,
            set,
            delta,
        } => {
            use domain::inventory::model::AdjustMovement;
            use domain::inventory::service as inventory;
            let actor_s = actor.map(|t| t.to_string());
            let (_mv, prod) = inventory::adjust(
                db,
                tenant,
                AdjustMovement {
                    product: product_id,
                    set,
                    delta,
                    reason: "Ajuste registrado por el agente".into(),
                    r#ref: None,
                },
                actor_s.as_deref(),
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!(
                    "Stock de {} ajustado: de {} a {} unidades.",
                    prod.name, old_stock, prod.stock
                ),
                data: serde_json::json!({
                    "id": prod.id,
                    "name": prod.name,
                    "old_stock": old_stock,
                    "new_stock": prod.stock,
                }),
            }
        }
        Action::RecibirOrdenCompra {
            po_id,
            items,
            total,
        } => {
            use domain::purchasing::service as purchasing;
            let actor_s = actor.map(|t| t.to_string());
            let po =
                purchasing::receive_purchase_order(db, tenant, &po_id, actor_s.as_deref()).await?;
            let word = if items == 1 { "producto" } else { "productos" };
            ActionOutcome {
                action: label,
                text: format!(
                    "Orden de compra recibida: actualicé el stock de {items} {word} y la \
                     dejé como {}.",
                    po.status
                ),
                data: serde_json::json!({
                    "id": po.id,
                    "status": po.status,
                    "items": items,
                    "total": total.to_string(),
                }),
            }
        }
        Action::CancelarOrdenCompra { po_id, total } => {
            use domain::purchasing::service as purchasing;
            let po = purchasing::cancel_purchase_order(db, tenant, &po_id).await?;
            ActionOutcome {
                action: label,
                text: format!("Orden de compra cancelada (quedó como {}).", po.status),
                data: serde_json::json!({
                    "id": po.id,
                    "status": po.status,
                    "total": total.to_string(),
                }),
            }
        }
        Action::AbrirCaja { opening_cash } => {
            use domain::cash_register::model::OpenSessionInput;
            use domain::cash_register::service as caja;
            let user = actor.ok_or_else(|| {
                domain::DomainError::Invalid(
                    "no pude identificar al usuario para abrir la caja".into(),
                )
            })?;
            let dto = caja::open_session(
                db,
                tenant,
                user,
                OpenSessionInput {
                    register_name: "caja-1".into(),
                    opening_cash,
                    notes: Some("Apertura registrada por el agente".into()),
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!(
                    "Caja «{}» abierta con un fondo de {}.",
                    dto.register_name,
                    clp(opening_cash)
                ),
                data: serde_json::json!({
                    "id": dto.id,
                    "register_name": dto.register_name,
                    "opening_cash": opening_cash.to_string(),
                }),
            }
        }
        Action::CrearProveedor {
            name,
            rut,
            phone,
            email,
        } => {
            use domain::purchasing::model::NewSupplier;
            use domain::purchasing::service as purchasing;
            let dto = purchasing::create_supplier(
                db,
                tenant,
                NewSupplier {
                    name,
                    rut,
                    contact_name: None,
                    contact_email: email,
                    contact_phone: phone,
                    default_invoice_format: None,
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!("Proveedor «{}» registrado.", dto.name),
                data: serde_json::json!({
                    "id": dto.id,
                    "name": dto.name,
                    "rut": dto.rut,
                }),
            }
        }
        Action::DispensarReceta {
            patient_name,
            patient_rut,
            product_id,
            product_name,
        } => {
            use domain::prescriptions::model::NewPrescription;
            use domain::prescriptions::service as prescriptions;
            let dto = prescriptions::create_prescription(
                db,
                tenant,
                NewPrescription {
                    product: product_id,
                    customer: None,
                    patient_name: patient_name.clone(),
                    patient_rut: patient_rut.clone(),
                    doctor_name: None,
                    doctor_rut: None,
                    controlled: false,
                    folio: None,
                    dispensed_at: None,
                },
            )
            .await?;
            let what = product_name
                .as_deref()
                .map(|p| format!("la dispensación de {p}"))
                .unwrap_or_else(|| "la receta".to_string());
            ActionOutcome {
                action: label,
                text: format!("Registré {what} a {patient_name} (RUT {patient_rut})."),
                data: serde_json::json!({
                    "id": dto.id,
                    "patient_name": dto.patient_name,
                    "patient_rut": dto.patient_rut,
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
    fn parse_apertura_extracts_opening() {
        for q in [
            "abre la caja con 50000",
            "abre la caja con $50.000 de fondo",
            "apertura de caja 50000",
        ] {
            match parse_action(q) {
                ActionParse::AperturaCaja { opening_cash } => {
                    assert_eq!(opening_cash, dec("50000"), "q={q}")
                }
                other => panic!("expected AperturaCaja for {q:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_apertura_without_amount_is_incomplete() {
        assert!(matches!(
            parse_action("abre la caja"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn open_and_close_caja_dont_collide() {
        assert!(matches!(
            parse_action("abre la caja con 50000"),
            ActionParse::AperturaCaja { .. }
        ));
        assert!(matches!(
            parse_action("cierra la caja con 50000"),
            ActionParse::CierreCaja { .. }
        ));
        // The read question is still neither.
        assert_eq!(parse_action("cuánto hay en caja"), ActionParse::NotAnAction);
    }

    #[test]
    fn read_cash_question_is_not_an_action() {
        // "cuánto hay en caja" has no close verb → read intent CajaActual, not a
        // write. The write path must never steal it.
        assert_eq!(parse_action("cuánto hay en caja"), ActionParse::NotAnAction);
        assert_eq!(parse_action("efectivo en caja"), ActionParse::NotAnAction);
    }

    #[test]
    fn parse_stock_delta_add() {
        match parse_action("repón 40 de paracetamol") {
            ActionParse::AjusteStock {
                product_name,
                set,
                delta,
            } => {
                assert_eq!(product_name, "paracetamol");
                assert_eq!(set, None);
                assert_eq!(delta, Some(40));
            }
            other => panic!("expected AjusteStock add, got {other:?}"),
        }
    }

    #[test]
    fn parse_stock_delta_sub() {
        match parse_action("descuenta 5 de paracetamol") {
            ActionParse::AjusteStock { set, delta, .. } => {
                assert_eq!(set, None);
                assert_eq!(delta, Some(-5));
            }
            other => panic!("expected AjusteStock sub, got {other:?}"),
        }
    }

    #[test]
    fn parse_stock_set_absolute() {
        match parse_action("ajusta el stock de paracetamol a 100") {
            ActionParse::AjusteStock {
                product_name,
                set,
                delta,
            } => {
                assert_eq!(product_name, "paracetamol");
                assert_eq!(set, Some(100));
                assert_eq!(delta, None);
            }
            other => panic!("expected AjusteStock set, got {other:?}"),
        }
        assert!(matches!(
            parse_action("deja el stock de ibuprofeno en 50"),
            ActionParse::AjusteStock { set: Some(50), .. }
        ));
    }

    #[test]
    fn parse_stock_without_amount_is_incomplete() {
        assert!(matches!(
            parse_action("repón paracetamol"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn read_stock_questions_are_not_actions() {
        // These belong to the read agent (StockProducto / StockBajo), never a
        // write — the adjust path must never steal them.
        assert_eq!(
            parse_action("stock de paracetamol"),
            ActionParse::NotAnAction
        );
        assert_eq!(
            parse_action("qué tengo que reponer"),
            ActionParse::NotAnAction
        );
        assert_eq!(
            parse_action("productos con stock bajo"),
            ActionParse::NotAnAction
        );
        assert_eq!(
            parse_action("cuánto stock hay de paracetamol"),
            ActionParse::NotAnAction
        );
    }

    #[test]
    fn create_and_price_not_stolen_by_stock() {
        // "agrega un cliente" stays a create; "ajusta el precio" stays a reprice.
        assert!(matches!(
            parse_action("agrega un cliente Juan Pérez"),
            ActionParse::Cliente { .. }
        ));
        assert!(matches!(
            parse_action("ajusta el precio de paracetamol a $1500"),
            ActionParse::AjustePrecio { .. }
        ));
    }

    #[test]
    fn parse_recibe_latest_draft() {
        for q in [
            "recibe la orden de compra",
            "recibe la última orden de compra",
            "recepciona la oc",
        ] {
            match parse_action(q) {
                ActionParse::RecibirOc { supplier_name } => {
                    assert_eq!(supplier_name, None, "q={q}")
                }
                other => panic!("expected RecibirOc for {q:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_recibe_with_supplier() {
        match parse_action("recibe la orden de compra de Farmaltda") {
            ActionParse::RecibirOc { supplier_name } => {
                assert_eq!(supplier_name.as_deref(), Some("farmaltda"))
            }
            other => panic!("expected RecibirOc supplier, got {other:?}"),
        }
        assert!(matches!(
            parse_action("recibe la oc del proveedor abcfarma"),
            ActionParse::RecibirOc {
                supplier_name: Some(_)
            }
        ));
    }

    #[test]
    fn create_oc_not_stolen_by_receive() {
        // "crea una orden de compra …" stays a create, not a receive.
        match parse_action("crea una orden de compra de 10 paracetamol a Farmaltda a $500") {
            ActionParse::Oc { .. } => {}
            other => panic!("expected Oc create, got {other:?}"),
        }
    }

    #[test]
    fn parse_cancela_latest_draft() {
        for q in [
            "cancela la orden de compra",
            "anula la última orden de compra",
            "cancela la oc",
        ] {
            match parse_action(q) {
                ActionParse::CancelarOc { supplier_name } => {
                    assert_eq!(supplier_name, None, "q={q}")
                }
                other => panic!("expected CancelarOc for {q:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn cancel_vs_create_and_receive() {
        // create stays create, receive stays receive — cancel never steals them.
        match parse_action("crea una orden de compra de 10 paracetamol a Farmaltda a $500") {
            ActionParse::Oc { .. } => {}
            other => panic!("expected Oc, got {other:?}"),
        }
        assert!(matches!(
            parse_action("recibe la orden de compra"),
            ActionParse::RecibirOc { .. }
        ));
        assert!(matches!(
            parse_action("cancela la orden de compra de Farmaltda"),
            ActionParse::CancelarOc {
                supplier_name: Some(_)
            }
        ));
    }

    #[test]
    fn parse_proveedor_name_and_rut() {
        match parse_action("crea el proveedor Farmaltda rut 76.123.456-7") {
            ActionParse::Proveedor { name, rut, .. } => {
                assert_eq!(name, "Farmaltda");
                assert_eq!(rut.as_deref(), Some("76.123.456-7"));
            }
            other => panic!("expected Proveedor, got {other:?}"),
        }
        assert!(matches!(
            parse_action("crea un proveedor Droguería Sur"),
            ActionParse::Proveedor { .. }
        ));
    }

    #[test]
    fn parse_proveedor_without_name_is_incomplete() {
        assert!(matches!(
            parse_action("crea un proveedor"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn parse_receta_patient_rut_product() {
        match parse_action("registra una receta a Juan Pérez rut 12.345.678-9 de paracetamol") {
            ActionParse::Receta {
                patient_name,
                patient_rut,
                product_name,
            } => {
                // normalize() folds accents before parsing (as every action does).
                assert_eq!(patient_name, "Juan Perez");
                assert_eq!(patient_rut, "12.345.678-9");
                assert_eq!(product_name.as_deref(), Some("paracetamol"));
            }
            other => panic!("expected Receta, got {other:?}"),
        }
        // Product is optional.
        assert!(matches!(
            parse_action("dispensa una receta a María Soto rut 9.876.543-2"),
            ActionParse::Receta {
                product_name: None,
                ..
            }
        ));
    }

    #[test]
    fn parse_receta_controlled_is_refused() {
        assert!(matches!(
            parse_action("registra una receta controlada a Juan rut 1-9 de morfina"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn parse_receta_without_rut_is_incomplete() {
        assert!(matches!(
            parse_action("registra una receta a Juan Pérez"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn read_recetas_question_is_not_an_action() {
        // "recetas del mes" / "libro de controlados" carry no create/dispense
        // verb → they belong to the read agent (RecetasMes / Controlados).
        assert_eq!(parse_action("recetas del mes"), ActionParse::NotAnAction);
        assert_eq!(
            parse_action("libro de controlados"),
            ActionParse::NotAnAction
        );
    }

    #[test]
    fn proveedor_vs_oc_and_receive() {
        // Creating an OC to a supplier is an OC, not a CrearProveedor.
        match parse_action("crea una orden de compra de 5 ibuprofeno a Farmaltda a $300") {
            ActionParse::Oc { .. } => {}
            other => panic!("expected Oc, got {other:?}"),
        }
        // Receiving from a supplier is a receive, not a CrearProveedor.
        assert!(matches!(
            parse_action("recibe la oc del proveedor abcfarma"),
            ActionParse::RecibirOc { .. }
        ));
        // The read "mejores proveedores" carries no create verb → not an action.
        assert_eq!(
            parse_action("cuántos proveedores tengo"),
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
