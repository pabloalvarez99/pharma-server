// Reportes view — read-only dashboard over real server data:
//   • Ventas hoy   → /reports/sales-daily (today's row: orders + revenue, with
//                    the cash/card tender split)
//   • Top 5        → /reports/top-products?limit=5 (Pareto ABC ranking)
//   • Inventario   → /products/stats (valuation + low/out of stock), reused
// Every panel loads independently with its own skeleton so one slow/failed
// call never blanks the others. Spanish throughout, CLP via ../format.
import {
  salesDaily,
  topProducts,
  inventorySummary,
  type DailySalesRow,
  type TopProductRow,
} from "../api";
import { clp, num } from "../format";
import {
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  asMessage,
  escapeHtml,
} from "./inventory";

const TOP_LIMIT = 5;

export function renderReports(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-reports">
      <div class="view-head">
        <div>
          <h2>Reportes</h2>
          <p class="muted">Resumen de ventas, ranking de productos e inventario.</p>
        </div>
      </div>

      <h3 class="section-title">Ventas de hoy</h3>
      <div id="rep-sales" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div class="report-cols">
        <div class="table-card">
          <h3 class="section-title">Top ${TOP_LIMIT} productos</h3>
          <div id="rep-top">${tableSkeleton(5)}</div>
        </div>
        <div class="table-card">
          <h3 class="section-title">Inventario</h3>
          <div id="rep-inv" class="kpi-grid kpi-grid-tight">${kpiSkeleton(3)}</div>
        </div>
      </div>
    </section>
  `;

  void loadSales(host.querySelector<HTMLElement>("#rep-sales")!, serverUrl);
  void loadTop(host.querySelector<HTMLElement>("#rep-top")!, serverUrl);
  void loadInventory(host.querySelector<HTMLElement>("#rep-inv")!, serverUrl);
}

async function loadSales(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rows: DailySalesRow[] = await salesDaily(serverUrl);
    const today = pickToday(rows);
    if (!today) {
      host.innerHTML = kpiCard("Ventas hoy", "$0", "sin ventas registradas");
      return;
    }
    host.innerHTML = [
      kpiCard("Ventas hoy", clp(today.revenue), `${num(today.orders)} venta(s)`, "accent"),
      kpiCard("Boletas", num(today.orders), today.date),
      kpiCard("Efectivo", clp(today.cash), "recaudado hoy"),
      kpiCard("Tarjeta", clp(today.card), "débito + crédito"),
    ].join("");
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}

/** Pick the most recent row whose date matches today (UTC, YYYY-MM-DD); fall
 *  back to the last row the server returned so a TZ edge never blanks the card. */
function pickToday(rows: DailySalesRow[]): DailySalesRow | undefined {
  if (rows.length === 0) return undefined;
  const today = new Date().toISOString().slice(0, 10);
  return rows.find((r) => r.date === today) ?? rows[rows.length - 1];
}

async function loadTop(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rows: TopProductRow[] = await topProducts(serverUrl, TOP_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">Aún no hay ventas para rankear.</p>`;
      return;
    }
    host.innerHTML = `
      <table class="data-table">
        <thead>
          <tr><th>#</th><th>Producto</th><th class="num">Unid.</th><th class="num">Ingresos</th><th>ABC</th></tr>
        </thead>
        <tbody>${rows.map(topRow).join("")}</tbody>
      </table>
    `;
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}

function topRow(r: TopProductRow): string {
  const cls = r.abc_class.toUpperCase();
  const badge = `<span class="abc abc-${cls.toLowerCase()}" title="Clase ${cls} · ${escapeHtml(r.revenue_pct)}%">${cls}</span>`;
  return `
    <tr>
      <td class="rank">${r.rank}</td>
      <td><div class="cell-main">${escapeHtml(r.product_name)}</div></td>
      <td class="num">${num(r.qty_sold)}</td>
      <td class="num">${clp(r.revenue)}</td>
      <td>${badge}</td>
    </tr>
  `;
}

async function loadInventory(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const s = await inventorySummary(serverUrl);
    host.innerHTML = [
      kpiCard("Valorización", clp(s.inventory_value), `${num(s.total)} productos`),
      kpiCard("Stock bajo", num(s.low_stock), "bajo el mínimo", s.low_stock > 0 ? "warn" : ""),
      kpiCard("Sin stock", num(s.out_of_stock), "agotados", s.out_of_stock > 0 ? "danger" : ""),
    ].join("");
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}
