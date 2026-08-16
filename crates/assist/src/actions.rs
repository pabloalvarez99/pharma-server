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
use domain::settings;
use domain::DomainResult;

use crate::money::Money;
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
    /// Record a payment against a customer's fiado (cuenta corriente) — reuses
    /// `domain::credit::service::record_abono` (which validates amount > 0, that
    /// there IS debt, and that the payment does not exceed it). `customer_id` +
    /// `debt_before` are resolved server-side at propose time (see [`build`]);
    /// `debt_before` is for the confirmation prose only.
    RegistrarAbono {
        customer_id: String,
        customer_name: String,
        amount: Decimal,
        debt_before: Decimal,
    },
    /// Sell over the counter — reuses `domain::sales::service::post_sale`, the
    /// SAME atomic transaction the POS screen posts (stock decrement, stock
    /// movements, FEFO lot consumption, loyalty, receta autodetection and the
    /// drug-interaction check). Nothing about a sale is reimplemented here.
    ///
    /// `lines` (with the catalog's own `unit_price`), `subtotal`, `total` and
    /// `warnings` are all resolved server-side at propose time (see [`build`]):
    /// the money comes from `domain::invariants`, never from arithmetic typed
    /// into this crate, so the agent and the till always agree on the number.
    Vender {
        lines: Vec<VentaLinea>,
        subtotal: Decimal,
        total: Decimal,
        /// Optional buyer (loyalty points, sale history). `None` = walk-in.
        customer_id: Option<String>,
        customer_name: Option<String>,
        /// es-CL drug-interaction warnings for this cart, so the owner reads
        /// them BEFORE confirming (see [`interaction_warnings`]).
        warnings: Vec<String>,
        /// Feria sin sesión abierta al proponer: al confirmar se abre el puesto
        /// con $0. La prosa de confirmación avisa; no aplica en farmacia.
        abre_puesto: bool,
    },
    /// The same sale, charged to the customer's cuenta corriente (`pos_fiado`):
    /// the drawer takes no cash and `post_sale` posts the cargo through
    /// `domain::credit::repo::post_cargo`. The customer is mandatory —
    /// "fiar" without a name is not a sale, it is a gift.
    FiarVenta {
        lines: Vec<VentaLinea>,
        subtotal: Decimal,
        total: Decimal,
        customer_id: String,
        customer_name: String,
        /// What the customer owed at propose time; confirmation prose only.
        debt_before: Decimal,
        warnings: Vec<String>,
    },
}

/// One resolved sale line. `unit_price` is the CATALOG price read at propose
/// time — never a number the owner said out loud, never computed here — and it
/// is frozen into the confirm token, so the confirmed sale charges exactly what
/// the confirmation prompt showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaLinea {
    pub product_id: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price: Decimal,
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
            Action::RegistrarAbono { .. } => "registrar_abono",
            Action::Vender { .. } => "vender",
            Action::FiarVenta { .. } => "fiar_venta",
        }
    }

    /// One-line es-CL summary the UI shows before the owner confirms.
    ///
    /// `m` es la moneda del tenant: la confirmación tiene que decir la misma
    /// plata que va a cobrar. Ver [`crate::money`].
    pub fn summary(&self, m: &Money) -> String {
        match self {
            Action::RegistrarGasto {
                description,
                amount,
                ..
            } => format!(
                "Registrar un gasto de {} por «{}».",
                m.fmt(*amount),
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
                m.fmt(*unit_cost),
            ),
            Action::CrearCliente { name, rut, .. } => match rut {
                Some(r) => format!("Registrar al cliente «{name}» (RUT {r})."),
                None => format!("Registrar al cliente «{name}»."),
            },
            Action::CrearProductoRapido { name, price, stock } => format!(
                "Crear el producto «{}» a {} ({} {} de stock inicial).",
                name,
                m.fmt(*price),
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
                m.fmt(*old_price),
                m.fmt(*new_price),
            ),
            Action::CerrarCaja {
                register_name,
                expected,
                counted,
                ..
            } => format!(
                "Cerrar la caja «{}» con {} contados (esperado {}, {}).",
                register_name,
                m.fmt(*counted),
                m.fmt(*expected),
                diff_text(*counted - *expected, m),
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
                m.fmt(*total),
            ),
            Action::CancelarOrdenCompra { total, .. } => format!(
                "Cancelar una orden de compra (borrador) por {}.",
                m.fmt(*total),
            ),
            Action::AbrirCaja { opening_cash } => format!(
                "Abrir la caja con un fondo inicial de {}.",
                m.fmt(*opening_cash),
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
            Action::RegistrarAbono {
                customer_name,
                amount,
                debt_before,
                ..
            } => format!(
                "Registrar un abono de {} de {} (debe {}).",
                m.fmt(*amount),
                customer_name,
                m.fmt(*debt_before),
            ),
            // A sale moves stock AND money, so the confirmation prompt is the
            // owner's only defence against a mis-heard product or quantity: it
            // spells out every line (producto, cantidad, precio unitario, total
            // de la línea), the total to charge, who is being fiado, and any
            // drug-interaction warning — never a one-line "vender 2 cosas".
            Action::Vender {
                lines,
                total,
                customer_name,
                warnings,
                abre_puesto,
                ..
            } => {
                let mut s = format!("Vender:\n{}\n", venta_lineas_text(lines, m));
                s.push_str(&format!("Total a cobrar: {} en efectivo.", m.fmt(*total)));
                if let Some(c) = customer_name {
                    s.push_str(&format!("\nSe la anoto a {c}."));
                }
                if *abre_puesto {
                    s.push_str(
                        "\nEl puesto no tenía el día abierto: lo abro con $0 y cobro.",
                    );
                }
                s.push_str(&warnings_text(warnings));
                s
            }
            Action::FiarVenta {
                lines,
                total,
                customer_name,
                debt_before,
                warnings,
                ..
            } => {
                let mut s = format!("Fiar:\n{}\n", venta_lineas_text(lines, m));
                s.push_str(&format!(
                    "Total: {}. Queda fiado a {} (hoy debe {}).",
                    m.fmt(*total),
                    customer_name,
                    m.fmt(*debt_before),
                ));
                s.push_str(&warnings_text(warnings));
                s
            }
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
            Action::RegistrarAbono {
                customer_id,
                customer_name,
                amount,
                debt_before,
            } => serde_json::json!({
                "customer_id": customer_id,
                "customer_name": customer_name,
                "amount": amount.to_string(),
                "debt_before": debt_before.to_string(),
            }),
            Action::Vender {
                lines,
                subtotal,
                total,
                customer_id,
                customer_name,
                warnings,
                abre_puesto,
            } => serde_json::json!({
                "lines": venta_lineas_json(lines),
                "subtotal": subtotal.to_string(),
                "total": total.to_string(),
                "payment_method": "pos_cash",
                "customer_id": customer_id,
                "customer_name": customer_name,
                "warnings": warnings,
                "abre_puesto": abre_puesto,
            }),
            Action::FiarVenta {
                lines,
                subtotal,
                total,
                customer_id,
                customer_name,
                debt_before,
                warnings,
            } => serde_json::json!({
                "lines": venta_lineas_json(lines),
                "subtotal": subtotal.to_string(),
                "total": total.to_string(),
                "payment_method": "pos_fiado",
                "customer_id": customer_id,
                "customer_name": customer_name,
                "debt_before": debt_before.to_string(),
                "warnings": warnings,
            }),
        }
    }
}

