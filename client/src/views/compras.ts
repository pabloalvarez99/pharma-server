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
import { listPurchaseOrders, type PurchaseOrder } from "../api";
import { clp, num } from "../format";
import { kpiSkeleton, tableSkeleton, asMessage, escapeHtml } from "./inventory";

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
        </div>
      </div>

      <div id="compras-kpis" class="kpi-grid">${kpiSkeleton(3)}</div>

      <div class="table-card">
        <div id="compras-table">${tableSkeleton()}</div>
      </div>
    </section>
  `;

  const kpiHost = host.querySelector<HTMLElement>("#compras-kpis")!;
  const tableHost = host.querySelector<HTMLElement>("#compras-table")!;
  const statusEl = host.querySelector<HTMLSelectElement>("#compras-status")!;

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

  statusEl.addEventListener("change", () => void reload());
  void reload();
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
    <tr>
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
