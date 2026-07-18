// Compras view — read-only purchase-order list for counter staff (cashier+).
//   Status filter (Todas / Borrador / Enviadas / Recibidas / Canceladas) →
//   table of PO headers (proveedor, referencia, estado, total, fecha). The
//   list endpoint returns headers only (no line items) which is enough for
//   the counter to track incoming orders.
//
// DEGRADES GRACEFULLY: the purchasing module may not be accessible from a
// cashier role on some deployments; a 403 is shown as a friendly note rather
// than a hard error — the rest of the app is unaffected.
// Spanish throughout, CLP via ../format. Same skeleton → fetch → swap pattern.
import {
  listPurchaseOrders,
  listSuppliers,
  createSupplier,
  getPurchaseOrder,
  createPurchaseOrder,
  sendPurchaseOrder,
  receivePurchaseOrder,
  getPoPayments,
  createPoPayment,
  cashSessions,
  type PurchaseOrder,
  type PurchaseOrderDetail,
  type NewPurchaseOrderItem,
  type ReceiveLine,
  type Supplier,
  type PurchasePaymentSummary,
  type PurchasePayment,
} from "../api";
import { clp, num, toNumber, fecha } from "../format";
import {
  kpiSkeleton,
  tableSkeleton,
  asMessage,
  escapeHtml,
  attachRutAdvisory,
  emptyStateHtml,
  errorStateHtml,
} from "./view-blocks";
import {
  poIsReceivable,
  poIsSendable,
  poStatusMeta,
  poKpis,
  poPending,
  parsePoLines,
  validateReceiveQty,
  comprasEmpty,
  type PoLineDraft,
} from "./stock-helpers";

const PAGE_LIMIT = 60;

const STATUS_OPTS: { value: string; label: string }[] = [
  { value: "", label: "Todas" },
  { value: "draft", label: "Borrador" },
  { value: "sent", label: "Enviadas" },
  { value: "approved", label: "Aprobadas" },
  { value: "received", label: "Recibidas" },
  // The server status is `partially_received` — the old `partial` matched
  // nothing, so the "Parciales" filter returned an empty list every time.
  { value: "partially_received", label: "Parciales" },
  { value: "cancelled", label: "Canceladas" },
];