/// One "- 2 × Paracetamol 500 mg a $990 c/u = $1.980" per line. The line total
/// comes from `domain::invariants::line_total` — the same formula the sale
/// itself uses — so the prompt can never quote a number the till disagrees with.
fn venta_lineas_text(lines: &[VentaLinea], m: &Money) -> String {
    lines
        .iter()
        .map(|l| {
            format!(
                "- {} × {} a {} c/u = {}",
                l.quantity,
                l.product_name,
                m.fmt(l.unit_price),
                m.fmt(domain::invariants::line_total(l.unit_price, l.quantity)),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Structured echo of the lines so the UI can render a confirmation card
/// without re-parsing [`venta_lineas_text`].
fn venta_lineas_json(lines: &[VentaLinea]) -> serde_json::Value {
    serde_json::Value::Array(
        lines
            .iter()
            .map(|l| {
                serde_json::json!({
                    "product_id": l.product_id,
                    "product_name": l.product_name,
                    "quantity": l.quantity,
                    "unit_price": l.unit_price.to_string(),
                    "line_total": domain::invariants::line_total(l.unit_price, l.quantity)
                        .to_string(),
                })
            })
            .collect(),
    )
}

/// Append drug-interaction warnings to a confirmation prompt (empty string when
/// the cart is clean).
fn warnings_text(warnings: &[String]) -> String {
    warnings
        .iter()
        .map(|w| format!("\nOjo: {w}"))
        .collect::<Vec<_>>()
        .join("")
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
fn diff_text(diff: Decimal, m: &Money) -> String {
    if diff.is_zero() {
        "calza exacto".to_string()
    } else if diff.is_sign_positive() {
        format!("sobran {}", m.fmt(diff))
    } else {
        format!("faltan {}", m.fmt(diff.abs()))
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
    /// `m` es la moneda del tenant: la lee el resumen que se le muestra al
    /// dueño antes de confirmar.
    pub fn propose(&self, action: Action, tenant: &Thing, m: &Money) -> ActionProposal {
        self.propose_with_ttl(action, tenant, m, TOKEN_TTL_SECS)
    }

    /// Issue a token with an explicit TTL (seconds). A non-positive `ttl_secs`
    /// produces an already-expired token — used by tests to exercise the expiry
    /// path deterministically.
    pub fn propose_with_ttl(
        &self,
        action: Action,
        tenant: &Thing,
        m: &Money,
        ttl_secs: i64,
    ) -> ActionProposal {
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(ttl_secs);
        let proposal = ActionProposal {
            name: action.label(),
            summary: action.summary(m),
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
    /// A "registrar abono" request (pago de fiado); `customer_name` still needs
    /// DB resolution into a record id + current debt (see [`build`]).
    Abono {
        customer_name: String,
        amount: Decimal,
    },
    /// A sale. `fiado` picks the payment method (cuenta corriente vs efectivo);
    /// every `product_name` still needs DB resolution into a record id + the
    /// catalog price, and `customer_name` into a customer id (see [`build`]).
    Venta {
        lines: Vec<VentaLineaParse>,
        customer_name: Option<String>,
        fiado: bool,
    },
}

/// One "<cantidad> <producto>" heard in a sale request, before any DB lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaLineaParse {
    pub product_name: String,
    pub quantity: i64,
    /// Precio oído en la misma frase («tomates a 2000»). None = usar catálogo.
    pub unit_price: Option<Decimal>,
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

    // THE SALE — the counter's central act, so it is tested early. Like
    // `parse_stock` it returns `Some` only when it confidently sees a sale
    // command (sale/charge/fiar verb + something to sell); otherwise `None`, so
    // the read intents about sales ("cuánto vendí hoy", "cuánto fiado tengo",
    // "quién me debe") and the other write branches below are left untouched.
    // `question` (not `q`) is threaded through because a capital letter is the
    // cheapest signal that "a Juan" is a person and "a granel" is not.
    if let Some(parsed) = parse_venta(&q, question) {
        return parsed;
    }

    // Feria notebook form: "Don Juan debe 5000" / "me debe 5000 don Juan".
    // Not a full sale (no product) — nudge to the fiar_venta shape the ERP
    // can execute. Before abono so "debe" is not confused with "pagó".
    if let Some(parsed) = parse_deuda_feria_nudge(&q, question) {
        return parsed;
    }

    // Fiado payment: "abónale 5000 a doña Ana", "doña Ana me pagó 3000". Gated
    // on a word-initial "abon*" cue (so "jabón" never trips it) or the "me pagó"
    // form — NEVER on a bare "pago", which collides with the read intents
    // VentasPorMetodo ("método de pago") and IvaMes ("cuánto IVA pago"). The
    // read intent PorCobrar ("cuánto me deben") carries no abono cue either.
    // "abono" is also fertilizer in es-CL, so a product create keeps priority.
    if (has_abono_cue(&q) || q.contains("me pago")) && !(has_create && q.contains("producto")) {
        return parse_abono(&q);
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

/// True when `q` carries a word-initial "abon*" cue (abona / abónale / abono /
/// abonar / abonó). Word-initial on purpose: "jabón" contains "abon" and must
/// never be read as a fiado payment.
fn has_abono_cue(q: &str) -> bool {
    q.split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with("abon"))
}

/// Parse a fiado payment: "abónale 5000 a doña Ana", "doña Ana me pagó 3000".
/// The amount is the first digit run; the customer comes from
/// [`capture_abono_customer`]. Missing either → a friendly es-CL nudge.
fn parse_abono(q: &str) -> ActionParse {
    let Some(amount) = extract_amount(q) else {
        return ActionParse::Incomplete(
            "¿De cuánto es el abono? Por ejemplo: «abónale 5000 a doña Ana».".into(),
        );
    };
    let customer_name = capture_abono_customer(q);
    if customer_name.is_empty() {
        return ActionParse::Incomplete(
            "¿Quién te pagó? Por ejemplo: «abónale 5000 a doña Ana».".into(),
        );
    }
    ActionParse::Abono {
        customer_name,
        amount,
    }
}

/// Customer connectors of a fiado payment, longest first so " a la " wins over
/// " a ".
const ABONO_CONN: &[&str] = &[" a la ", " al ", " a ", " de la ", " del ", " de "];

/// Capture the customer of a fiado payment. Three forms, in order: "<cliente> me
/// pagó 5000" (name BEFORE the cue), "abónale 5000 a <cliente>" (name after the
/// connector that follows the amount) and "abónale a <cliente> 5000" (connector
/// before the amount). Empty when none of them yields a name.
fn capture_abono_customer(q: &str) -> String {
    if let Some(p) = q.find("me pago") {
        let head = clean_abono_name(&q[..p]);
        if !head.is_empty() {
            return head;
        }
    }
    let tail = after_amount(q);
    for c in ABONO_CONN {
        if let Some(p) = tail.find(c) {
            let name = clean_abono_name(&tail[p + c.len()..]);
            if !name.is_empty() {
                return name;
            }
        }
    }
    // "abónale a doña Ana 5000": the connector sits between the cue and the
    // amount, so scan that window instead.
    if let Some(p) = q.find("abon") {
        let from_cue = &q[p..];
        let cut = from_cue
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(from_cue.len());
        let window = &from_cue[..cut];
        for c in ABONO_CONN {
            if let Some(pp) = window.find(c) {
                let name = clean_abono_name(&window[pp + c.len()..]);
                if !name.is_empty() {
                    return name;
                }
            }
        }
    }
    String::new()
}

/// Slice of `q` following the first run of money digits. Empty when `q` carries
/// no digits (or the amount ends the text).
fn after_amount(q: &str) -> &str {
    let bytes = q.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            return &q[i..];
        }
        i += 1;
    }
    ""
}

/// Strip imperative verbs, articles and honorifics off a captured customer name
/// ("registra que la señora ana" → "ana"). Dropping "doña"/"don" widens the
/// fuzzy search on purpose: the owner says "doña Ana", the DB row says "Ana
/// Pérez", and `search_customers` matches on substring.
fn clean_abono_name(s: &str) -> String {
    const LEADS: &[&str] = &[
        "registra ",
        "registrar ",
        "anota ",
        "anotar ",
        "carga ",
        "cargar ",
        "ingresa ",
        "ingresar ",
        "guarda ",
        "guardar ",
        "abono ",
        "abonar ",
        "que ",
        "un ",
        "una ",
        "el ",
        "la ",
        "cliente ",
        "clienta ",
        "senora ",
        "senor ",
        "sra ",
        "sr ",
        "dona ",
        "don ",
    ];
    let mut t = strip_trailing_punct(s.trim()).trim();
    loop {
        let before = t;
        for lead in LEADS {
            if let Some(x) = t.strip_prefix(lead) {
                t = x.trim();
            }
        }
        if t == before {
            break;
        }
    }
    strip_trailing_punct(t).trim().to_string()
}

// ---- venta (el acto central del mesón) ---------------------------------------

/// Largest quantity a single spoken line may carry. A shop order of more than
/// this is far likelier to be a mis-heard price than a real quantity, so the
/// agent asks again instead of proposing it.
const VENTA_MAX_QTY: i64 = 10_000;

/// Cap on lines in one spoken sale. Past this the till screen is the right tool.
const VENTA_MAX_LINEAS: usize = 20;

/// Imperative forms that introduce a SALE. Exact words on purpose (not stems):
/// "vendidos"/"vendí" belong to the READ intents ("los más vendidos", "cuánto
/// vendí hoy") and a stem match would steal them. "bend*" is the everyday b/v
/// misspelling in Chile.
const VENTA_VERBS: &[&str] = &[
    "vende",
    "vendeme",
    "vendele",
    "vendelo",
    "vendela",
    "vendeles",
    "vender",
    "venderle",
    "venda",
    "vendame",
    "bende",
    "bendeme",
    "bendele",
    "bender",
    "despacha",
    "despachame",
    "despachale",
    "despachar",
];

/// Imperative "chárgaselo a X" forms. Apart from [`VENTA_VERBS`] because
/// "cobrar" is also the READ vocabulary of PorCobrar ("cuentas por cobrar"),
/// which [`parse_venta`] refuses outright.
const COBRO_VERBS: &[&str] = &["cobra", "cobrale", "cobrame", "cobrales", "cobrar", "cobrarle"];

/// Imperative forms of "fiar" that can ANCHOR a sale ("fíale 1 alcohol gel a
/// Juan"). The adjective forms ("fiado"/"fiada") are deliberately absent: they
/// sit AFTER the product ("1 alcohol gel fiado a Juan"), so anchoring on them
/// would swallow the product name.
const FIAR_VERBS: &[&str] = &[
    "fia", "fiale", "fiame", "fialo", "fiala", "fiar", "fiarle", "fiaselo",
];

/// Honorifics that mark the tail of a sale as a PERSON, not more product.
const VENTA_HONORIFICS: &[&str] = &[
    "senora ", "senor ", "sra ", "sr ", "srta ", "senorita ", "don ", "dona ", "cliente ",
    "clienta ", "caballero ", "la senora ", "el senor ",
];

/// Connectors that can introduce the buyer at the tail of a sale request.
const VENTA_CUST_CONN: &[&str] = &[" a la ", " para la ", " para el ", " al ", " a ", " para "];

/// Spoken filler that is never product nor customer ("vende 2 x 500 AL TIRO").
/// Stripped BEFORE the customer is captured, so "al tiro" can never be read as
/// "al «Tiro»".
const VENTA_FILLER: &[&str] = &[
    " al tiro",
    " altiro",
    " por favor",
    " porfa",
    " porfis",
    " ahora",
    " ya",
    " rapidito",
    " rapido",
    " gracias",
    " please",
    " de una",
];

/// Spoken small numbers. The owner says "un paracetamol", not "1 paracetamol".
const VENTA_NUM_WORDS: &[(&str, i64)] = &[
    ("un", 1),
    ("una", 1),
    ("uno", 1),
    ("dos", 2),
    ("tres", 3),
    ("cuatro", 4),
    ("cinco", 5),
    ("seis", 6),
    ("siete", 7),
    ("ocho", 8),
    ("nueve", 9),
    ("diez", 10),
    ("once", 11),
    ("doce", 12),
    ("docena", 12),
    ("par", 2),
];

/// What kind of verb opened the request. It decides how a request WITHOUT a
/// product is answered: "véndeme" with nothing to sell earns a nudge, while
/// "cóbrale 5000 a doña Ana" is not a sale at all and must fall through to the
/// read agent untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VentaAnchor {
    /// vender / fiar — unmistakably a sale.
    Venta,
    /// cobrar, or a generic create verb riding a "fiado" cue.
    Otro,
}

/// True when `word` is one of `verbs`, or one single-character typo away from
/// one of them (both at least 5 chars, so short words are matched exactly).
/// Covers the everyday "vendme"/"cobrle" slips without opening the door to
/// unrelated vocabulary.
fn fuzzy_verb(word: &str, verbs: &[&str]) -> bool {
    if verbs.contains(&word) {
        return true;
    }
    if word.chars().count() < 5 {
        return false;
    }
    verbs
        .iter()
        .filter(|v| v.chars().count() >= 5)
        .any(|v| edit_distance_1(word, v))
}

/// True when `a` and `b` are exactly one insertion, deletion or substitution
/// apart. Cheap early-outs, no allocation, no full distance matrix.
fn edit_distance_1(a: &str, b: &str) -> bool {
    let (av, bv): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (la, lb) = (av.len(), bv.len());
    if la.abs_diff(lb) > 1 {
        return false;
    }
    let mut i = 0;
    let mut j = 0;
    let mut edits = 0;
    while i < la && j < lb {
        if av[i] == bv[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match la.cmp(&lb) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + (la - i) + (lb - j) == 1
}

/// True when `q` carries a fiado cue: the adjective ("fiado"/"fiada"), an
/// imperative form of "fiar", or the everyday paraphrases. Word-initial like
/// [`has_abono_cue`], so "confía" or "desafía" never trip it.
fn has_fiado_cue(q: &str) -> bool {
    q.split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with("fiad") || FIAR_VERBS.contains(&w))
        || q.contains("a cuenta")
        || q.contains("la libreta")
}

/// Feria notebook: "Don Juan debe 5000" — name + debe + money, no product.
/// We refuse to invent a SKU; we teach the full fiar_venta phrase instead.
fn parse_deuda_feria_nudge(q: &str, raw: &str) -> Option<ActionParse> {
    // Questions stay on the read path ("¿quién me debe?").
    if q.starts_with("quien")
        || q.starts_with("cuanto")
        || q.starts_with("cuánt")
        || q.contains("quien me debe")
        || q.contains("cuanto me deben")
        || q.contains("cuanto me debe")
        || q.contains("me deben")
    {
        return None;
    }
    // Need a "debe" token (not "debemos" stock talk) and a money amount.
    let has_debe = q.split(|c: char| !c.is_alphanumeric()).any(|w| w == "debe" || w == "deben");
    if !has_debe {
        return None;
    }
    // Pull first money-looking run of digits.
    let amount = {
        let mut found = None;
        for tok in q.split_whitespace() {
            let digits: String = tok.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 3 {
                if let Some(m) = parse_money(&digits) {
                    found = Some(m);
                    break;
                }
            }
        }
        found?
    };
    // Name: prefer text before "debe", else after honorifics in the raw string.
    let name = {
        let before = q.split(" debe").next().unwrap_or("").trim();
        let cleaned = before
            .trim_start_matches("don ")
            .trim_start_matches("dona ")
            .trim_start_matches("doña ")
            .trim_start_matches("sr ")
            .trim_start_matches("sra ")
            .trim_start_matches("me ")
            .trim();
        if cleaned.is_empty() || cleaned.chars().all(|c| c.is_ascii_digit()) {
            // "me debe 5000 don juan" — tail after money.
            if let Some(idx) = raw.to_lowercase().find("debe") {
                let tail = raw[idx..].split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
                let t = clean_abono_name(&tail);
                if t.is_empty() {
                    return Some(ActionParse::Incomplete(
                        "¿A quién se lo fío y qué le vendiste? Por ejemplo: «anota 2 kg de \
                         tomates a 2000 fiado a Don Juan»."
                            .into(),
                    ));
                }
                titlecase(&t)
            } else {
                return Some(ActionParse::Incomplete(
                    "¿A quién se lo fío y qué le vendiste? Por ejemplo: «anota 2 kg de \
                     tomates a 2000 fiado a Don Juan»."
                        .into(),
                ));
            }
        } else {
            titlecase(cleaned)
        }
    };
    Some(ActionParse::Incomplete(format!(
        "Anoté que {name} debería unos ${amount}, pero necesito qué le vendiste. \
         Por ejemplo: «anota 2 kg de tomates a 2000 fiado a {name}»."
    )))
}

/// Byte offset just past the verb that opens a sale, plus what kind of verb it
/// was. A generic create verb ("anota …") only anchors when the text also
/// carries a fiado cue — that is the "anota 1 alcohol gel fiado a Juan" form.
fn venta_anchor(q: &str, fiado: bool) -> Option<(usize, VentaAnchor)> {
    let mut create_at: Option<usize> = None;
    let mut idx = 0usize;
    for w in q.split_whitespace() {
        let start = idx + q[idx..].find(w)?;
        let end = start + w.len();
        idx = end;
        let word: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
        if fuzzy_verb(&word, VENTA_VERBS) || FIAR_VERBS.contains(&word.as_str()) {
            return Some((end, VentaAnchor::Venta));
        }
        if fuzzy_verb(&word, COBRO_VERBS) {
            return Some((end, VentaAnchor::Otro));
        }
        if create_at.is_none() && CREATE_VERBS.iter().any(|v| v.trim() == word) {
            create_at = Some(end);
        }
    }
    if fiado {
        create_at.map(|p| (p, VentaAnchor::Otro))
    } else {
        None
    }
}

/// Drop spoken filler off the tail (repeatedly: "al tiro por favor").
fn strip_venta_filler(s: &str) -> &str {
    let mut t = strip_trailing_punct(s.trim());
    loop {
        let before = t;
        for f in VENTA_FILLER {
            if let Some(x) = t.strip_suffix(f) {
                t = strip_trailing_punct(x.trim());
            }
        }
        if t == before {
            return t;
        }
    }
}

/// Remove the fiado markers from the item region so they never land inside a
/// product name ("1 alcohol gel FIADO a Juan"). The two "… de <cliente>" forms
/// collapse to a plain " a " so the buyer connector survives the cleanup.
fn strip_fiado_markers(s: &str) -> String {
    let mut t = format!(" {} ", s.trim());
    for (from, to) in [
        (" a cuenta de ", " a "),
        (" en la libreta de ", " a "),
        (" a la libreta de ", " a "),
    ] {
        while let Some(p) = t.find(from) {
            t.replace_range(p..p + from.len(), to);
        }
    }
    for m in [
        " al fiado ",
        " fiado ",
        " fiada ",
        " fiados ",
        " a cuenta ",
        " en la libreta ",
        " a la libreta ",
    ] {
        while let Some(p) = t.find(m) {
            t.replace_range(p..p + m.len(), " ");
        }
    }
    t.trim().to_string()
}

/// Split "<items> a <cliente>" at the LAST buyer connector. The tail is only
/// taken as a person when the text really points at one — a fiado sale (there
/// is always someone to fiar to), an honorific ("a la señora Pérez"), or a
/// capitalised word in the ORIGINAL question ("a Juan"). Otherwise the whole
/// region stays product, so "arroz a granel" is never sold to «Granel».
fn split_venta_customer(region: &str, raw: &str, fiado: bool) -> (String, Option<String>) {
    let whole = region.trim().to_string();
    let hay = format!(" {} ", region.trim());
    let mut best: Option<(usize, usize)> = None;
    for c in VENTA_CUST_CONN {
        let mut from = 0usize;
        while let Some(rel) = hay[from..].find(c) {
            let pos = from + rel;
            let better = match best {
                Some((p, l)) => pos > p || (pos == p && c.len() > l),
                None => true,
            };
            if better {
                best = Some((pos, c.len()));
            }
            from = pos + 1;
        }
    }
    let Some((pos, len)) = best else {
        return (whole, None);
    };
    let head = hay[..pos].trim().to_string();
    let tail = hay[pos + len..].trim();
    if head.is_empty() || tail.is_empty() {
        return (whole, None);
    }
    let honorific = VENTA_HONORIFICS.iter().any(|h| tail.starts_with(h));
    let name = clean_abono_name(tail);
    let Some(first) = name.split_whitespace().next() else {
        return (whole, None);
    };
    if fiado || honorific || word_is_capitalized_in(raw, first) {
        (head, Some(titlecase(&name)))
    } else {
        (whole, None)
    }
}

/// True when `word` (already normalized) appears capitalised in the owner's
/// ORIGINAL text. `normalize` folds accents, which shifts byte offsets, so the
/// comparison runs word by word instead of by slicing.
fn word_is_capitalized_in(raw: &str, word: &str) -> bool {
    raw.split_whitespace().any(|w| {
        let stripped = strip_trailing_punct(w);
        normalize(stripped) == word
            && stripped
                .chars()
                .next()
                .is_some_and(|c| c.is_uppercase())
    })
}

/// Split the item region into lines on " y " / ",". Only splits when EVERY
/// later segment opens with a quantity, so "alcohol gel y jabón" (one product
/// whose name carries a "y") stays a single line.
fn split_venta_lineas(items: &str) -> Vec<String> {
    let parts: Vec<&str> = items
        .split(" y ")
        .flat_map(|p| p.split(','))
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() > 1 && parts[1..].iter().all(|p| starts_with_quantity(p)) {
        parts.into_iter().map(str::to_string).collect()
    } else {
        vec![items.trim().to_string()]
    }
}

fn starts_with_quantity(seg: &str) -> bool {
    let s = seg.trim_start();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        return true;
    }
    let first = s.split_whitespace().next().unwrap_or("");
    VENTA_NUM_WORDS.iter().any(|(w, _)| *w == first)
}

/// Leading quantity of a line: digits, a spoken small number, or an implicit 1
/// ("véndeme paracetamol" = one).
fn take_quantity(seg: &str) -> (i64, &str) {
    let s = seg.trim();
    let head: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if !head.is_empty() {
        let digits: String = head.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<i64>() {
            return (n, &s[head.len()..]);
        }
    }
    let first = s.split_whitespace().next().unwrap_or("");
    if let Some((_, n)) = VENTA_NUM_WORDS.iter().find(|(w, _)| *w == first) {
        return (*n, &s[first.len()..]);
    }
    (1, s)
}

/// Strip counting words and articles off a captured product name. "2 x 500"
/// leaves "500" — a product token (the 500 mg presentation), which the catalog
/// lookup in [`build`] then resolves or reports as ambiguous.
fn clean_venta_product(s: &str) -> String {
    let mut t = strip_trailing_punct(s.trim()).trim();
    loop {
        let before = t;
        // Feria / calle (ADR-0022): "2 kg de tomates", "1 atado de cilantro",
        // "arroz a granel". Quantity already taken; unit words are not product.
        for lead in [
            "unidades de ",
            "unidad de ",
            "cajas de ",
            "caja de ",
            "tiras de ",
            "tira de ",
            "frascos de ",
            "frasco de ",
            "kilos de ",
            "kilo de ",
            "kg de ",
            "kgs de ",
            "atados de ",
            "atado de ",
            "bolsas de ",
            "bolsa de ",
            "bandejas de ",
            "bandeja de ",
            "docenas de ",
            "docena de ",
            "mallas de ",
            "malla de ",
            "medios kilos de ",
            "medio kilo de ",
            "media de ",
            "productos ",
            "producto ",
            "unidades ",
            "unidad ",
            "kilos ",
            "kilo ",
            "kg ",
            "kgs ",
            "atados ",
            "atado ",
            "bolsas ",
            "bolsa ",
            "bandejas ",
            "bandeja ",
            "docenas ",
            "docena ",
            "mallas ",
            "malla ",
            "x ",
            "de ",
            "del ",
            "el ",
            "la ",
            "los ",
            "las ",
        ] {
            if let Some(rest) = t.strip_prefix(lead) {
                t = rest.trim();
            }
        }
        if t == before {
            break;
        }
    }
    // Trailing unit: "tomates kg", "cebolla granel".
    for cut in [
        " a granel",
        " granel",
        " kg",
        " kgs",
        " kilo",
        " kilos",
        " atado",
        " atados",
        " bandeja",
        " bandejas",
        " docena",
        " docenas",
        " malla",
        " mallas",
        " a",
        " al",
        " para",
        " por",
    ] {
        if let Some(x) = t.strip_suffix(cut) {
            t = x.trim();
            break;
        }
    }
    strip_trailing_punct(t).trim().to_string()
}

/// Try to read a sale. `None` (→ read path) unless the text confidently carries
/// a sale command; `Some(Incomplete)` when the verb is there but the order is
/// not; `Some(Venta)` when it is. Never guesses a product: the catalog lookup
/// and the human confirmation in [`build`] are what stand between a mis-heard
/// word and a stock movement.
fn parse_venta(q: &str, raw: &str) -> Option<ActionParse> {
    // A question is never a sale.
    if ["que ", "cuanto", "cual", "quien", "cuando", "donde", "como "]
        .iter()
        .any(|p| q.starts_with(p))
    {
        return None;
    }
    // The fiado LEDGER reads share the vocabulary but never carry an order.
    if ["por cobrar", "me deben", "me debe", "quien debe", "cuentas por"]
        .iter()
        .any(|w| q.contains(w))
    {
        return None;
    }
    // Other whitelisted writes own these nouns.
    if q.contains("orden de compra") || q.contains("gasto") {
        return None;
    }
    let fiado = has_fiado_cue(q);
    let (anchor, kind) = venta_anchor(q, fiado)?;
    // A generic create verb is only borrowed for the fiado form; it must not
    // hijack "crea un producto/cliente/proveedor/receta".
    if kind == VentaAnchor::Otro
        && ["producto", "proveedor", "receta"]
            .iter()
            .any(|n| q.contains(n))
    {
        return None;
    }
    let region = strip_fiado_markers(strip_venta_filler(&q[anchor..]));
    let (items, customer_name) = split_venta_customer(&region, raw, fiado);
    let items = strip_venta_filler(&items).to_string();

    let nudge = |msg: &str| match kind {
        // "véndeme" with nothing to sell deserves an answer; "cóbrale 5000 a
        // doña Ana" is not a sale at all — leave it to the read agent.
        VentaAnchor::Venta => Some(ActionParse::Incomplete(msg.to_string())),
        VentaAnchor::Otro => None,
    };
    if items.is_empty() {
        return nudge("¿Qué te compraron? Por ejemplo: «véndeme 2 paracetamol».");
    }

    let mut lines = Vec::new();
    for seg in split_venta_lineas(&items) {
        let (quantity, rest) = take_quantity(&seg);
        let unit_price = crate::feria_catalogo::precio_dicho(&seg);
        let mut product_name = clean_venta_product(rest);
        if unit_price.is_some() {
            product_name = crate::feria_catalogo::sin_precio_cola(&product_name);
        }
        if product_name.is_empty() {
            continue;
        }
        if quantity <= 0 || quantity > VENTA_MAX_QTY {
            return Some(ActionParse::Incomplete(format!(
                "¿{quantity} unidades de {product_name}? Dime la cantidad de nuevo, por si te \
                 entendí mal."
            )));
        }
        lines.push(VentaLineaParse {
            product_name,
            quantity,
            unit_price,
        });
    }
    if lines.is_empty() {
        return nudge("¿Qué te compraron? Por ejemplo: «véndeme 2 paracetamol».");
    }
    if lines.len() > VENTA_MAX_LINEAS {
        return Some(ActionParse::Incomplete(
            "Son demasiados productos para cantármelos de una; cóbralos en la pantalla de \
             venta."
                .into(),
        ));
    }
    if fiado && customer_name.is_none() {
        return Some(ActionParse::Incomplete(
            "¿A quién se lo fío? Por ejemplo: «anota 1 alcohol gel fiado a Juan».".into(),
        ));
    }
    Some(ActionParse::Venta {
        lines,
        customer_name,
        fiado,
    })
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
    // La moneda del tenant, para los rechazos que citan un monto. Ver
    // `crate::money`.
    let m = Money::from(settings::currency(db, tenant).await?);
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
        ActionParse::Abono {
            customer_name,
            amount,
        } => {
            use domain::customers::model::CustomerSearchQuery;
            use domain::customers::service as customers;
            let matches = customers::search_customers(
                db,
                tenant,
                CustomerSearchQuery {
                    q: Some(customer_name.clone()),
                },
            )
            .await?;
            let Some(c) = pick_customer(&matches, &customer_name) else {
                return Ok(BuildOutcome::Reject(format!(
                    "No encontré ningún cliente que coincida con «{customer_name}». \
                     Revisa el nombre o créalo primero."
                )));
            };
            let Ok(customer_thing) = surrealdb::sql::thing(&c.id) else {
                return Ok(BuildOutcome::Reject(format!(
                    "No pude identificar al cliente «{}».",
                    c.name
                )));
            };
            // The debt is read at propose time so the confirmation prose can say
            // what the customer owes; `record_abono` re-validates at execute time
            // (the ledger may have moved between the two steps).
            let debt = domain::credit::repo::balance(db, tenant, &customer_thing).await?;
            if debt <= Decimal::ZERO {
                return Ok(BuildOutcome::Reject(format!(
                    "{} no tiene deuda pendiente, así que no hay nada que abonar.",
                    c.name
                )));
            }
            if amount > debt {
                return Ok(BuildOutcome::Reject(format!(
                    "El abono de {} supera lo que {} debe ({}). Dime un monto menor o igual.",
                    m.fmt(amount),
                    c.name,
                    m.fmt(debt),
                )));
            }
            Ok(BuildOutcome::Ready(Action::RegistrarAbono {
                customer_id: c.id.clone(),
                customer_name: c.name.clone(),
                amount,
                debt_before: debt,
            }))
        }
        ActionParse::Venta {
            lines,
            customer_name,
            fiado,
        } => build_venta(db, tenant, &lines, customer_name.as_deref(), fiado).await,
    }
}

/// Resolve a spoken sale into a ready [`Action::Vender`] / [`Action::FiarVenta`]
/// — or into a friendly es-CL refusal. Everything that can go wrong is caught
/// HERE, before a token is minted, so the owner reads a sentence instead of
/// confirming a sale that will fail:
///
/// * caja cerrada (efectivo only — fiado moves no cash),
/// * producto inexistente / ambiguo / que se vende por variante,
/// * medicamento controlado (Ley 20.000 — se va a Recetas, ver abajo),
/// * stock insuficiente,
/// * cliente inexistente o ambiguo (obligatorio al fiar).
///
/// Clinical safety: this path crosses the SAME two controls a screen sale
/// crosses — `domain::sales::controlled::is_controlled` and
/// `domain::sales::interactions::check` — and it crosses them EARLIER. A
/// controlled substance is refused outright by voice (dispensing it needs the
/// prescribing doctor on the record, which a spoken order cannot carry), and
/// interaction warnings are shown BEFORE the owner confirms, not only in the
/// sale's response. `post_sale` runs both again at execute time; nothing here
/// replaces them.
async fn build_venta(
    db: &Db,
    tenant: &Thing,
    parsed: &[VentaLineaParse],
    customer_name: Option<&str>,
    fiado: bool,
) -> DomainResult<BuildOutcome> {
    // Cash needs an open drawer: the sale's cash lands in the OPEN session's
    // running total (migración 0030), so selling with the drawer closed would
    // put money in a till that no arqueo will ever count. Fiado takes no cash.
    // Feria: no Reject — el puesto se abre en execute (build no tiene user).
    let mut abre_puesto = false;
    if !fiado {
        let abierta = crate::feria_caja::hay_caja_abierta(db, tenant).await?;
        if !abierta {
            if crate::feria_caja::es_feria(db, tenant).await? {
                abre_puesto = true;
            } else {
                return Ok(BuildOutcome::Reject(
                    "No tienes la caja abierta, así que todavía no puedo cobrar. Ábrela primero \
                     («abre la caja con $50.000») y te la registro al tiro."
                        .into(),
                ));
            }
        }
    }

    use domain::catalog::model::ProductFilters;
    use domain::catalog::service as catalog;
    let mut lines: Vec<VentaLinea> = Vec::with_capacity(parsed.len());
    let mut ingredients: Vec<String> = Vec::new();
    // Same SKU on two lines still has to fit in one stock: accumulate.
    let mut pedido: HashMap<String, i64> = HashMap::new();
    for want in parsed {
        let products = catalog::list_products(
            db,
            tenant,
            ProductFilters {
                search: Some(want.product_name.clone()),
                active: Some(true),
                limit: Some(5),
                ..Default::default()
            },
        )
        .await?;
        let p = match pick_venta_product(&products, &want.product_name) {
            VentaMatch::Varios(names) => {
                return Ok(BuildOutcome::Reject(format!(
                    "Tengo varios productos que coinciden con «{}»: {}. ¿Cuál te compraron?",
                    want.product_name,
                    names.join(", "),
                )));
            }
            VentaMatch::Uno(p) => p.clone(),
            VentaMatch::Ninguno => {
                if crate::feria_caja::es_feria(db, tenant).await? {
                    match want.unit_price {
                        Some(price) => {
                            crate::feria_catalogo::asegurar_cosa_feria(
                                db,
                                tenant,
                                &want.product_name,
                                price,
                            )
                            .await?
                        }
                        None => {
                            return Ok(BuildOutcome::Reject(format!(
                                "No tengo «{}» en lo que vendes. Decime el precio, por ejemplo: \
                                 «vendí {} a 2000».",
                                want.product_name, want.product_name
                            )));
                        }
                    }
                } else {
                    return Ok(BuildOutcome::Reject(format!(
                        "No encontré ningún producto que se llame «{}». Revisa el nombre o créalo \
                         primero.",
                        want.product_name
                    )));
                }
            }
        };
        // A multi-SKU parent is not sellable: `post_sale` refuses it too, but
        // saying so before the confirmation saves the owner a dead end.
        if p.variant_count.unwrap_or(0) > 0 {
            return Ok(BuildOutcome::Reject(format!(
                "«{}» se vende por talla o modelo. Dime cuál o escanea el código de la que se \
                 llevan.",
                p.name
            )));
        }
        // Ley 20.000: dispensing a controlled substance needs the prescribing
        // doctor identified, which a spoken order cannot carry. Refuse rather
        // than sell it with a thinner record than the screen would keep.
        if domain::sales::controlled::is_controlled(p.active_ingredient.as_deref()) {
            return Ok(BuildOutcome::Reject(format!(
                "«{}» es un medicamento controlado: hay que dejar registrado al médico y la \
                 receta (Ley 20.000), así que esa venta va por la pantalla de Recetas.",
                p.name
            )));
        }
        let acc = pedido.entry(p.id.clone()).or_insert(0);
        *acc += want.quantity;
        if p.physical_stock && p.stock < *acc {
            return Ok(BuildOutcome::Reject(if p.stock <= 0 {
                format!("Te quedaste sin stock de {}.", p.name)
            } else {
                format!(
                    "No me alcanza el stock de {}: me pides {} y quedan {}.",
                    p.name, *acc, p.stock
                )
            }));
        }
        if let Some(ai) = p.active_ingredient.as_deref() {
            if !ai.trim().is_empty() {
                ingredients.push(ai.to_string());
            }
        }
        lines.push(VentaLinea {
            product_id: p.id.clone(),
            product_name: p.name.clone(),
            quantity: want.quantity,
            unit_price: p.price,
        });
    }

    // Buyer. Strict on purpose: fiando to the wrong Juan creates a debt on an
    // innocent customer, so an ambiguous name asks instead of guessing (unlike
    // the looser `pick_customer` the abono flow can afford).
    let mut buyer: Option<(String, String)> = None;
    if let Some(name) = customer_name {
        use domain::customers::model::CustomerSearchQuery;
        use domain::customers::service as customers;
        let matches = customers::search_customers(
            db,
            tenant,
            CustomerSearchQuery {
                q: Some(name.to_string()),
            },
        )
        .await?;
        match pick_venta_customer(&matches, name) {
            VentaMatch::Ninguno => {
                return Ok(BuildOutcome::Reject(format!(
                    "No encontré a ningún cliente que se llame «{name}». Créalo primero («crea \
                     un cliente {name}») y te lo anoto."
                )));
            }
            VentaMatch::Varios(names) => {
                return Ok(BuildOutcome::Reject(format!(
                    "Tengo varios clientes que coinciden con «{name}»: {}. ¿A cuál se la anoto?",
                    names.join(", "),
                )));
            }
            VentaMatch::Uno(c) => buyer = Some((c.id.clone(), c.name.clone())),
        }
    }

    // Money: the domain's own canonical formulas — the very ones `post_sale`
    // applies to these same frozen lines — so the confirmation prompt and the
    // till can never quote two different totals. `assist` does no arithmetic.
    let subtotal = domain::invariants::order_subtotal(
        lines.iter().map(|l| (l.unit_price, l.quantity)),
    );
    let total = domain::invariants::order_total(subtotal, Decimal::ZERO);
    let warnings = interaction_warnings(&ingredients);

    if fiado {
        let Some((customer_id, customer_name)) = buyer else {
            return Ok(BuildOutcome::Reject(
                "Para fiar necesito saber a quién. Dime el cliente y lo anoto.".into(),
            ));
        };
        let Ok(customer_thing) = surrealdb::sql::thing(&customer_id) else {
            return Ok(BuildOutcome::Reject(format!(
                "No pude identificar al cliente «{customer_name}»."
            )));
        };
        let debt_before = domain::credit::repo::balance(db, tenant, &customer_thing).await?;
        return Ok(BuildOutcome::Ready(Action::FiarVenta {
            lines,
            subtotal,
            total,
            customer_id,
            customer_name,
            debt_before,
            warnings,
        }));
    }
    let (customer_id, customer_name) = match buyer {
        Some((id, name)) => (Some(id), Some(name)),
        None => (None, None),
    };
    Ok(BuildOutcome::Ready(Action::Vender {
        lines,
        subtotal,
        total,
        customer_id,
        customer_name,
        warnings,
        abre_puesto,
    }))
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
fn pick_customer<'a>(
    customers: &'a [domain::customers::model::CustomerDto],
    name: &str,
) -> Option<&'a domain::customers::model::CustomerDto> {
    let want = name.to_lowercase();
    customers
        .iter()
        .find(|c| c.name.to_lowercase() == want)
        .or_else(|| customers.first())
}

/// Outcome of resolving a spoken name against the catalog / the customer book
/// for a SALE. Unlike the looser `pick_*` helpers, "several hits and no exact
/// match" is its own answer: a sale moves stock and money, so the agent asks
/// rather than picking the first row.
enum VentaMatch<'a, T> {
    Ninguno,
    Uno(&'a T),
    /// Names of the candidates, for the "¿cuál?" question.
    Varios(Vec<String>),
}

fn pick_venta_product<'a>(
    products: &'a [domain::catalog::model::ProductDto],
    name: &str,
) -> VentaMatch<'a, domain::catalog::model::ProductDto> {
    let want = name.to_lowercase();
    if let Some(p) = products.iter().find(|p| p.name.to_lowercase() == want) {
        return VentaMatch::Uno(p);
    }
    match products.len() {
        0 => VentaMatch::Ninguno,
        1 => VentaMatch::Uno(&products[0]),
        _ => VentaMatch::Varios(products.iter().take(3).map(|p| p.name.clone()).collect()),
    }
}

fn pick_venta_customer<'a>(
    customers: &'a [domain::customers::model::CustomerDto],
    name: &str,
) -> VentaMatch<'a, domain::customers::model::CustomerDto> {
    let want = name.to_lowercase();
    if let Some(c) = customers.iter().find(|c| c.name.to_lowercase() == want) {
        return VentaMatch::Uno(c);
    }
    match customers.len() {
        0 => VentaMatch::Ninguno,
        1 => VentaMatch::Uno(&customers[0]),
        _ => VentaMatch::Varios(customers.iter().take(3).map(|c| c.name.clone()).collect()),
    }
}

