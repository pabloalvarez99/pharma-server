// Clientes view — customer lookup for the counter:
//   search box (name / rut / phone) → debounced results list → click a result →
//   detail panel (loyalty points + lifetime spend + visits) plus purchase
//   history.
//
// DEGRADES GRACEFULLY: the `/api/v1/customers/{search,{id},{id}/history}` surface
// ships on `feat/customers-loyalty-history`, which may NOT be merged into the
// server this client talks to. The Tauri commands reject with the sentinel
// `CUSTOMERS_MODULE_MISSING` on a 404; this view catches it and shows a friendly
// "módulo requiere merge de customers-loyalty" note instead of a hard error — it
// never blocks the rest of the app.
// Spanish throughout, CLP via ../format. Same skeleton → fetch → swap pattern.
import {
  customerSearch,
  customerDetail,
  customerHistory,
  createCustomer,
  updateCustomer,
  customerAccount,
  recordAbono,
  CUSTOMERS_MODULE_MISSING,
  type Customer,
  type CustomerDetail,
  type CustomerOrder,
  type CustomerAccount,
  type LedgerEntry,
} from "../api";
import { clp, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml, attachRutAdvisory } from "./view-blocks";
import { bindModalKeys } from "./modal-keys";
import "./rutbrand.css";

const HISTORY_LIMIT = 20;

