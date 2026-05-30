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
  receivePurchaseOrder,
  type PurchaseOrder,
  type PurchaseOrderDetail,
  type NewPurchaseOrderItem,
  type ReceiveLine,
  type Supplier,
} from "../api";
import { clp, num } from "../format";
import { kpiSkeleton, tableSkeleton, asMessage, escapeHtml } from "./inventory";

/** Statuses a PO can be received from (server allows receiving from these). */
const RECEIVABLE = ["sent", "approved", "partial", "partially_received"];

const PAGE_LIMIT = 60;

const STATUS_OPTS: { value: string; label: string }[] = [
  { value: "", label: "Todas" },
  { value: "draft", label: "Borrador" },
  { value: "sent", label: "Enviadas" },
  { value: "received", label: "Recibidas" },
  { value: "partial", label: "Parciales" },
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
      renderTable(tableHost, rows);
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
        rutEl.value.trim() || undefined,
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
  const total = rows.length;
  const pending = rows.filter((r) =>
    ["draft", "sent", "partial"].includes(r.status.toLowerCase()),
  ).length;
  const totalValue = rows.reduce((sum, r) => sum + Number(r.total), 0);
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

function renderTable(host: HTMLElement, rows: PurchaseOrder[]): void {
  if (rows.length === 0) {
    host.innerHTML = `<p class="empty">Sin órdenes de compra para el filtro seleccionado.</p>`;
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
  const date = fmtDate(po.created_at);
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
  const s = status.toLowerCase();
  let cls = "pill-ok";
  let label = status;
  switch (s) {
    case "draft":
      cls = "pill-warn";
      label = "Borrador";
      break;
    case "sent":
      cls = "pill-warn";
      label = "Enviada";
      break;
    case "received":
      cls = "pill-ok";
      label = "Recibida";
      break;
    case "partial":
      cls = "pill-warn";
      label = "Parcial";
      break;
    case "cancelled":
      cls = "pill-danger";
      label = "Cancelada";
      break;
    default:
      cls = "pill-ok";
      label = status;
  }
  return `<span class="pill ${cls}">${escapeHtml(label)}</span>`;
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
      const items: NewPurchaseOrderItem[] = [];
      for (const row of Array.from(linesHost.querySelectorAll<HTMLElement>(".po-line"))) {
        const name = row.querySelector<HTMLInputElement>(".po-l-name")!.value.trim();
        const qty = Number(row.querySelector<HTMLInputElement>(".po-l-qty")!.value);
        const cost = row.querySelector<HTMLInputElement>(".po-l-cost")!.value.trim();
        if (name === "" && cost === "") continue; // skip blank rows
        if (name === "") return fail("Cada línea necesita un nombre de producto.");
        if (!Number.isInteger(qty) || qty < 1) return fail("La cantidad debe ser un entero ≥ 1.");
        const costNum = Number(cost);
        if (cost === "" || !Number.isFinite(costNum) || costNum < 0) {
          return fail("El costo unitario debe ser un número válido ≥ 0.");
        }
        items.push({ product_name: name, quantity: qty, unit_cost: cost });
      }
      if (items.length === 0) return fail("Agrega al menos una línea con producto y costo.");

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

    const pending = po.items.reduce((n, it) => n + Math.max(0, it.quantity - it.qty_received), 0);
    const canReceive = RECEIVABLE.includes(po.status.toLowerCase()) && pending > 0;
    body.innerHTML = `
      ${renderPoDetail(po)}
      <div class="modal-actions">
        <button type="button" class="btn-ghost" id="po-d-close">Cerrar</button>
        ${canReceive ? `<button type="button" class="btn-primary modal-confirm" id="po-d-receive">Recibir mercadería</button>` : ""}
      </div>
    `;
    body.querySelector<HTMLButtonElement>("#po-d-close")!.addEventListener("click", close);
    if (canReceive) {
      body.querySelector<HTMLButtonElement>("#po-d-receive")!.addEventListener("click", () => {
        openReceiveModal(modalHost, serverUrl, po, onChanged);
      });
    }
  })();
}

function renderPoDetail(po: PurchaseOrderDetail): string {
  const supplierShort = po.supplier.replace(/^supplier:/, "");
  const head = `
    <div class="po-detail-head">
      <div class="cell-main">${escapeHtml(supplierShort)} · ${statusPill(po.status)}</div>
      <div class="po-detail-meta">
        ${po.external_ref ? `<span>Ref: ${escapeHtml(po.external_ref)}</span>` : ""}
        <span>Moneda: ${escapeHtml(po.currency)}</span>
        <span>Fecha: ${escapeHtml(fmtDate(po.created_at))}</span>
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
      if (!Number.isInteger(qty) || qty < 0) return fail("Las cantidades deben ser enteros ≥ 0.");
      if (qty > max) return fail("No puedes recibir más de lo pendiente.");
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
  const msg = asMessage(err);
  // 403 = cashier doesn't have access (shouldn't happen but be graceful)
  if (msg.includes("403") || msg.includes("denegado")) {
    return `
      <div class="caja-empty">
        <div class="caja-empty-mark">●</div>
        <h3>Sin acceso a compras</h3>
        <p class="muted">Tu rol no tiene permiso para ver las órdenes de compra. Contacta al administrador.</p>
      </div>
    `;
  }
  return `<div class="view-error">${escapeHtml(msg)}</div>`;
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-CL", {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
  });
}