/// Run the SAME drug-interaction checker the till screen runs
/// (`domain::sales::interactions::check`) over the cart's active ingredients
/// and phrase each hit in es-CL. Surfacing it at PROPOSE time is the point: the
/// screen only shows the warning in the sale's response, i.e. after the fact.
fn interaction_warnings(ingredients: &[String]) -> Vec<String> {
    domain::sales::interactions::check(ingredients)
        .iter()
        .map(warning_text)
        .collect()
}

/// One interaction, in the owner's words.
fn warning_text(d: &domain::sales::interactions::InteractionDetail) -> String {
    use domain::sales::interactions::Severity;
    let sev = match d.severity {
        Severity::Critica => "riesgo crítico",
        Severity::Mayor => "riesgo alto",
        Severity::Moderada => "riesgo moderado",
    };
    format!(
        "{} + {} ({}). {} {}",
        d.drugs.0, d.drugs.1, sev, d.effect, d.recommendation
    )
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
    // La moneda del tenant, para la prosa del resultado. Ver `crate::money`.
    let m = Money::from(settings::currency(db, tenant).await?);
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
                    m.fmt(dto.amount)
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
                    // Casa matriz: el agente todavía no pregunta "¿para qué
                    // local?". Elegir sucursal por voz es lane aparte; hasta
                    // entonces el borrador entra donde entraba antes de 0042.
                    branch: None,
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
                    m.fmt(dto.total),
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
                    physical_stock: None,
                    discount_percent: None,
                    attrs: None,
                },
            )
            .await?;
            ActionOutcome {
                action: label,
                text: format!("Producto «{}» creado a {}.", dto.name, m.fmt(dto.price)),
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
                    m.fmt(old_price),
                    m.fmt(dto.price),
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
                    m.fmt(counted),
                    m.fmt(expected),
                    diff_text(diff, &m),
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
                    register: None,
                    branch: None,
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
                    m.fmt(opening_cash)
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
        Action::RegistrarAbono {
            customer_id,
            customer_name,
            amount,
            debt_before,
        } => {
            use domain::credit::service as credit;
            let customer = surrealdb::sql::thing(&customer_id).map_err(|_| {
                domain::DomainError::Invalid("no pude identificar al cliente del abono".into())
            })?;
            let entry = credit::record_abono(
                db,
                tenant,
                &customer,
                amount,
                None,
                Some("Registrado por el agente"),
                actor,
            )
            .await?;
            let saldo = debt_before - amount;
            ActionOutcome {
                action: label,
                text: if saldo.is_zero() {
                    format!(
                        "Abono de {} registrado a {}: quedó al día.",
                        m.fmt(amount),
                        customer_name,
                    )
                } else {
                    format!(
                        "Abono de {} registrado a {}: ahora debe {}.",
                        m.fmt(amount),
                        customer_name,
                        m.fmt(saldo),
                    )
                },
                data: serde_json::json!({
                    "id": entry.id,
                    "customer_id": customer_id,
                    "customer_name": customer_name,
                    "amount": amount.to_string(),
                    "saldo": saldo.to_string(),
                }),
            }
        }
        Action::Vender {
            lines,
            total,
            customer_id,
            customer_name,
            ..
        } => {
            // Feria day-1: open the puesto with $0 before posting so the cash
            // counts in the arqueo. Fiado never opens. No actor → skip open
            // (post_sale still records the order without a cash session).
            if crate::feria_caja::es_feria(db, tenant).await? {
                if let Some(user) = actor {
                    crate::feria_caja::asegurar_caja_feria(db, tenant, user).await?;
                }
            }
            // `cash_amount` = the frozen total: the lines and their prices are
            // the same ones `post_sale` re-totals, so the drawer's running cash
            // and the order's total can never drift apart.
            let resp = post_venta(
                db,
                tenant,
                actor,
                &lines,
                "pos_cash",
                Some(total),
                customer_id.clone(),
                customer_name.clone(),
            )
            .await?;
            let mut text = format!(
                "Venta lista: {}. Total {}, en efectivo.",
                venta_detalle(&lines),
                m.fmt(resp.order.total),
            );
            if resp.loyalty_points_awarded > 0 {
                if let Some(c) = customer_name.as_deref() {
                    text.push_str(&format!(
                        " Le sumé {} puntos a {c}.",
                        resp.loyalty_points_awarded
                    ));
                }
            }
            text.push_str(&outcome_warnings(&resp.interaction_warnings));
            ActionOutcome {
                action: label,
                text,
                data: serde_json::json!({
                    "id": resp.order.id,
                    "total": resp.order.total.to_string(),
                    "payment_method": resp.order.payment_method,
                    "customer_id": customer_id,
                    "customer_name": customer_name,
                    "lines": venta_lineas_json(&lines),
                    "loyalty_points_awarded": resp.loyalty_points_awarded,
                }),
            }
        }
        Action::FiarVenta {
            lines,
            total: _,
            customer_id,
            customer_name,
            ..
        } => {
            let customer = surrealdb::sql::thing(&customer_id).map_err(|_| {
                domain::DomainError::Invalid("no pude identificar al cliente de la venta".into())
            })?;
            // `pos_fiado`: no cash. `post_sale` posts the cargo to the customer's
            // ledger through `domain::credit::repo::post_cargo` itself.
            let resp = post_venta(
                db,
                tenant,
                actor,
                &lines,
                "pos_fiado",
                None,
                Some(customer_id.clone()),
                Some(customer_name.clone()),
            )
            .await?;
            // Re-read the ledger instead of subtracting here: the balance the
            // owner hears is the one the books actually hold.
            let saldo = domain::credit::repo::balance(db, tenant, &customer).await?;
            let mut text = format!(
                "Fiado anotado: {}. Total {}. {} ahora debe {}.",
                venta_detalle(&lines),
                m.fmt(resp.order.total),
                customer_name,
                m.fmt(saldo),
            );
            text.push_str(&outcome_warnings(&resp.interaction_warnings));
            ActionOutcome {
                action: label,
                text,
                data: serde_json::json!({
                    "id": resp.order.id,
                    "total": resp.order.total.to_string(),
                    "payment_method": resp.order.payment_method,
                    "customer_id": customer_id,
                    "customer_name": customer_name,
                    "lines": venta_lineas_json(&lines),
                    "saldo": saldo.to_string(),
                }),
            }
        }
    };

    write_audit(db, tenant, actor, label).await?;
    Ok(outcome)
}

