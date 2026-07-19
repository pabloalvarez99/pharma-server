// Gastos view — daily egresos / caja-chica tracking for the counter operator:
//   • category filter (Todas / Arriendo / Servicios / …) → table of expenses
//     (categoría, descripción, monto, medio de pago, fecha) with a running total.
//   • "Nuevo gasto" → modal asking categoría, monto, descripción, medio de pago
//     and fecha → POST /expenses.
// Money (`amount`) crosses the wire as a STRING (Decimal) and is re-emitted
// verbatim; local Number is display-only. Spanish throughout, CLP via ../format.
// Same skeleton → fetch → swap pattern as the other views (compras + caja).
import { listExpenses, createExpense, type Expense } from "../api";
import { clp, num, fecha } from "../format";
import {
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  asMessage,
  escapeHtml,
  emptyStateHtml,
  errorStateHtml,
} from "./view-blocks";
import { toRfc3339Noon, parseExpense, expenseTotal, gastosEmpty } from "./stock-helpers";
import { exportRows, type ExportColumn } from "./export-helpers";

const PAGE_LIMIT = 100;

// Export columns (sin lock-in, ADR-0005 #4): es-CL headers the owner reads;
// `monto` stays the raw Decimal STRING so the file re-imports losslessly. Built
// once and reused for the CSV + JSON download.
export const GASTO_EXPORT_COLUMNS: ExportColumn<Expense>[] = [
  { header: "categoria", key: "category", value: (e) => categoryLabel(e.category) },
  { header: "descripcion", key: "description", value: (e) => e.description ?? "" },
  { header: "medio_pago", key: "payment_method", value: (e) => paymentLabel(e.payment_method) },
  { header: "monto", key: "amount", value: (e) => e.amount },
  { header: "fecha", key: "incurred_at", value: (e) => e.incurred_at },
];

// Closed category set (value sent to the server / English; label shown / Spanish).
// "Otros" is the catch-all. The same list drives the filter and the form select.
const CATEGORIES: { value: string; label: string }[] = [
  { value: "rent", label: "Arriendo" },
  { value: "utilities", label: "Servicios básicos" },
  { value: "salaries", label: "Sueldos" },
  { value: "supplies", label: "Insumos" },
  { value: "maintenance", label: "Mantención" },
  { value: "taxes", label: "Impuestos" },
  { value: "transport", label: "Transporte" },
  { value: "other", label: "Otros" },
];

// Payment methods the server accepts (NewExpense.payment_method). `cash` default.
const PAYMENT_METHODS: { value: string; label: string }[] = [
  { value: "cash", label: "Efectivo" },
  { value: "bank", label: "Banco" },
  { value: "card", label: "Tarjeta" },
  { value: "transfer", label: "Transferencia" },
];

function categoryLabel(value: string): string {
  return CATEGORIES.find((c) => c.value === value)?.label ?? value;
}

function paymentLabel(value: string): string {
  return PAYMENT_METHODS.find((p) => p.value === value)?.label ?? value;
}

export function renderGastos(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-gastos">
      <div class="view-head">
        <div>
          <h2>Gastos</h2>
          <p class="muted">Egresos y caja chica del local.</p>
        </div>
        <div class="gastos-actions">
          <select id="gastos-category" class="view-select">
            <option value="">Todas las categorías</option>
            ${CATEGORIES.map(
              (c) => `<option value="${escapeHtml(c.value)}">${escapeHtml(c.label)}</option>`,
            ).join("")}
          </select>
          <button id="gastos-export-btn" class="btn-ghost" type="button" disabled>
            Exportar
          </button>
          <button id="gastos-new-btn" class="btn-primary">
            <span class="btn-label">Nuevo gasto</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>

      <div id="gastos-kpis" class="kpi-grid">${kpiSkeleton(2)}</div>

      <div class="table-card">
        <div id="gastos-table">${tableSkeleton()}</div>
      </div>

      <div id="gastos-modal-host"></div>
      <div id="gastos-toast" class="toast" hidden></div>
    </section>
  `;

  const kpiHost = host.querySelector<HTMLElement>("#gastos-kpis")!;
  const tableHost = host.querySelector<HTMLElement>("#gastos-table")!;
  const categoryEl = host.querySelector<HTMLSelectElement>("#gastos-category")!;
  const modalHost = host.querySelector<HTMLElement>("#gastos-modal-host")!;
  const toastEl = host.querySelector<HTMLElement>("#gastos-toast")!;
  const newBtn = host.querySelector<HTMLButtonElement>("#gastos-new-btn")!;
  const exportBtn = host.querySelector<HTMLButtonElement>("#gastos-export-btn")!;

  // Rows currently in view — what the "Exportar" button writes out (sin lock-in).
  let currentRows: Expense[] = [];

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  async function reload(): Promise<void> {
    tableHost.innerHTML = tableSkeleton();
    kpiHost.innerHTML = kpiSkeleton(2);
    const category = categoryEl.value || undefined;
    try {
      const rows = await listExpenses(serverUrl, category, undefined, PAGE_LIMIT);
      currentRows = rows;
      exportBtn.disabled = rows.length === 0;
      renderKpis(kpiHost, rows);
      renderTable(tableHost, rows, category !== undefined);
    } catch (err) {
      currentRows = [];
      exportBtn.disabled = true;
      kpiHost.innerHTML = "";
      tableHost.innerHTML = renderError(err);
    }
  }

  categoryEl.addEventListener("change", () => void reload());
  exportBtn.addEventListener("click", () => {
    if (currentRows.length === 0) return;
    exportRows("gastos", GASTO_EXPORT_COLUMNS, currentRows);
    toast(`Exportados ${currentRows.length} gasto(s)`);
  });
  newBtn.addEventListener("click", () =>
    openModal(modalHost, serverUrl, async () => {
      toast("Gasto registrado");
      await reload();
    }),
  );

  void reload();
}

function renderKpis(host: HTMLElement, rows: Expense[]): void {
  // Sum amounts as numbers for the display total only (server money stays STRING).
  const total = expenseTotal(rows);
  host.innerHTML = [
    kpiCard("Gastos", num(rows.length), "en el período visible"),
    kpiCard("Total", clp(total), "suma de gastos filtrados", "accent"),
  ].join("");
}

function renderTable(host: HTMLElement, rows: Expense[], filtered: boolean): void {
  if (rows.length === 0) {
    host.innerHTML = emptyStateHtml(gastosEmpty(filtered), filtered ? undefined : "gastos-empty-new");
    host
      .querySelector<HTMLButtonElement>("#gastos-empty-new")
      ?.addEventListener("click", () =>
        host.closest(".view-gastos")?.querySelector<HTMLButtonElement>("#gastos-new-btn")?.click(),
      );
    return;
  }

  host.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>Categoría</th>
          <th>Descripción</th>
          <th>Medio</th>
          <th class="num">Monto</th>
          <th>Fecha</th>
        </tr>
      </thead>
      <tbody>
        ${rows.map(expenseRow).join("")}
      </tbody>
    </table>
    <p class="table-foot muted">${rows.length} gasto(s)${
      rows.length === PAGE_LIMIT ? ` · mostrando los primeros ${PAGE_LIMIT}` : ""
    }</p>
  `;
}

