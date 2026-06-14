// Devoluciones view — refunds / returns over POS sales.
//   A list of recent devoluciones (fecha, orden, tipo, motivo, método, total) +
//   a "Nueva devolución" modal driven by the original sale's boleta: enter the
//   order id → "Cargar boleta" pulls its line items → pick quantities to refund
//   per line → motivo + tipo + método → confirm.
//
// Restock note: the server REQUIRES a product id on a line to return it to stock,
// and the boleta projection carries none — so refunds here record the money +
// flip the order to `refunded` but do NOT restock. Re-stock sellable goods via
// Inventario → Ajustar stock. (Surfaced in the modal.)
// Spanish throughout, CLP via ../format. Same skeleton → fetch → swap pattern.
import {
  getReceipt,
  createRefund,
  listRefunds,
  type Receipt,
  type Devolucion,
  type RefundItem,
} from "../api";
import { clp, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

const PAGE_LIMIT = 60;

const METODO_OPTS: { value: string; label: string }[] = [
  { value: "efectivo", label: "Efectivo" },
  { value: "tarjeta", label: "Tarjeta" },
  { value: "transferencia", label: "Transferencia" },
];

export function renderDevoluciones(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-devoluciones">
      <div class="view-head">
        <div>
          <h2>Devoluciones</h2>
          <p class="muted">Reembolsos sobre ventas. El dinero y el registro quedan; el stock se reabastece aparte.</p>
        </div>
        <button id="dev-new" type="button" class="btn-ghost">+ Nueva devolución</button>
      </div>

      <div id="dev-modal-host"></div>

      <div class="table-card">
        <h3 class="section-title">Devoluciones recientes</h3>
        <div id="dev-table">${tableSkeleton(6)}</div>
      </div>
    </section>
  `;

  const tableHost = host.querySelector<HTMLElement>("#dev-table")!;
  const modalHost = host.querySelector<HTMLElement>("#dev-modal-host")!;

  async function reload(): Promise<void> {
    tableHost.innerHTML = tableSkeleton(6);
    try {
      const rows = await listRefunds(serverUrl, undefined, undefined, PAGE_LIMIT);
      renderTable(tableHost, rows);
    } catch (err) {
      tableHost.innerHTML = renderError(err);
    }
  }

  host.querySelector<HTMLButtonElement>("#dev-new")!.addEventListener("click", () => {
    openRefundModal(modalHost, serverUrl, () => void reload());
  });

  void reload();
}

function renderTable(host: HTMLElement, rows: Devolucion[]): void {
  if (rows.length === 0) {
    host.innerHTML = `<p class="empty">Sin devoluciones registradas.</p>`;
    return;
  }
  host.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>Fecha</th>
          <th>Orden</th>
          <th>Tipo</th>
          <th>Motivo</th>
          <th>Método</th>
          <th class="num">Total</th>
        </tr>
      </thead>
      <tbody>${rows.map(devolucionRow).join("")}</tbody>
    </table>
    <p class="table-foot muted">${rows.length} devolución(es)${
      rows.length === PAGE_LIMIT ? ` · mostrando las primeras ${PAGE_LIMIT}` : ""
    }</p>
  `;
}

function devolucionRow(d: Devolucion): string {
  const orderShort = d.order ? d.order.replace(/^order:/, "") : "—";
  const tipo = d.tipo.toLowerCase() === "total"
    ? `<span class="pill pill-danger">Total</span>`
    : `<span class="pill pill-warn">Parcial</span>`;
  return `
    <tr>
      <td><span class="muted">${escapeHtml(fmtDate(d.created_at))}</span></td>
      <td><div class="cell-sub muted">${escapeHtml(orderShort)}</div></td>
      <td>${tipo}</td>
      <td><div class="cell-main">${escapeHtml(d.motivo)}</div></td>
      <td><span class="muted">${escapeHtml(metodoLabel(d.metodo_reembolso))}</span></td>
      <td class="num">${clp(d.total_devuelto)}</td>
    </tr>
  `;
}

/** "Nueva devolución" modal — load a sale's boleta, pick quantities, refund. */
function openRefundModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onSaved: () => void,
): void {
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="dev-modal-backdrop">
      <div class="modal modal-wide">
        <div class="modal-title">Nueva devolución</div>
        <div class="modal-field">
          <label class="modal-label" for="dev-f-order">Número de orden</label>
          <div class="dev-order-row">
            <input id="dev-f-order" type="text" placeholder="order:xxxx o el id de la venta" autocomplete="off" />
            <button type="button" class="btn-ghost" id="dev-f-load">Cargar boleta</button>
          </div>
        </div>
        <div id="dev-f-items"><p class="empty">Carga una boleta para elegir qué devolver.</p></div>
        <div class="dev-fields-grid">
          <div class="modal-field">
            <span class="modal-label">Tipo</span>
            <!-- Derived from the quantities below, not picked by hand: "Total"
                 only when every sold line is returned in full, else "Parcial".
                 Removes the old free select that could contradict the items. -->
            <div id="dev-f-tipo-badge" class="pill pill-warn dev-tipo-badge">Parcial</div>
          </div>
          <div class="modal-field">
            <label class="modal-label" for="dev-f-metodo">Método de reembolso</label>
            <select id="dev-f-metodo" class="view-select">
              ${METODO_OPTS.map((o) => `<option value="${escapeHtml(o.value)}">${escapeHtml(o.label)}</option>`).join("")}
            </select>
          </div>
        </div>
        <div class="modal-field">
          <label class="modal-label" for="dev-f-motivo">Motivo</label>
          <input id="dev-f-motivo" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="dev-f-notas">Notas (opcional)</label>
          <input id="dev-f-notas" type="text" autocomplete="off" />
        </div>
        <label class="dev-restock-toggle">
          <input id="dev-f-restock" type="checkbox" disabled />
          <span>Reingresar al stock</span>
        </label>
        <p class="muted dev-restock-note">No disponible desde la boleta: no identifica el producto (sólo nombre). Reingresa el stock vendible vía Inventario → Ajustar stock.</p>
        <div id="dev-f-error" class="form-error" hidden></div>
        <div class="modal-actions">
          <button type="button" class="btn-ghost" id="dev-f-cancel">Cancelar</button>
          <button type="button" class="btn-primary modal-confirm" id="dev-f-save" disabled>
            <span class="btn-label">Confirmar devolución</span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const orderEl = modalHost.querySelector<HTMLInputElement>("#dev-f-order")!;
  const loadBtn = modalHost.querySelector<HTMLButtonElement>("#dev-f-load")!;
  const itemsHost = modalHost.querySelector<HTMLElement>("#dev-f-items")!;
  const motivoEl = modalHost.querySelector<HTMLInputElement>("#dev-f-motivo")!;
  const tipoBadge = modalHost.querySelector<HTMLElement>("#dev-f-tipo-badge")!;
  const restockEl = modalHost.querySelector<HTMLInputElement>("#dev-f-restock")!;
  const metodoEl = modalHost.querySelector<HTMLSelectElement>("#dev-f-metodo")!;
  const notasEl = modalHost.querySelector<HTMLInputElement>("#dev-f-notas")!;
  const errEl = modalHost.querySelector<HTMLElement>("#dev-f-error")!;
  const saveBtn = modalHost.querySelector<HTMLButtonElement>("#dev-f-save")!;

  modalHost.querySelector<HTMLButtonElement>("#dev-f-cancel")!.addEventListener("click", close);
  modalHost.querySelector<HTMLElement>("#dev-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  orderEl.focus();

  // The receipt items currently loaded; the rows in #dev-f-items are aligned to it.
  let receipt: Receipt | null = null;

  const fail = (msg: string) => {
    errEl.hidden = false;
    errEl.textContent = msg;
  };

  // "Total" only when every sold line is returned in full; otherwise "Parcial".
  // Derived live from the qty inputs so the badge can never disagree with what
  // the cashier actually picked.
  const currentQtys = (): number[] =>
    Array.from(itemsHost.querySelectorAll<HTMLInputElement>(".dev-l-qty")).map((el) =>
      Number(el.value),
    );
  const deriveTipo = (): "total" | "parcial" => {
    if (!receipt) return "parcial";
    const qtys = currentQtys();
    const anyReturned = qtys.some((q) => q > 0);
    const allFull = receipt.items.every((it, i) => qtys[i] === it.qty);
    return anyReturned && allFull ? "total" : "parcial";
  };
  const refreshTipoBadge = () => {
    const t = deriveTipo();
    tipoBadge.textContent = t === "total" ? "Total" : "Parcial";
    tipoBadge.className = `pill dev-tipo-badge ${t === "total" ? "pill-danger" : "pill-warn"}`;
  };
  // Recompute as the cashier edits any quantity (delegated; survives reloads).
  itemsHost.addEventListener("input", refreshTipoBadge);

  const loadReceipt = async () => {
    const id = orderEl.value.trim();
    if (id === "") return fail("Ingresa el número de orden.");
    errEl.hidden = true;
    loadBtn.classList.add("loading");
    loadBtn.disabled = true;
    itemsHost.innerHTML = tableSkeleton(4);
    try {
      receipt = await getReceipt(serverUrl, id);
      renderItemRows(itemsHost, receipt);
      refreshTipoBadge();
      saveBtn.disabled = false;
    } catch (err) {
      receipt = null;
      saveBtn.disabled = true;
      itemsHost.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    } finally {
      loadBtn.classList.remove("loading");
      loadBtn.disabled = false;
    }
  };
  loadBtn.addEventListener("click", () => void loadReceipt());
  orderEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void loadReceipt();
    }
  });

  saveBtn.addEventListener("click", async () => {
    if (!receipt) return fail("Carga una boleta primero.");
    const motivo = motivoEl.value.trim();
    if (motivo === "") return fail("El motivo es obligatorio.");

    const items: RefundItem[] = [];
    const rows = Array.from(itemsHost.querySelectorAll<HTMLElement>(".dev-line"));
    for (let i = 0; i < rows.length; i++) {
      const item = receipt.items[i];
      const qty = Number(rows[i].querySelector<HTMLInputElement>(".dev-l-qty")!.value);
      if (!Number.isInteger(qty) || qty < 0) return fail("Las cantidades deben ser enteros ≥ 0.");
      if (qty > item.qty) return fail(`No puedes devolver más de lo vendido en «${item.name}».`);
      if (qty > 0) {
        items.push({
          product_name: item.name,
          quantity: qty,
          // restock only when the toggle is on AND the line identifies a product
          // (boleta lines don't, so this stays false — the toggle is disabled).
          unit_price: item.unit_price,
          restock: restockEl.checked && Boolean((item as { product?: string }).product),
        });
      }
    }
    if (items.length === 0) return fail("Ingresa al menos una cantidad a devolver.");

    errEl.hidden = true;
    saveBtn.classList.add("loading");
    saveBtn.disabled = true;
    try {
      await createRefund(serverUrl, motivo, items, {
        order: receipt.order_id,
        tipo: deriveTipo(),
        notas: notasEl.value.trim() || undefined,
        metodoReembolso: metodoEl.value,
      });
      close();
      onSaved();
    } catch (err) {
      saveBtn.classList.remove("loading");
      saveBtn.disabled = false;
      fail(asMessage(err));
    }
  });
}