/// Post a confirmed sale through `domain::sales::service::post_sale` — the same
/// entry point `POST /api/v1/pos/sale` uses. NOTHING about a sale is
/// reimplemented in this crate: stock decrement, stock movements, FEFO lot
/// consumption, the fiado cargo, loyalty points, receta autodetection and the
/// interaction check all happen inside that one call, so a sale by voice and a
/// sale by screen leave identical books behind.
#[allow(clippy::too_many_arguments)]
async fn post_venta(
    db: &Db,
    tenant: &Thing,
    actor: Option<&Thing>,
    lines: &[VentaLinea],
    payment_method: &str,
    cash_amount: Option<Decimal>,
    customer: Option<String>,
    customer_name: Option<String>,
) -> DomainResult<domain::sales::model::PosSaleResponse> {
    use domain::sales::model::{PosSaleItem, PosSaleRequest};
    use domain::sales::service as sales;
    let req = PosSaleRequest {
        items: lines
            .iter()
            .map(|l| PosSaleItem {
                product: l.product_id.clone(),
                product_name: l.product_name.clone(),
                quantity: l.quantity,
                unit_price: l.unit_price,
            })
            .collect(),
        payment_method: payment_method.to_string(),
        cash_amount,
        card_amount: None,
        discount: None,
        customer,
        customer_name,
        customer_phone: None,
        notes: Some("Venta registrada por el agente".into()),
        external_ref: None,
        prescriptions: Vec::new(),
        branch: None,
    };
    sales::post_sale(db, tenant, actor, None, None, req).await
}