function expenseRow(e: Expense): string {
  const date = fecha(e.incurred_at);
  const desc = e.description || "—";
  return `
    <tr>
      <td><div class="cell-main">${escapeHtml(categoryLabel(e.category))}</div></td>
      <td><div class="cell-sub muted">${escapeHtml(desc)}</div></td>
      <td><span class="muted">${escapeHtml(paymentLabel(e.payment_method))}</span></td>
      <td class="num">${clp(e.amount)}</td>
      <td><span class="muted">${escapeHtml(date)}</span></td>
    </tr>
  `;
}

// --- create flow -----------------------------------------------------------

function openModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onDone: () => void | Promise<void>,
): void {
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal" role="dialog" aria-modal="true" aria-label="Nuevo gasto">
        <h3 class="modal-title">Nuevo gasto</h3>
        <label class="field modal-field">
          <span class="modal-label">Categoría</span>
          <select id="gasto-category">
            ${CATEGORIES.map(
              (c) => `<option value="${escapeHtml(c.value)}">${escapeHtml(c.label)}</option>`,
            ).join("")}
          </select>
        </label>
        <label class="field modal-field">
          <span class="modal-label">Monto (CLP)</span>
          <input id="gasto-amount" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
        </label>
        <label class="field modal-field">
          <span class="modal-label">Descripción</span>
          <input id="gasto-description" type="text" autocomplete="off" placeholder="Ej: Boleta de luz mayo" />
        </label>
        <label class="field modal-field">
          <span class="modal-label">Medio de pago</span>
          <select id="gasto-method">
            ${PAYMENT_METHODS.map(
              (p) => `<option value="${escapeHtml(p.value)}">${escapeHtml(p.label)}</option>`,
            ).join("")}
          </select>
        </label>
        <label class="field modal-field">
          <span class="modal-label">Fecha</span>
          <input id="gasto-date" type="date" value="${todayInput()}" />
        </label>
        <div id="gasto-modal-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="gasto-cancel" class="btn-ghost">Cancelar</button>
          <button id="gasto-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">Registrar</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const categoryEl = modalHost.querySelector<HTMLSelectElement>("#gasto-category")!;
  const amountEl = modalHost.querySelector<HTMLInputElement>("#gasto-amount")!;
  const descEl = modalHost.querySelector<HTMLInputElement>("#gasto-description")!;
  const methodEl = modalHost.querySelector<HTMLSelectElement>("#gasto-method")!;
  const dateEl = modalHost.querySelector<HTMLInputElement>("#gasto-date")!;
  const errEl = modalHost.querySelector<HTMLElement>("#gasto-modal-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#gasto-confirm")!;

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#gasto-cancel")!.addEventListener("click", close);
  amountEl.focus();

  confirmBtn.addEventListener("click", async () => {
    const parsed = parseExpense(amountEl.value, descEl.value);
    if (!parsed.ok) {
      errEl.textContent = parsed.error;
      errEl.hidden = false;
      return;
    }
    const { amount, description } = parsed.value;
    errEl.hidden = true;
    confirmBtn.disabled = true;
    confirmBtn.classList.add("loading");
    try {
      await createExpense(serverUrl, {
        category: categoryEl.value,
        description,
        // Send the amount as a plain integer string — server parses Decimal.
        amount,
        paymentMethod: methodEl.value,
        incurredAt: toRfc3339Noon(dateEl.value),
      });
      close();
      await onDone();
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
      confirmBtn.disabled = false;
      confirmBtn.classList.remove("loading");
    }
  });
}

// --- helpers ---------------------------------------------------------------

function renderError(err: unknown): string {
  // Shared classifier → same degrade (permiso / sin conexión / genérico) as
  // inventory and compras. No blank screen, no raw English code to the operator.
  return errorStateHtml(err, "los gastos");
}

/** `YYYY-MM-DD` for today (local) — the default value of the date input. */
function todayInput(): string {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
}