function renderItemRows(host: HTMLElement, receipt: Receipt): void {
  if (receipt.items.length === 0) {
    host.innerHTML = `<p class="empty">La boleta no tiene ítems.</p>`;
    return;
  }
  const rows = receipt.items
    .map(
      (it) => `
      <div class="po-line dev-line">
        <div class="cell-sub">
          <div class="cell-main">${escapeHtml(it.name)}</div>
          <span class="muted">vendido ${num(it.qty)} · ${clp(it.unit_price)} c/u</span>
        </div>
        <input class="po-r-qty dev-l-qty" type="number" min="0" step="1" max="${it.qty}" value="0" />
      </div>`,
    )
    .join("");
  host.innerHTML = `
    <div class="modal-field">
      <label class="modal-label">Ítems a devolver (boleta ${escapeHtml(receipt.folio_or_number)})</label>
      <div class="po-lines">${rows}</div>
    </div>
  `;
}

function renderError(err: unknown): string {
  const msg = asMessage(err);
  if (msg.includes("403") || msg.includes("denegado")) {
    return `
      <div class="caja-empty">
        <div class="caja-empty-mark">●</div>
        <h3>Sin acceso a devoluciones</h3>
        <p class="muted">Tu rol no tiene permiso para registrar o ver devoluciones. Contacta al administrador.</p>
      </div>
    `;
  }
  return `<div class="view-error">${escapeHtml(msg)}</div>`;
}

function metodoLabel(m: string | null): string {
  if (!m) return "—";
  const found = METODO_OPTS.find((o) => o.value === m.toLowerCase());
  return found ? found.label : m;
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-CL", { day: "2-digit", month: "2-digit", year: "2-digit" });
}