/// "2 × Paracetamol 500 mg, 1 × Ibuprofeno" — the inline form used once the
/// sale already happened (the multi-line breakdown belongs to the confirmation).
fn venta_detalle(lines: &[VentaLinea]) -> String {
    lines
        .iter()
        .map(|l| format!("{} × {}", l.quantity, l.product_name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Interaction warnings returned by the executed sale, appended to the outcome
/// so they reach the owner even if the cart changed between the two steps.
fn outcome_warnings(warnings: &[domain::sales::interactions::InteractionDetail]) -> String {
    warnings
        .iter()
        .map(|w| format!(" Ojo: {}", warning_text(w)))
        .collect::<Vec<_>>()
        .join("")
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
    fn parse_abono_imperative_form() {
        match parse_action("abónale 5000 a doña ana") {
            ActionParse::Abono {
                customer_name,
                amount,
            } => {
                // normalize() folds accents; the honorific is dropped so the
                // fuzzy search still finds "Ana Pérez".
                assert_eq!(customer_name, "ana");
                assert_eq!(amount, dec("5000"));
            }
            other => panic!("expected Abono, got {other:?}"),
        }
        assert!(matches!(
            parse_action("registra un abono de 10000 de juan soto"),
            ActionParse::Abono { .. }
        ));
    }

    #[test]
    fn parse_abono_me_pago_form() {
        match parse_action("doña ana me pagó 3000") {
            ActionParse::Abono {
                customer_name,
                amount,
            } => {
                assert_eq!(customer_name, "ana");
                assert_eq!(amount, dec("3000"));
            }
            other => panic!("expected Abono, got {other:?}"),
        }
    }

    #[test]
    fn parse_abono_missing_fields_is_incomplete() {
        // No amount, and no customer.
        assert!(matches!(
            parse_action("abónale a doña ana"),
            ActionParse::Incomplete(_)
        ));
        assert!(matches!(
            parse_action("abona 5000"),
            ActionParse::Incomplete(_)
        ));
    }

    #[test]
    fn abono_does_not_steal_read_or_product_intents() {
        // PorCobrar (read) carries no abono cue → read path.
        assert_eq!(parse_action("cuánto me deben"), ActionParse::NotAnAction);
        assert_eq!(parse_action("quién me debe"), ActionParse::NotAnAction);
        // "pago" alone belongs to other intents (VentasPorMetodo / IvaMes).
        assert_eq!(
            parse_action("ventas por método de pago"),
            ActionParse::NotAnAction
        );
        assert_eq!(
            parse_action("cuánto iva pago este mes"),
            ActionParse::NotAnAction
        );
        // "jabón" contains "abon" but is not a payment.
        assert!(matches!(
            parse_action("crea un producto jabón a $1000"),
            ActionParse::Producto { .. }
        ));
        // Neither is fertilizer called "abono".
        assert!(matches!(
            parse_action("crea un producto abono a $2500"),
            ActionParse::Producto { .. }
        ));
    }

    #[test]
    fn abono_summary_is_es_cl_prose() {
        let a = Action::RegistrarAbono {
            customer_id: "customer:x".into(),
            customer_name: "Ana Pérez".into(),
            amount: dec("5000"),
            debt_before: dec("12000"),
        };
        assert_eq!(a.label(), "registrar_abono");
        assert!(
            a.summary(&Money::default()).contains("Registrar un abono de"),
            "got {}",
            a.summary(&Money::default())
        );
        assert!(a.summary(&Money::default()).contains("Ana Pérez"));
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
        let p = store.propose(action.clone(), &tenant(), &Money::default());
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
        let p = store.propose(action, &tenant(), &Money::default());
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
        let p = store.propose_with_ttl(action, &tenant(), &Money::default(), -1);
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

    // ---- venta: parsing ------------------------------------------------------

    /// Assert `q` parses as a sale and return its parts.
    fn venta(q: &str) -> (Vec<VentaLineaParse>, Option<String>, bool) {
        match parse_action(q) {
            ActionParse::Venta {
                lines,
                customer_name,
                fiado,
            } => (lines, customer_name, fiado),
            other => panic!("expected Venta for {q:?}, got {other:?}"),
        }
    }

    fn linea(name: &str, qty: i64) -> VentaLineaParse {
        VentaLineaParse {
            product_name: name.into(),
            quantity: qty,
            unit_price: None,
        }
    }

    /// The four phrases the owner actually says (ADR-0016 wave "el agente
    /// vende"), verbatim.
    #[test]
    fn parse_venta_las_cuatro_frases() {
        let (lines, cliente, fiado) = venta("vendeme 2 paracetamol");
        assert_eq!(lines, vec![linea("paracetamol", 2)]);
        assert_eq!(cliente, None);
        assert!(!fiado);

        let (lines, cliente, fiado) = venta("cobrale 3 ibuprofeno a la senora Perez");
        assert_eq!(lines, vec![linea("ibuprofeno", 3)]);
        assert_eq!(cliente.as_deref(), Some("Perez"));
        assert!(!fiado);

        let (lines, cliente, fiado) = venta("anota 1 alcohol gel fiado a Juan");
        assert_eq!(lines, vec![linea("alcohol gel", 1)]);
        assert_eq!(cliente.as_deref(), Some("Juan"));
        assert!(fiado, "«fiado» debe mandar la venta a la cuenta corriente");

        // "2 x 500" = dos del de 500; "al tiro" es muletilla, no un cliente.
        let (lines, cliente, fiado) = venta("vende 2 x 500 al tiro");
        assert_eq!(lines, vec![linea("500", 2)]);
        assert_eq!(cliente, None);
        assert!(!fiado);
    }

    #[test]
    fn parse_venta_sin_acentos_y_con_typos() {
        for q in [
            "véndeme 2 paracetamol",
            "vendeme 2 paracetamol",
            "bendeme 2 paracetamol", // b/v, el typo chileno de siempre
            "vendme 2 paracetamol",  // una letra comida
            "véndele 2 paracetamol",
            "despachame 2 paracetamol",
            "vendeme dos paracetamol", // número hablado
        ] {
            let (lines, _, fiado) = venta(q);
            assert_eq!(lines, vec![linea("paracetamol", 2)], "q={q}");
            assert!(!fiado, "q={q}");
        }
        // Sin cantidad = uno.
        assert_eq!(venta("vendeme paracetamol").0, vec![linea("paracetamol", 1)]);
    }

    #[test]
    fn parse_venta_fiado_en_todas_sus_formas() {
        for q in [
            "anota 1 alcohol gel fiado a Juan",
            "fiale 1 alcohol gel a Juan",
            "fíale un alcohol gel a Juan",
            "vendele 1 alcohol gel fiado a Juan",
            "registra 1 alcohol gel a cuenta de Juan",
            "anota 1 alcohol gel en la libreta de Juan",
        ] {
            let (lines, cliente, fiado) = venta(q);
            assert_eq!(lines, vec![linea("alcohol gel", 1)], "q={q}");
            assert_eq!(cliente.as_deref(), Some("Juan"), "q={q}");
            assert!(fiado, "q={q}");
        }
    }

    #[test]
    fn parse_venta_cliente_con_y_sin_honorifico() {
        for (q, esperado) in [
            ("cobrale 3 ibuprofeno a la señora Pérez", "Perez"),
            ("cóbrale 3 ibuprofeno al señor Pérez", "Perez"),
            ("cobrale 3 ibuprofeno a don Juan", "Juan"),
            ("vendele 3 ibuprofeno a Juan", "Juan"),
        ] {
            let (lines, cliente, _) = venta(q);
            assert_eq!(lines, vec![linea("ibuprofeno", 3)], "q={q}");
            assert_eq!(cliente.as_deref(), Some(esperado), "q={q}");
        }
    }

    /// Un " a " en minúscula dentro del nombre del producto NO es un cliente:
    /// "arroz a granel" se vende, no se le fía a «Granel». La unidad se limpia
    /// (feria: granel/kg no son parte del nombre de catálogo).
    #[test]
    fn parse_venta_no_confunde_producto_con_cliente() {
        let (lines, cliente, _) = venta("vendeme 2 arroz a granel");
        assert_eq!(lines, vec![linea("arroz", 2)]);
        assert_eq!(cliente, None);
    }

    #[test]
    fn parse_venta_feria_con_precio_dicho() {
        let (lines, _, _) = venta("vendeme 1 tomates a 2000");
        assert_eq!(lines[0].product_name, "tomates");
        assert_eq!(lines[0].quantity, 1);
        assert_eq!(lines[0].unit_price, Some(dec("2000")));
        let (lines, _, _) = venta("vendí 2 kg de tomates a $2.000");
        assert_eq!(lines[0].product_name, "tomates");
        assert_eq!(lines[0].quantity, 2);
        assert_eq!(lines[0].unit_price, Some(dec("2000")));
    }

    /// Feria / calle (ADR-0022): kg, atado, bolsa se despegan del producto.
    #[test]
    fn parse_venta_unidades_feria_kg_atado() {
        let (lines, cliente, fiado) = venta("vendeme 2 kg de tomates");
        assert_eq!(lines, vec![linea("tomates", 2)]);
        let (lines2, _, _) = venta("vendeme 1 bandeja de frutillas");
        assert_eq!(lines2, vec![linea("frutillas", 1)]);
        let (lines3, _, _) = venta("vendeme 1 docena de huevos");
        assert_eq!(lines3, vec![linea("huevos", 1)]);
        assert_eq!(cliente, None);
        assert!(!fiado);

        let (lines, _, _) = venta("vendeme 1 atado de cilantro");
        assert_eq!(lines, vec![linea("cilantro", 1)]);

        let (lines, _, _) = venta("vendeme 3 kilos de papa");
        assert_eq!(lines, vec![linea("papa", 3)]);

        // Fiado feria: unidad + persona.
        let (lines, cliente, fiado) = venta("anota 1 atado de cilantro fiado a doña Ana");
        assert_eq!(lines, vec![linea("cilantro", 1)]);
        assert_eq!(cliente.as_deref(), Some("Ana"));
        assert!(fiado);
    }

    /// Cuaderno: "Don Juan debe 5000" enseña, no inventa un SKU.
    /// El ejemplo debe traer precio (`a 2000`) para que la siguiente frase no pida precio otra vez.
    #[test]
    fn parse_deuda_feria_nudge_pide_producto() {
        match parse_action("Don Juan debe 5000") {
            ActionParse::Incomplete(msg) => {
                assert!(msg.contains("Juan") || msg.contains("juan"), "{msg}");
                assert!(msg.contains("fiado"), "{msg}");
                assert!(msg.contains("a 2000"), "{msg}");
            }
            other => panic!("expected Incomplete, got {other:?}"),
        }
        // Forma ASI_SE_FIA antigua: Incomplete con precio en el ejemplo, sin inventar SKU.
        match parse_action("Don Juan me debe 5000") {
            ActionParse::Incomplete(msg) => {
                assert!(msg.contains("a 2000"), "{msg}");
                assert!(msg.contains("fiado"), "{msg}");
            }
            other => panic!("expected Incomplete, got {other:?}"),
        }
        // Preguntas de lectura no se roban.
        assert_eq!(parse_action("¿quién me debe?"), ActionParse::NotAnAction);
    }

    #[test]
    fn parse_venta_varias_lineas() {
        let (lines, _, _) = venta("vendeme 2 paracetamol y 1 ibuprofeno");
        assert_eq!(lines, vec![linea("paracetamol", 2), linea("ibuprofeno", 1)]);
        // Un "y" DENTRO de un nombre no parte la línea (el segundo trozo no
        // empieza con cantidad).
        let (lines, _, _) = venta("vendeme 2 alcohol gel y jabon");
        assert_eq!(lines, vec![linea("alcohol gel y jabon", 2)]);
    }

    #[test]
    fn parse_venta_incompleta_pide_lo_que_falta() {
        // Verbo de venta sin nada que vender.
        assert!(matches!(
            parse_action("vendeme"),
            ActionParse::Incomplete(_)
        ));
        // Fiado sin cliente: nunca se fía "al aire".
        match parse_action("anota 2 paracetamol fiado") {
            ActionParse::Incomplete(msg) => assert!(msg.contains("quién"), "msg={msg}"),
            other => panic!("expected Incomplete, got {other:?}"),
        }
        // Una cantidad absurda se pregunta, no se propone.
        assert!(matches!(
            parse_action("vendeme 99999 paracetamol"),
            ActionParse::Incomplete(_)
        ));
    }

    /// The read agent owns every one of these. A write branch that steals a
    /// question is worse than one that misses it.
    #[test]
    fn parse_venta_no_le_roba_preguntas_al_agente_de_lectura() {
        for q in [
            "cuánto vendí hoy",
            "cuánto vendí este mes",
            "cuánto se vendió ayer",
            "los más vendidos",
            "productos más vendidos",
            "cuánto fiado tengo",
            "quién me debe",
            "cuentas por cobrar",
            "cuánto me deben",
            "qué tengo que reponer",
        ] {
            assert_eq!(parse_action(q), ActionParse::NotAnAction, "q={q}");
        }
    }

    /// The other write branches keep their vocabulary.
    #[test]
    fn parse_venta_no_colisiona_con_las_otras_acciones() {
        assert!(matches!(
            parse_action("registra un gasto de 5000 en arriendo"),
            ActionParse::Gasto { .. }
        ));
        assert!(matches!(
            parse_action("crea un producto Aspirina a $1000"),
            ActionParse::Producto { .. }
        ));
        assert!(matches!(
            parse_action("crea un cliente Juan Pérez"),
            ActionParse::Cliente { .. }
        ));
        assert!(matches!(
            parse_action("abónale 5000 a doña Ana"),
            ActionParse::Abono { .. }
        ));
        assert!(matches!(
            parse_action("crea una orden de compra de 10 paracetamol a Farmaltda a $500"),
            ActionParse::Oc { .. }
        ));
        assert!(matches!(
            parse_action("cierra la caja con 50000"),
            ActionParse::CierreCaja { .. }
        ));
        assert!(matches!(
            parse_action("repón 40 de paracetamol"),
            ActionParse::AjusteStock { .. }
        ));
    }

    // ---- venta: la propuesta que lee la dueña --------------------------------

    fn linea_resuelta(name: &str, qty: i64, price: &str) -> VentaLinea {
        VentaLinea {
            product_id: "product:x".into(),
            product_name: name.into(),
            quantity: qty,
            unit_price: dec(price),
        }
    }

    /// The confirmation prompt is the owner's only defence against a mis-heard
    /// order, so it must spell out product, quantity, unit price and total.
    #[test]
    fn summary_de_venta_muestra_todo_lo_que_hay_que_revisar() {
        let a = Action::Vender {
            lines: vec![
                linea_resuelta("Paracetamol 500 mg", 2, "990"),
                linea_resuelta("Ibuprofeno 400 mg", 1, "1200"),
            ],
            subtotal: dec("3180"),
            total: dec("3180"),
            customer_id: None,
            customer_name: None,
            warnings: vec![],
            abre_puesto: false,
        };
        let s = a.summary(&Money::default());
        assert!(s.contains("2 × Paracetamol 500 mg"), "{s}");
        assert!(s.contains("$990 c/u"), "{s}");
        assert!(s.contains("= $1.980"), "{s}");
        assert!(s.contains("1 × Ibuprofeno 400 mg"), "{s}");
        assert!(s.contains("Total a cobrar: $3.180"), "{s}");
    }

    #[test]
    fn summary_de_fiado_dice_a_quien_y_cuanto_debe() {
        let a = Action::FiarVenta {
            lines: vec![linea_resuelta("Alcohol Gel", 1, "2500")],
            subtotal: dec("2500"),
            total: dec("2500"),
            customer_id: "customer:1".into(),
            customer_name: "Juan Pérez".into(),
            debt_before: dec("5000"),
            warnings: vec!["Warfarina + Ibuprofeno (riesgo crítico). Sangrado.".into()],
        };
        let s = a.summary(&Money::default());
        assert!(s.contains("1 × Alcohol Gel a $2.500 c/u"), "{s}");
        assert!(s.contains("Total: $2.500"), "{s}");
        assert!(s.contains("Queda fiado a Juan Pérez"), "{s}");
        assert!(s.contains("hoy debe $5.000"), "{s}");
        assert!(s.contains("Ojo: Warfarina + Ibuprofeno"), "{s}");
        assert_eq!(a.label(), "fiar_venta");
    }

    #[test]
    fn params_de_venta_exponen_las_lineas() {
        let a = Action::Vender {
            lines: vec![linea_resuelta("Paracetamol 500 mg", 2, "990")],
            subtotal: dec("1980"),
            total: dec("1980"),
            customer_id: Some("customer:1".into()),
            customer_name: Some("Juan".into()),
            warnings: vec![],
            abre_puesto: false,
        };
        let p = a.params();
        assert_eq!(p["total"], "1980");
        assert_eq!(p["payment_method"], "pos_cash");
        assert_eq!(p["lines"][0]["quantity"], 2);
        assert_eq!(p["lines"][0]["unit_price"], "990");
        assert_eq!(p["lines"][0]["line_total"], "1980");
        assert_eq!(p["customer_name"], "Juan");
    }

    /// The interaction checker is the domain's, not a copy living here.
    #[test]
    fn interacciones_se_leen_del_chequeo_del_dominio() {
        let w = interaction_warnings(&["Warfarina".to_string(), "Ibuprofeno".to_string()]);
        assert_eq!(w.len(), 1, "warfarina + AINE es una interacción conocida");
        assert!(w[0].contains("riesgo crítico"), "{}", w[0]);
        assert!(interaction_warnings(&["Paracetamol".to_string()]).is_empty());
    }

    #[test]
    fn edit_distance_1_no_se_pasa_de_generosa() {
        assert!(edit_distance_1("vendme", "vendeme"));
        assert!(edit_distance_1("vende", "vender"));
        assert!(!edit_distance_1("vendidos", "vender"));
        assert!(!edit_distance_1("vende", "vende"));
    }
}