export function renderCompras(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-compras">
      <div class="view-head">
        <div>
          <h2>Compras</h2>
          <p class="muted">Órdenes de compra a proveedores.</p>
        </div>
        <div class="compras-filters">
          <select id="compras-status" class="view-select">
            ${STATUS_OPTS.map(
              (o) => `<option value="${escapeHtml(o.value)}">${escapeHtml(o.label)}</option>`,
            ).join("")}
          </select>
          <button id="po-new" type="button" class="btn-ghost">+ Nueva OC</button>
        </div>
      </div>

      <div id="compras-kpis" class="kpi-grid">${kpiSkeleton(3)}</div>

      <div class="table-card">
        <div id="compras-table">${tableSkeleton()}</div>
      </div>

      <div class="table-card">
        <div class="view-head">
          <h3 class="section-title">Proveedores</h3>
          <button id="prov-new" type="button" class="btn-ghost">+ Nuevo proveedor</button>
        </div>
        <div id="prov-list">${tableSkeleton(4)}</div>
      </div>

      <div id="compras-modal-host"></div>
    </section>
  `;

  const kpiHost = host.querySelector<HTMLElement>("#compras-kpis")!;
  const tableHost = host.querySelector<HTMLElement>("#compras-table")!;
  const statusEl = host.querySelector<HTMLSelectElement>("#compras-status")!;
  const provHost = host.querySelector<HTMLElement>("#prov-list")!;
  const modalHost = host.querySelector<HTMLElement>("#compras-modal-host")!;

  async function reload(): Promise<void> {
    tableHost.innerHTML = tableSkeleton();
    kpiHost.innerHTML = kpiSkeleton(3);
    const status = statusEl.value || undefined;
    try {
      const rows = await listPurchaseOrders(serverUrl, status, PAGE_LIMIT);
      renderKpis(kpiHost, rows);
      renderTable(tableHost, rows, status !== undefined);
    } catch (err) {
      kpiHost.innerHTML = "";
      tableHost.innerHTML = renderError(err);
    }
  }

  async function reloadSuppliers(): Promise<void> {
    provHost.innerHTML = tableSkeleton(4);
    try {
      const rows = await listSuppliers(serverUrl, undefined, PAGE_LIMIT);
      renderSuppliers(provHost, rows);
    } catch (err) {
      provHost.innerHTML = renderError(err);
    }
  }

  statusEl.addEventListener("change", () => void reload());
  host.querySelector<HTMLButtonElement>("#prov-new")!.addEventListener("click", () => {
    openSupplierModal(modalHost, serverUrl, () => void reloadSuppliers());
  });
  host.querySelector<HTMLButtonElement>("#po-new")!.addEventListener("click", () => {
    openPoCreateModal(modalHost, serverUrl, () => void reload());
  });
  // Click a PO row → detail drawer (with receive action when applicable).
  tableHost.addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>(".po-row");
    if (!row?.dataset.id) return;
    openPoDetailModal(modalHost, serverUrl, row.dataset.id, () => void reload());
  });
  void reload();
  void reloadSuppliers();
}

function renderSuppliers(host: HTMLElement, rows: Supplier[]): void {
  if (rows.length === 0) {
    host.innerHTML = `<p class="empty">Sin proveedores registrados. Crea el primero con «Nuevo proveedor».</p>`;
    return;
  }
  host.innerHTML = `
    <table class="data-table">
      <thead>
        <tr><th>Proveedor</th><th>RUT</th><th>Contacto</th><th>Estado</th></tr>
      </thead>
      <tbody>${rows.map(supplierRow).join("")}</tbody>
    </table>
    <p class="table-foot muted">${rows.length} proveedor(es)</p>
  `;
}

function supplierRow(s: Supplier): string {
  const contact = [s.contact_name, s.contact_phone, s.contact_email].filter(Boolean).join(" · ");
  const estado = s.active
    ? `<span class="pill pill-ok">Activo</span>`
    : `<span class="pill pill-warn">Inactivo</span>`;
  return `
    <tr>
      <td><div class="cell-main">${escapeHtml(s.name)}</div></td>
      <td><span class="muted">${escapeHtml(s.rut ?? "—")}</span></td>
      <td><div class="cell-sub muted">${contact ? escapeHtml(contact) : "—"}</div></td>
      <td>${estado}</td>
    </tr>
  `;
}

/** "Nuevo proveedor" modal — name required, contact fields optional. Reuses the
 *  shared `.modal` chrome. `onSaved` fires after a successful create. */
function openSupplierModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onSaved: () => void,
): void {
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="prov-modal-backdrop">
      <div class="modal">
        <div class="modal-title">Nuevo proveedor</div>
        <div class="modal-field">
          <label class="modal-label" for="prov-f-name">Nombre</label>
          <input id="prov-f-name" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="prov-f-rut">RUT</label>
          <input id="prov-f-rut" type="text" autocomplete="off" />
          <div id="prov-f-rut-hint" class="field-hint" hidden></div>
        </div>
        <div class="modal-field">
          <label class="modal-label" for="prov-f-contact">Contacto</label>
          <input id="prov-f-contact" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="prov-f-phone">Teléfono</label>
          <input id="prov-f-phone" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="prov-f-email">Email</label>
          <input id="prov-f-email" type="email" autocomplete="off" />
        </div>
        <div id="prov-f-error" class="form-error" hidden></div>
        <div class="modal-actions">
          <button type="button" class="btn-ghost" id="prov-f-cancel">Cancelar</button>
          <button type="button" class="btn-primary modal-confirm" id="prov-f-save">
            <span class="btn-label">Crear</span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const nameEl = modalHost.querySelector<HTMLInputElement>("#prov-f-name")!;
  const rutEl = modalHost.querySelector<HTMLInputElement>("#prov-f-rut")!;
  const contactEl = modalHost.querySelector<HTMLInputElement>("#prov-f-contact")!;
  const phoneEl = modalHost.querySelector<HTMLInputElement>("#prov-f-phone")!;
  const emailEl = modalHost.querySelector<HTMLInputElement>("#prov-f-email")!;
  const errEl = modalHost.querySelector<HTMLElement>("#prov-f-error")!;
  const saveBtn = modalHost.querySelector<HTMLButtonElement>("#prov-f-save")!;
  const rutAdvice = attachRutAdvisory(rutEl, modalHost.querySelector<HTMLElement>("#prov-f-rut-hint")!);

  modalHost.querySelector<HTMLButtonElement>("#prov-f-cancel")!.addEventListener("click", close);
  modalHost.querySelector<HTMLElement>("#prov-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  nameEl.focus();

  saveBtn.addEventListener("click", async () => {
    const name = nameEl.value.trim();
    if (name === "") {
      errEl.hidden = false;
      errEl.textContent = "El nombre es obligatorio.";
      return;
    }
    errEl.hidden = true;
    saveBtn.classList.add("loading");
    saveBtn.disabled = true;
    try {
      await createSupplier(
        serverUrl,
        name,
        rutAdvice.canonical() || undefined,
        contactEl.value.trim() || undefined,
        emailEl.value.trim() || undefined,
        phoneEl.value.trim() || undefined,
      );
      close();
      onSaved();
    } catch (err) {
      saveBtn.classList.remove("loading");
      saveBtn.disabled = false;
      errEl.hidden = false;
      errEl.textContent = asMessage(err);
    }
  });
}

function renderKpis(host: HTMLElement, rows: PurchaseOrder[]): void {
  // Open = draft·sent·approved·partially_received (a partially-received OC still
  // has goods incoming). The old set missed approved + partially_received and
  // used the phantom `partial` status → the count under-reported pending orders.
  const { total, pending, totalValue } = poKpis(rows);
  host.innerHTML = `
    <div class="kpi-card">
      <span class="kpi-label">Órdenes</span>
      <strong class="kpi-value">${num(total)}</strong>
      <span class="kpi-sub muted">en el período visible</span>
    </div>
    <div class="kpi-card ${pending > 0 ? "kpi-warn" : ""}">
      <span class="kpi-label">Pendientes</span>
      <strong class="kpi-value">${num(pending)}</strong>
      <span class="kpi-sub muted">borrador · enviadas · parciales</span>
    </div>
    <div class="kpi-card">
      <span class="kpi-label">Valor total</span>
      <strong class="kpi-value">${clp(totalValue)}</strong>
      <span class="kpi-sub muted">suma de órdenes filtradas</span>
    </div>
  `;
}

function renderTable(host: HTMLElement, rows: PurchaseOrder[], filtered: boolean): void {
  if (rows.length === 0) {
    host.innerHTML = emptyStateHtml(comprasEmpty(filtered), filtered ? undefined : "po-empty-new");
    host
      .querySelector<HTMLButtonElement>("#po-empty-new")
      ?.addEventListener("click", () =>
        host.closest(".view-compras")?.querySelector<HTMLButtonElement>("#po-new")?.click(),
      );
    return;
  }

  host.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>Proveedor</th>
          <th>Referencia</th>
          <th>Moneda</th>
          <th class="num">Total</th>
          <th>Estado</th>
          <th>Fecha</th>
        </tr>
      </thead>
      <tbody>
        ${rows.map(poRow).join("")}
      </tbody>
    </table>
    <p class="table-foot muted">${rows.length} orden(es)${
      rows.length === PAGE_LIMIT ? ` · mostrando los primeros ${PAGE_LIMIT}` : ""
    }</p>
  `;
}