export function renderClientes(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-clientes">
      <div class="view-head">
        <div>
          <h2>Clientes</h2>
          <p class="muted">Busca por nombre, RUT o teléfono y revisa su historial.</p>
        </div>
        <div class="cli-head-actions">
          <div class="view-search">
            <input id="cli-search" type="search" placeholder="Buscar cliente…" autocomplete="off" />
          </div>
          <button id="cli-new" type="button" class="btn-ghost">+ Nuevo cliente</button>
        </div>
      </div>

      <div id="cli-modal-host"></div>

      <div class="clientes-grid">
        <div class="table-card">
          <h3 class="section-title">Resultados</h3>
          <div id="cli-results"><p class="empty">Escribe para buscar un cliente.</p></div>
        </div>
        <div class="table-card">
          <h3 class="section-title">Detalle</h3>
          <div id="cli-detail"><p class="empty">Selecciona un cliente para ver su detalle.</p></div>
        </div>
      </div>
    </section>
  `;

  const searchEl = host.querySelector<HTMLInputElement>("#cli-search")!;
  const resultsEl = host.querySelector<HTMLElement>("#cli-results")!;
  const detailEl = host.querySelector<HTMLElement>("#cli-detail")!;
  const modalHost = host.querySelector<HTMLElement>("#cli-modal-host")!;
  const newBtn = host.querySelector<HTMLButtonElement>("#cli-new")!;

  let timer: number | undefined;
  const search = () => {
    const q = searchEl.value.trim();
    if (q === "") {
      resultsEl.innerHTML = `<p class="empty">Escribe para buscar un cliente.</p>`;
      return;
    }
    resultsEl.innerHTML = tableSkeleton(5);
    void runSearch(resultsEl, detailEl, serverUrl, q);
  };
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    if (searchEl.value.trim() === "") {
      resultsEl.innerHTML = `<p class="empty">Escribe para buscar un cliente.</p>`;
      return;
    }
    timer = window.setTimeout(search, 240);
  });

  // Create: open an empty form. On save, re-run the current search so the new
  // customer shows up.
  newBtn.addEventListener("click", () => {
    openCustomerModal(modalHost, serverUrl, null, (c) => {
      searchEl.value = c.name;
      search();
    });
  });

  // Edit: the detail panel renders a button carrying the customer fields as data
  // attrs (delegated here so it survives detail re-renders). On save, reload the
  // detail panel for that id.
  detailEl.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(".cli-edit");
    if (!btn) return;
    const existing: Customer = {
      id: btn.dataset.id ?? "",
      name: btn.dataset.name ?? "",
      rut: btn.dataset.rut || null,
      phone: btn.dataset.phone || null,
      email: btn.dataset.email || null,
      loyalty_points: 0,
      active: btn.dataset.active === "true",
      created_at: "",
      updated_at: "",
    };
    openCustomerModal(modalHost, serverUrl, existing, (c) => {
      detailEl.innerHTML = tableSkeleton(5);
      void loadDetail(detailEl, serverUrl, c.id);
    });
  });

  // Abono (fiado): el panel de cuenta corriente rinde un form con el id del
  // cliente. Delegado acá para que sobreviva los re-render del detalle. En éxito
  // recarga la ficha (saldo + movimientos al día).
  detailEl.addEventListener("submit", (e) => {
    const form = (e.target as HTMLElement).closest<HTMLFormElement>(".cli-abono-form");
    if (!form) return;
    e.preventDefault();
    const id = form.dataset.id ?? "";
    const input = form.querySelector<HTMLInputElement>('input[name="amount"]');
    const msg = detailEl.querySelector<HTMLElement>(".cli-abono-msg");
    const raw = (input?.value ?? "").replace(/[^\d]/g, "");
    const showMsg = (text: string, ok: boolean): void => {
      if (!msg) return;
      msg.textContent = text;
      msg.className = `cli-abono-msg ${ok ? "is-ok" : "is-err"}`;
      msg.hidden = false;
    };
    if (!raw || Number(raw) <= 0) {
      showMsg("Ingresa un monto mayor a cero.", false);
      input?.focus();
      return;
    }
    const btn = form.querySelector<HTMLButtonElement>('button[type="submit"]');
    if (btn) btn.disabled = true;
    void recordAbono(serverUrl, id, raw)
      .then(() => {
        detailEl.innerHTML = tableSkeleton(5);
        return loadDetail(detailEl, serverUrl, id);
      })
      .catch((err) => {
        if (btn) btn.disabled = false;
        showMsg(asMessage(err), false);
      });
  });
}

/** Create/edit customer modal. `existing` null ⇒ create; otherwise edit (PATCH
 *  only the changed fields). `onSaved` gets the server's returned customer. */
function openCustomerModal(
  modalHost: HTMLElement,
  serverUrl: string,
  existing: Customer | null,
  onSaved: (c: Customer) => void,
): void {
  const isEdit = existing !== null;
  const val = (s: string | null | undefined) => (s ? escapeHtml(s) : "");
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="cli-modal-backdrop">
      <div class="modal">
        <div class="modal-title">${isEdit ? "Editar cliente" : "Nuevo cliente"}</div>
        <div class="modal-field">
          <label class="modal-label" for="cli-f-name">Nombre</label>
          <input id="cli-f-name" type="text" value="${val(existing?.name)}" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="cli-f-rut">RUT</label>
          <input id="cli-f-rut" type="text" value="${val(existing?.rut)}" autocomplete="off" />
          <div id="cli-f-rut-hint" class="field-hint" hidden></div>
        </div>
        <div class="modal-field">
          <label class="modal-label" for="cli-f-phone">Teléfono</label>
          <input id="cli-f-phone" type="text" value="${val(existing?.phone)}" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="cli-f-email">Email</label>
          <input id="cli-f-email" type="email" value="${val(existing?.email)}" autocomplete="off" />
        </div>
        <div id="cli-f-error" class="form-error" hidden></div>
        <div class="modal-actions">
          <button type="button" class="btn-ghost" id="cli-f-cancel">Cancelar</button>
          <button type="button" class="btn-primary modal-confirm" id="cli-f-save">
            <span class="btn-label">${isEdit ? "Guardar" : "Crear"}</span>
          </button>
        </div>
      </div>
    </div>
  `;

  // Escape closes the form so a mouseless operator can back out (Enter is left
  // free — a multi-field form shouldn't submit on a stray Enter mid-typing).
  const detachKeys = bindModalKeys(() => close());
  const close = () => {
    detachKeys();
    modalHost.innerHTML = "";
  };
  const nameEl = modalHost.querySelector<HTMLInputElement>("#cli-f-name")!;
  const rutEl = modalHost.querySelector<HTMLInputElement>("#cli-f-rut")!;
  const phoneEl = modalHost.querySelector<HTMLInputElement>("#cli-f-phone")!;
  const emailEl = modalHost.querySelector<HTMLInputElement>("#cli-f-email")!;
  const errEl = modalHost.querySelector<HTMLElement>("#cli-f-error")!;
  const saveBtn = modalHost.querySelector<HTMLButtonElement>("#cli-f-save")!;
  const rutAdvice = attachRutAdvisory(rutEl, modalHost.querySelector<HTMLElement>("#cli-f-rut-hint")!);

  modalHost.querySelector<HTMLButtonElement>("#cli-f-cancel")!.addEventListener("click", close);
  modalHost.querySelector<HTMLElement>("#cli-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  nameEl.focus();

  saveBtn.addEventListener("click", async () => {
    const name = nameEl.value.trim();
    const rut = rutAdvice.canonical();
    const phone = phoneEl.value.trim();
    const email = emailEl.value.trim();
    if (name === "") {
      errEl.hidden = false;
      errEl.textContent = "El nombre es obligatorio.";
      return;
    }
    errEl.hidden = true;
    saveBtn.classList.add("loading");
    saveBtn.disabled = true;
    try {
      const saved = isEdit
        ? await updateCustomer(serverUrl, existing!.id, {
            name,
            rut: rut || undefined,
            phone: phone || undefined,
            email: email || undefined,
          })
        : await createCustomer(serverUrl, name, rut || undefined, phone || undefined, email || undefined);
      close();
      onSaved(saved);
    } catch (err) {
      saveBtn.classList.remove("loading");
      saveBtn.disabled = false;
      errEl.hidden = false;
      errEl.textContent =
        err === CUSTOMERS_MODULE_MISSING
          ? "El módulo de clientes no está disponible en este servidor."
          : asMessage(err);
    }
  });
}

async function runSearch(
  resultsEl: HTMLElement,
  detailEl: HTMLElement,
  serverUrl: string,
  q: string,
): Promise<void> {
  try {
    const rows: Customer[] = await customerSearch(serverUrl, q);
    if (rows.length === 0) {
      resultsEl.innerHTML = `<p class="empty">Sin clientes para «${escapeHtml(q)}».</p>`;
      return;
    }
    resultsEl.innerHTML = rows.map(resultRow).join("");
    resultsEl.querySelectorAll<HTMLButtonElement>(".cli-result").forEach((b, i) => {
      b.addEventListener("click", () => {
        resultsEl
          .querySelectorAll(".cli-result")
          .forEach((x) => x.classList.remove("active"));
        b.classList.add("active");
        detailEl.innerHTML = tableSkeleton(5);
        void loadDetail(detailEl, serverUrl, rows[i].id);
      });
    });
  } catch (err) {
    resultsEl.innerHTML = renderError(err);
  }
}

function resultRow(c: Customer): string {
  const sub = [c.rut, c.phone].filter(Boolean).join(" · ");
  return `
    <button type="button" class="cli-result" data-id="${escapeHtml(c.id)}">
      <div class="cli-result-info">
        <div class="cell-main">${escapeHtml(c.name)}</div>
        ${sub ? `<div class="cell-sub muted rb-num">${escapeHtml(sub)}</div>` : ""}
      </div>
      <div class="cli-points rb-num">${num(c.loyalty_points)} pts</div>
    </button>
  `;
}

async function loadDetail(
  detailEl: HTMLElement,
  serverUrl: string,
  id: string,
): Promise<void> {
  try {
    const [detail, history, account] = await Promise.all([
      customerDetail(serverUrl, id),
      customerHistory(serverUrl, id, HISTORY_LIMIT).catch((err) => {
        // History is secondary — a missing module sentinel here would already
        // have surfaced via the detail call. Treat other failures as empty.
        if (err === CUSTOMERS_MODULE_MISSING) throw err;
        return [] as CustomerOrder[];
      }),
      // Cuenta corriente (fiado). Secundaria: un server sin la migración 0039
      // simplemente no muestra el bloque en vez de romper la ficha.
      customerAccount(serverUrl, id).catch(() => null),
    ]);
    detailEl.innerHTML = renderDetail(detail, history, account);
  } catch (err) {
    detailEl.innerHTML = renderError(err);
  }
}

/** Cuenta corriente (fiado): saldo + abonar + movimientos. `null` cuando el
 *  server no expone el módulo (no rompe la ficha del cliente). Todo lo
 *  interpolado pasa por `escapeHtml`/`clp` como el resto de la vista. */
function renderAccount(c: CustomerDetail, account: CustomerAccount | null): string {
  if (!account) return "";
  const debt = Number(account.balance);
  const owes = Number.isFinite(debt) && debt > 0;

  const abonoForm = owes
    ? `
      <form class="cli-abono-form" data-id="${escapeHtml(c.id)}">
        <label class="sr-only" for="cli-abono">Monto del abono</label>
        <input id="cli-abono" name="amount" type="text" inputmode="numeric"
               placeholder="Monto del abono" autocomplete="off" required />
        <button type="submit" class="btn-primary">Registrar abono</button>
      </form>
      <p class="cli-abono-msg" role="status" hidden></p>`
    : `<p class="empty">Sin deuda pendiente.</p>`;

  const rows =
    account.entries.length === 0
      ? ""
      : `
        <table class="data-table">
          <thead><tr><th>Fecha</th><th>Movimiento</th><th class="num">Monto</th></tr></thead>
          <tbody>${account.entries.map(ledgerRow).join("")}</tbody>
        </table>`;

  return `
    <h3 class="section-title cli-cuenta-title">Cuenta corriente</h3>
    <div class="cli-stats">
      <div class="cli-stat">
        <span class="kpi-label">Saldo (debe)</span>
        <strong class="rb-num ${owes ? "is-debt" : ""}">${clp(account.balance)}</strong>
      </div>
      <div class="cli-stat"><span class="kpi-label">Total fiado</span><strong class="rb-num">${clp(account.total_charged)}</strong></div>
      <div class="cli-stat"><span class="kpi-label">Total abonado</span><strong class="rb-num">${clp(account.total_paid)}</strong></div>
    </div>
    ${abonoForm}
    ${rows}
  `;
}

function ledgerRow(e: LedgerEntry): string {
  const isCargo = e.kind === "cargo";
  return `
    <tr>
      <td>${fmtDate(e.created_at)}</td>
      <td><span class="pill ${isCargo ? "pill-warn" : "pill-ok"}">${isCargo ? "Fiado" : "Abono"}</span>${
        e.note ? ` <span class="cell-sub muted">${escapeHtml(e.note)}</span>` : ""
      }</td>
      <td class="num rb-num">${isCargo ? "" : "−"}${clp(e.amount)}</td>
    </tr>
  `;
}

function renderDetail(
  c: CustomerDetail,
  history: CustomerOrder[],
  account: CustomerAccount | null = null,
): string {
  const contact = [
    c.rut ? `RUT <span class="rb-num">${escapeHtml(c.rut)}</span>` : "",
    c.phone ? escapeHtml(c.phone) : "",
    c.email ? escapeHtml(c.email) : "",
  ]
    .filter(Boolean)
    .join(" · ");

  const editBtn = `
    <button type="button" class="btn-ghost cli-edit"
      data-id="${escapeHtml(c.id)}"
      data-name="${escapeHtml(c.name)}"
      data-rut="${escapeHtml(c.rut ?? "")}"
      data-phone="${escapeHtml(c.phone ?? "")}"
      data-email="${escapeHtml(c.email ?? "")}"
      data-active="${c.active}">Editar</button>`;

  const head = `
    <div class="cli-detail-head">
      <div class="cli-detail-name-row">
        <div class="cli-detail-name">${escapeHtml(c.name)}</div>
        ${editBtn}
      </div>
      ${contact ? `<div class="cell-sub muted">${contact}</div>` : ""}
      ${c.active ? "" : `<span class="pill pill-warn">Inactivo</span>`}
    </div>
    <div class="cli-stats">
      <div class="cli-stat"><span class="kpi-label">Puntos</span><strong class="rb-num">${num(c.loyalty_points)}</strong></div>
      <div class="cli-stat"><span class="kpi-label">Total comprado</span><strong class="rb-num">${clp(c.total_spent)}</strong></div>
      <div class="cli-stat"><span class="kpi-label">Visitas</span><strong class="rb-num">${num(c.visit_count)}</strong></div>
    </div>
  `;

  const hist =
    history.length === 0
      ? `<p class="empty">Sin compras registradas.</p>`
      : `
        <table class="data-table">
          <thead>
            <tr><th>Fecha</th><th>Pago</th><th class="num">Ítems</th><th class="num">Total</th><th>Estado</th></tr>
          </thead>
          <tbody>${history.map(historyRow).join("")}</tbody>
        </table>
      `;

  return `${head}${renderAccount(c, account)}<h3 class="section-title cli-hist-title">Historial de compras</h3>${hist}`;
}

function historyRow(o: CustomerOrder): string {
  const st = o.status.toLowerCase();
  const tone =
    st === "refunded" || st === "cancelled" ? "pill-danger" : st === "pending" ? "pill-warn" : "pill-ok";
  return `
    <tr>
      <td>${fmtDate(o.created_at)}</td>
      <td>${escapeHtml(payLabel(o.payment_method))}</td>
      <td class="num rb-num">${num(o.items_count)}</td>
      <td class="num rb-num">${clp(o.total)}</td>
      <td><span class="pill ${tone}">${escapeHtml(statusLabel(o.status))}</span></td>
    </tr>
  `;
}

// --- the graceful-degrade branch -------------------------------------------

function renderError(err: unknown): string {
  if (err === CUSTOMERS_MODULE_MISSING) {
    return `
      <div class="cli-missing">
        <div class="caja-empty-mark">●</div>
        <h3>Módulo de clientes no disponible</h3>
        <p class="muted">Este servidor aún no expone la búsqueda de clientes. Requiere el merge de <code>customers-loyalty</code> en el servidor.</p>
      </div>
    `;
  }
  return `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
}

// --- label helpers ---------------------------------------------------------

function payLabel(method: string): string {
  switch (method) {
    case "pos_cash":
      return "Efectivo";
    case "pos_debit":
      return "Débito";
    case "pos_credit":
      return "Crédito";
    case "pos_mixed":
      return "Mixto";
    default:
      return method;
  }
}

function statusLabel(status: string): string {
  switch (status.toLowerCase()) {
    case "paid":
    case "completed":
      return "Pagada";
    case "pending":
      return "Pendiente";
    case "refunded":
      return "Devuelta";
    case "cancelled":
      return "Anulada";
    default:
      return status;
  }
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-CL", { day: "2-digit", month: "2-digit", year: "2-digit" });
}
