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
  CUSTOMERS_MODULE_MISSING,
  type Customer,
  type CustomerDetail,
  type CustomerOrder,
} from "../api";
import { clp, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

const HISTORY_LIMIT = 20;

export function renderClientes(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-clientes">
      <div class="view-head">
        <div>
          <h2>Clientes</h2>
          <p class="muted">Busca por nombre, RUT o teléfono y revisa su historial.</p>
        </div>
        <div class="view-search">
          <input id="cli-search" type="search" placeholder="Buscar cliente…" autocomplete="off" />
        </div>
      </div>

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

  let timer: number | undefined;
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    const q = searchEl.value.trim();
    if (q === "") {
      resultsEl.innerHTML = `<p class="empty">Escribe para buscar un cliente.</p>`;
      return;
    }
    timer = window.setTimeout(() => {
      resultsEl.innerHTML = tableSkeleton(5);
      void runSearch(resultsEl, detailEl, serverUrl, q);
    }, 240);
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
        ${sub ? `<div class="cell-sub muted">${escapeHtml(sub)}</div>` : ""}
      </div>
      <div class="cli-points">${num(c.loyalty_points)} pts</div>
    </button>
  `;
}

async function loadDetail(
  detailEl: HTMLElement,
  serverUrl: string,
  id: string,
): Promise<void> {
  try {
    const [detail, history] = await Promise.all([
      customerDetail(serverUrl, id),
      customerHistory(serverUrl, id, HISTORY_LIMIT).catch((err) => {
        // History is secondary — a missing module sentinel here would already
        // have surfaced via the detail call. Treat other failures as empty.
        if (err === CUSTOMERS_MODULE_MISSING) throw err;
        return [] as CustomerOrder[];
      }),
    ]);
    detailEl.innerHTML = renderDetail(detail, history);
  } catch (err) {
    detailEl.innerHTML = renderError(err);
  }
}

function renderDetail(c: CustomerDetail, history: CustomerOrder[]): string {
  const contact = [
    c.rut ? `RUT ${escapeHtml(c.rut)}` : "",
    c.phone ? escapeHtml(c.phone) : "",
    c.email ? escapeHtml(c.email) : "",
  ]
    .filter(Boolean)
    .join(" · ");

  const head = `
    <div class="cli-detail-head">
      <div class="cli-detail-name">${escapeHtml(c.name)}</div>
      ${contact ? `<div class="cell-sub muted">${contact}</div>` : ""}
      ${c.active ? "" : `<span class="pill pill-warn">Inactivo</span>`}
    </div>
    <div class="cli-stats">
      <div class="cli-stat"><span class="kpi-label">Puntos</span><strong>${num(c.loyalty_points)}</strong></div>
      <div class="cli-stat"><span class="kpi-label">Total comprado</span><strong>${clp(c.total_spent)}</strong></div>
      <div class="cli-stat"><span class="kpi-label">Visitas</span><strong>${num(c.visit_count)}</strong></div>
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

  return `${head}<h3 class="section-title cli-hist-title">Historial de compras</h3>${hist}`;
}

function historyRow(o: CustomerOrder): string {
  const st = o.status.toLowerCase();
  const tone =
    st === "refunded" || st === "cancelled" ? "pill-danger" : st === "pending" ? "pill-warn" : "pill-ok";
  return `
    <tr>
      <td>${fmtDate(o.created_at)}</td>
      <td>${escapeHtml(payLabel(o.payment_method))}</td>
      <td class="num">${num(o.items_count)}</td>
      <td class="num">${clp(o.total)}</td>
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