function poRow(po: PurchaseOrder): string {
  const statusBadge = statusPill(po.status);
  const date = fecha(po.created_at);
  const ref = po.external_ref || po.notes?.slice(0, 28) || "—";
  const supplierShort = po.supplier.replace(/^supplier:/, "");
  return `
    <tr class="po-row" data-id="${escapeHtml(po.id)}" title="Ver detalle">
      <td><div class="cell-main">${escapeHtml(supplierShort)}</div></td>
      <td><div class="cell-sub muted">${escapeHtml(ref)}</div></td>
      <td><span class="muted">${escapeHtml(po.currency)}</span></td>
      <td class="num">${clp(po.total)}</td>
      <td>${statusBadge}</td>
      <td><span class="muted">${escapeHtml(date)}</span></td>
    </tr>
  `;
}

function statusPill(status: string): string {
  // Centralised es-CL labels + tones. Adds the `approved` + `partially_received`
  // cases the old switch was missing (a real PO in those states used to leak the
  // raw English status to the operator via the `default` branch).
  const { label, tone } = poStatusMeta(status);
  return `<span class="pill pill-${tone}">${escapeHtml(label)}</span>`;
}

// --- Nueva OC (create) modal -----------------------------------------------

/** Multi-line purchase-order create modal. Loads suppliers for the picker, then
 *  renders a dynamic list of line rows (producto · cantidad · costo unit.).
 *  Lines are off-catalog free text (product id omitted); the server computes
 *  subtotals + total. `onSaved` fires after a successful create. */
function openPoCreateModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onSaved: () => void,
): void {
  const close = () => (modalHost.innerHTML = "");
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="po-modal-backdrop">
      <div class="modal modal-wide">
        <div class="modal-title">Nueva orden de compra</div>
        <div id="po-c-body">${tableSkeleton(3)}</div>
      </div>
    </div>
  `;
  modalHost.querySelector<HTMLElement>("#po-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });

  const body = modalHost.querySelector<HTMLElement>("#po-c-body")!;
  void (async () => {
    let suppliers: Supplier[];
    try {
      suppliers = await listSuppliers(serverUrl, undefined, 200);
    } catch (err) {
      body.innerHTML = `${renderError(err)}<div class="modal-actions"><button type="button" class="btn-ghost" id="po-c-cancel">Cerrar</button></div>`;
      body.querySelector<HTMLButtonElement>("#po-c-cancel")!.addEventListener("click", close);
      return;
    }
    if (suppliers.length === 0) {
      body.innerHTML = `
        <p class="empty">Registra un proveedor antes de crear una orden de compra.</p>
        <div class="modal-actions"><button type="button" class="btn-ghost" id="po-c-cancel">Cerrar</button></div>`;
      body.querySelector<HTMLButtonElement>("#po-c-cancel")!.addEventListener("click", close);
      return;
    }

    body.innerHTML = `
      <div class="modal-field">
        <label class="modal-label" for="po-c-supplier">Proveedor</label>
        <select id="po-c-supplier" class="view-select">
          ${suppliers
            .map((s) => `<option value="${escapeHtml(s.id)}">${escapeHtml(s.name)}</option>`)
            .join("")}
        </select>
      </div>
      <div class="modal-field">
        <label class="modal-label">Ítems</label>
        <div class="po-line-head">
          <span>Producto</span><span>Cant.</span><span>Costo unit.</span><span></span>
        </div>
        <div id="po-c-lines" class="po-lines"></div>
        <button type="button" class="btn-ghost po-line-add" id="po-c-add">+ Agregar línea</button>
      </div>
      <div class="modal-field">
        <label class="modal-label" for="po-c-ref">Referencia (opcional)</label>
        <input id="po-c-ref" type="text" autocomplete="off" />
      </div>
      <div class="modal-field">
        <label class="modal-label" for="po-c-notes">Notas (opcional)</label>
        <input id="po-c-notes" type="text" autocomplete="off" />
      </div>
      <div id="po-c-error" class="form-error" hidden></div>
      <div class="modal-actions">
        <button type="button" class="btn-ghost" id="po-c-cancel">Cancelar</button>
        <button type="button" class="btn-primary modal-confirm" id="po-c-save">
          <span class="btn-label">Crear OC</span>
        </button>
      </div>
    `;

    const linesHost = body.querySelector<HTMLElement>("#po-c-lines")!;
    const addLine = () => {
      const row = document.createElement("div");
      row.className = "po-line";
      row.innerHTML = `
        <input class="po-l-name" type="text" placeholder="Producto" autocomplete="off" />
        <input class="po-l-qty" type="number" min="1" step="1" value="1" />
        <input class="po-l-cost" type="number" min="0" step="1" placeholder="0" />
        <button class="po-l-del" type="button" title="Quitar">✕</button>
      `;
      row.querySelector<HTMLButtonElement>(".po-l-del")!.addEventListener("click", () => {
        if (linesHost.querySelectorAll(".po-line").length > 1) row.remove();
      });
      linesHost.appendChild(row);
    };
    addLine();
    body.querySelector<HTMLButtonElement>("#po-c-add")!.addEventListener("click", addLine);

    const supplierEl = body.querySelector<HTMLSelectElement>("#po-c-supplier")!;
    const refEl = body.querySelector<HTMLInputElement>("#po-c-ref")!;
    const notesEl = body.querySelector<HTMLInputElement>("#po-c-notes")!;
    const errEl = body.querySelector<HTMLElement>("#po-c-error")!;
    const saveBtn = body.querySelector<HTMLButtonElement>("#po-c-save")!;
    body.querySelector<HTMLButtonElement>("#po-c-cancel")!.addEventListener("click", close);

    saveBtn.addEventListener("click", async () => {
      const fail = (msg: string) => {
        errEl.hidden = false;
        errEl.textContent = msg;
      };
      const drafts: PoLineDraft[] = Array.from(
        linesHost.querySelectorAll<HTMLElement>(".po-line"),
      ).map((row) => ({
        name: row.querySelector<HTMLInputElement>(".po-l-name")!.value,
        qty: row.querySelector<HTMLInputElement>(".po-l-qty")!.value,
        cost: row.querySelector<HTMLInputElement>(".po-l-cost")!.value,
      }));
      const parsed = parsePoLines(drafts);
      if (!parsed.ok) return fail(parsed.error);
      const items: NewPurchaseOrderItem[] = parsed.value;

      errEl.hidden = true;
      saveBtn.classList.add("loading");
      saveBtn.disabled = true;
      try {
        await createPurchaseOrder(
          serverUrl,
          supplierEl.value,
          items,
          undefined,
          notesEl.value.trim() || undefined,
          refEl.value.trim() || undefined,
        );
        close();
        onSaved();
      } catch (err) {
        saveBtn.classList.remove("loading");
        saveBtn.disabled = false;
        fail(asMessage(err));
      }
    });
  })();
}

// --- PO detail drawer ------------------------------------------------------

/** Fetch a PO with its line items and show a read-only detail drawer. When the
 *  PO is in a receivable state, a "Recibir mercadería" button swaps in the
 *  receive form. `onChanged` fires after a successful receive (to reload the list). */
function openPoDetailModal(
  modalHost: HTMLElement,
  serverUrl: string,
  id: string,
  onChanged: () => void,
): void {
  const close = () => (modalHost.innerHTML = "");
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="po-modal-backdrop">
      <div class="modal modal-wide">
        <div class="modal-title">Orden de compra</div>
        <div id="po-d-body">${tableSkeleton(4)}</div>
      </div>
    </div>
  `;
  modalHost.querySelector<HTMLElement>("#po-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });

  const body = modalHost.querySelector<HTMLElement>("#po-d-body")!;
  void (async () => {
    let po: PurchaseOrderDetail;
    try {
      po = await getPurchaseOrder(serverUrl, id);
    } catch (err) {
      body.innerHTML = `${renderError(err)}<div class="modal-actions"><button type="button" class="btn-ghost" id="po-d-close">Cerrar</button></div>`;
      body.querySelector<HTMLButtonElement>("#po-d-close")!.addEventListener("click", close);
      return;
    }

    const pending = poPending(po.items);
    const canReceive = poIsReceivable(po.status) && pending > 0;
    // A draft has not been issued yet — the server refuses to receive it (409).
    // Surface an "Enviar al proveedor" action so the operator can complete the
    // draft → sent → recibir flow without leaving the app (BUG-bob-002 bridge).
    const canSend = poIsSendable(po.status);
    body.innerHTML = `
      ${renderPoDetail(po)}
      <div id="po-d-ap" class="po-ap">${tableSkeleton(2)}</div>
      <div id="po-d-error" class="form-error" hidden></div>
      <div class="modal-actions">
        <button type="button" class="btn-ghost" id="po-d-close">Cerrar</button>
        ${canSend ? `<button type="button" class="btn-primary modal-confirm" id="po-d-send">Enviar al proveedor</button>` : ""}
        ${canReceive ? `<button type="button" class="btn-primary modal-confirm" id="po-d-receive">Recibir mercadería</button>` : ""}
      </div>
    `;
    body.querySelector<HTMLButtonElement>("#po-d-close")!.addEventListener("click", close);
    const errEl = body.querySelector<HTMLElement>("#po-d-error")!;
    if (canSend) {
      const sendBtn = body.querySelector<HTMLButtonElement>("#po-d-send")!;
      sendBtn.addEventListener("click", async () => {
        errEl.hidden = true;
        sendBtn.classList.add("loading");
        sendBtn.disabled = true;
        try {
          await sendPurchaseOrder(serverUrl, po.id);
          // Reload the list (estado now «Enviada») and re-open the drawer so the
          // freshly-sent PO shows the "Recibir mercadería" action in place.
          onChanged();
          openPoDetailModal(modalHost, serverUrl, po.id, onChanged);
        } catch (err) {
          sendBtn.classList.remove("loading");
          sendBtn.disabled = false;
          errEl.hidden = false;
          errEl.textContent = asMessage(err);
        }
      });
    }
    if (canReceive) {
      body.querySelector<HTMLButtonElement>("#po-d-receive")!.addEventListener("click", () => {
        openReceiveModal(modalHost, serverUrl, po, onChanged);
      });
    }
    void loadAp(serverUrl, po, body.querySelector<HTMLElement>("#po-d-ap")!);
  })();
}

/** Load and render the accounts-payable block of a PO: total / paid / balance,
 *  the recorded payments, and (when there's a balance and the PO is live) an
 *  inline "registrar pago" form. Re-invokes itself after a payment so the
 *  drawer stays open and refreshes in place. */
async function loadAp(
  serverUrl: string,
  po: PurchaseOrderDetail,
  apHost: HTMLElement,
): Promise<void> {
  apHost.innerHTML = tableSkeleton(2);
  let summary: PurchasePaymentSummary;
  try {
    summary = await getPoPayments(serverUrl, po.id);
  } catch (err) {
    apHost.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    return;
  }
  apHost.innerHTML = renderApSection(summary, po);

  const payBtn = apHost.querySelector<HTMLButtonElement>("#po-ap-pay");
  const formHost = apHost.querySelector<HTMLElement>("#po-ap-form");
  if (!payBtn || !formHost) return;
  payBtn.addEventListener("click", () => {
    payBtn.hidden = true;
    formHost.hidden = false;
    formHost.innerHTML = paymentFormHtml(summary.balance);
    const amountEl = formHost.querySelector<HTMLInputElement>("#po-ap-amount")!;
    const methodEl = formHost.querySelector<HTMLSelectElement>("#po-ap-method")!;
    const refEl = formHost.querySelector<HTMLInputElement>("#po-ap-ref")!;
    const errEl = formHost.querySelector<HTMLElement>("#po-ap-error")!;
    const confirmBtn = formHost.querySelector<HTMLButtonElement>("#po-ap-confirm")!;
    formHost.querySelector<HTMLButtonElement>("#po-ap-cancel")!.addEventListener("click", () => {
      formHost.hidden = true;
      formHost.innerHTML = "";
      payBtn.hidden = false;
    });
    amountEl.focus();
    amountEl.select();

    confirmBtn.addEventListener("click", async () => {
      const fail = (msg: string) => {
        errEl.hidden = false;
        errEl.textContent = msg;
      };
      const amount = amountEl.value.trim();
      const amountNum = Number(amount);
      if (!amount || !Number.isFinite(amountNum) || amountNum <= 0) {
        return fail("Ingresa un monto mayor a 0.");
      }
      if (amountNum > toNumber(summary.balance)) {
        return fail(`El monto supera el saldo pendiente (${clp(summary.balance)}).`);
      }
      const method = methodEl.value;
      errEl.hidden = true;
      confirmBtn.classList.add("loading");
      confirmBtn.disabled = true;
      try {
        // Cash payments surface in the drawer arqueo, so attach the open session
        // when there is one (the server requires it for cash with an open till).
        let cashSession: string | undefined;
        if (method === "cash") {
          const open = await cashSessions(serverUrl, "open", 1);
          cashSession = open[0]?.id;
        }
        await createPoPayment(serverUrl, po.id, {
          amount,
          paymentMethod: method,
          cashSession,
          reference: refEl.value.trim() || undefined,
        });
        await loadAp(serverUrl, po, apHost);
      } catch (err) {
        confirmBtn.classList.remove("loading");
        confirmBtn.disabled = false;
        fail(asMessage(err));
      }
    });
  });
}

/** Payment medios — server values `cash`/`bank`/`card`/`transfer` mapped to
 *  Spanish labels for the picker + the payments table. */
const PAY_METHODS: { value: string; label: string }[] = [
  { value: "transfer", label: "Transferencia" },
  { value: "bank", label: "Depósito bancario" },
  { value: "card", label: "Tarjeta" },
  { value: "cash", label: "Efectivo (caja)" },
];

function methodLabel(m: string): string {
  return PAY_METHODS.find((x) => x.value === m)?.label ?? m;
}

function renderApSection(s: PurchasePaymentSummary, po: PurchaseOrderDetail): string {
  const cancelled = po.status.toLowerCase() === "cancelled";
  const canPay = !cancelled && !s.fully_paid && toNumber(s.balance) > 0;
  const badge = s.fully_paid
    ? `<span class="pill pill-ok">Pagada</span>`
    : cancelled
      ? `<span class="pill pill-danger">OC cancelada</span>`
      : `<span class="pill pill-warn">Saldo ${clp(s.balance)}</span>`;
  const payments = s.payments.length
    ? `<table class="data-table">
        <thead><tr><th>Fecha</th><th>Medio</th><th>Referencia</th><th class="num">Monto</th></tr></thead>
        <tbody>${s.payments.map(payRow).join("")}</tbody>
      </table>`
    : `<p class="muted">Sin pagos registrados.</p>`;
  return `
    <h4 class="section-title po-ap-title">Cuenta por pagar ${badge}</h4>
    <div class="rcpt-totals po-ap-totals">
      <div class="rcpt-line"><span>Total OC</span><strong>${clp(s.total)}</strong></div>
      <div class="rcpt-line"><span>Pagado</span><strong>${clp(s.paid)}</strong></div>
      <div class="rcpt-line total"><span>Saldo</span><strong>${clp(s.balance)}</strong></div>
    </div>
    ${payments}
    ${canPay
      ? `<button type="button" class="btn-ghost" id="po-ap-pay">+ Registrar pago</button>
         <div id="po-ap-form" hidden></div>`
      : ""}
  `;
}

function payRow(p: PurchasePayment): string {
  return `
    <tr>
      <td>${escapeHtml(fecha(p.paid_at))}</td>
      <td>${escapeHtml(methodLabel(p.payment_method))}</td>
      <td><span class="muted">${escapeHtml(p.reference || "—")}</span></td>
      <td class="num">${clp(p.amount)}</td>
    </tr>`;
}

function paymentFormHtml(balance: string): string {
  return `
    <div class="po-ap-pay-form">
      <div class="modal-field">
        <label class="modal-label" for="po-ap-amount">Monto del pago</label>
        <input id="po-ap-amount" type="number" min="0" step="1" value="${escapeHtml(balance)}" />
      </div>
      <div class="modal-field">
        <label class="modal-label" for="po-ap-method">Medio de pago</label>
        <select id="po-ap-method">
          ${PAY_METHODS.map((m) => `<option value="${m.value}">${m.label}</option>`).join("")}
        </select>
      </div>
      <div class="modal-field">
        <label class="modal-label" for="po-ap-ref">Referencia (N° transferencia / comprobante, opcional)</label>
        <input id="po-ap-ref" type="text" autocomplete="off" />
      </div>
      <div id="po-ap-error" class="form-error" hidden></div>
      <div class="modal-actions">
        <button type="button" class="btn-ghost" id="po-ap-cancel">Cancelar</button>
        <button type="button" class="btn-primary" id="po-ap-confirm">Registrar pago</button>
      </div>
    </div>`;
}

function renderPoDetail(po: PurchaseOrderDetail): string {
  const supplierShort = po.supplier.replace(/^supplier:/, "");
  const head = `
    <div class="po-detail-head">
      <div class="cell-main">${escapeHtml(supplierShort)} · ${statusPill(po.status)}</div>
      <div class="po-detail-meta">
        ${po.external_ref ? `<span>Ref: ${escapeHtml(po.external_ref)}</span>` : ""}
        <span>Moneda: ${escapeHtml(po.currency)}</span>
        <span>Fecha: ${escapeHtml(fecha(po.created_at))}</span>
        <span>Total: ${clp(po.total)}</span>
      </div>
      ${po.notes ? `<div class="cell-sub muted">${escapeHtml(po.notes)}</div>` : ""}
    </div>
  `;
  const rows = po.items
    .map((it) => {
      const remaining = Math.max(0, it.quantity - it.qty_received);
      return `
        <tr>
          <td><div class="cell-main">${escapeHtml(it.product_name)}</div></td>
          <td class="num">${num(it.quantity)}</td>
          <td class="num">${num(it.qty_received)}</td>
          <td class="num">${remaining > 0 ? `<span class="pill pill-warn">${num(remaining)}</span>` : num(0)}</td>
          <td class="num">${clp(it.unit_cost)}</td>
          <td class="num">${clp(it.subtotal)}</td>
        </tr>`;
    })
    .join("");
  return `
    ${head}
    <table class="data-table">
      <thead>
        <tr>
          <th>Producto</th>
          <th class="num">Pedido</th>
          <th class="num">Recibido</th>
          <th class="num">Pendiente</th>
          <th class="num">Costo unit.</th>
          <th class="num">Subtotal</th>
        </tr>
      </thead>
      <tbody>${rows}</tbody>
    </table>
  `;
}

// --- receive (goods receipt) modal -----------------------------------------

/** Goods-receipt form: one quantity input per line with a pending balance,
 *  defaulted to the remaining quantity (capped at it). On submit, bumps stock +
 *  weighted-average cost server-side. `onSaved` fires after success. */
function openReceiveModal(
  modalHost: HTMLElement,
  serverUrl: string,
  po: PurchaseOrderDetail,
  onSaved: () => void,
): void {
  const close = () => (modalHost.innerHTML = "");
  const pending = po.items.filter((it) => it.quantity - it.qty_received > 0);
  const lineInputs = pending
    .map((it) => {
      const remaining = it.quantity - it.qty_received;
      return `
        <div class="po-line" data-line="${escapeHtml(it.id)}" data-max="${remaining}">
          <div class="cell-sub">
            <div class="cell-main">${escapeHtml(it.product_name)}</div>
            <span class="muted">pendiente ${num(remaining)} de ${num(it.quantity)}</span>
          </div>
          <input class="po-r-qty" type="number" min="0" step="1" max="${remaining}" value="${remaining}" />
        </div>`;
    })
    .join("");

  modalHost.innerHTML = `
    <div class="modal-backdrop" id="po-modal-backdrop">
      <div class="modal modal-wide">
        <div class="modal-title">Recibir mercadería</div>
        <p class="muted">Ajusta las cantidades recibidas. El stock y el costo promedio se actualizan al confirmar.</p>
        <div class="po-lines">${lineInputs}</div>
        <div class="modal-field">
          <label class="modal-label" for="po-r-notes">Notas (opcional)</label>
          <input id="po-r-notes" type="text" autocomplete="off" />
        </div>
        <div id="po-r-error" class="form-error" hidden></div>
        <div class="modal-actions">
          <button type="button" class="btn-ghost" id="po-r-cancel">Cancelar</button>
          <button type="button" class="btn-primary modal-confirm" id="po-r-save">
            <span class="btn-label">Confirmar recepción</span>
          </button>
        </div>
      </div>
    </div>
  `;
  modalHost.querySelector<HTMLElement>("#po-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });

  const errEl = modalHost.querySelector<HTMLElement>("#po-r-error")!;
  const notesEl = modalHost.querySelector<HTMLInputElement>("#po-r-notes")!;
  const saveBtn = modalHost.querySelector<HTMLButtonElement>("#po-r-save")!;
  modalHost.querySelector<HTMLButtonElement>("#po-r-cancel")!.addEventListener("click", close);

  saveBtn.addEventListener("click", async () => {
    const fail = (msg: string) => {
      errEl.hidden = false;
      errEl.textContent = msg;
    };
    const lines: ReceiveLine[] = [];
    for (const row of Array.from(modalHost.querySelectorAll<HTMLElement>(".po-line"))) {
      const id = row.dataset.line!;
      const max = Number(row.dataset.max);
      const qty = Number(row.querySelector<HTMLInputElement>(".po-r-qty")!.value);
      const err = validateReceiveQty(qty, max);
      if (err) return fail(err);
      if (qty > 0) lines.push({ po_line_id: id, qty_received: qty });
    }
    if (lines.length === 0) return fail("Ingresa al menos una cantidad a recibir.");

    errEl.hidden = true;
    saveBtn.classList.add("loading");
    saveBtn.disabled = true;
    try {
      await receivePurchaseOrder(serverUrl, po.id, lines, notesEl.value.trim() || undefined);
      close();
      onSaved();
    } catch (err) {
      saveBtn.classList.remove("loading");
      saveBtn.disabled = false;
      fail(asMessage(err));
    }
  });
}

function renderError(err: unknown): string {
  // Permission / offline / generic copy is centralized so compras, gastos and
  // inventory all degrade the same way (no blank screen, no raw English code).
  return errorStateHtml(err, "las compras");
}
