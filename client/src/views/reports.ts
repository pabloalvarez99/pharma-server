// Reportes view — read-only dashboard over real server data:
//   • Ventas hoy   → /reports/sales-daily (today's row: orders + revenue, with
//                    the cash/card tender split)
//   • Márgenes     → /reports/margins-daily (Pro-gated: shows an upgrade note on
//                    FEATURE_REQUIRES_UPGRADE instead of a hard error)
//   • Top 5        → /reports/top-products?limit=5 (Pareto ABC ranking)
//   • Inventario   → /products/stats (valuation + low/out of stock), reused
//   • Rotación     → /reports/stock-rotation (turnover + días de inventario)
// Every panel loads independently with its own skeleton so one slow/failed/gated
// call never blanks the others. Spanish throughout, CLP via ../format.
import {
  salesDaily,
  topProducts,
  inventorySummary,
  marginsDaily,
  stockRotation,
  parseSaleError,
  type DailySalesRow,
  type TopProductRow,
  type DailyMarginRow,
  type StockRotationRow,
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
const ROTATION_LIMIT = 15;

export function renderReports(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-reports">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Reportes</h2>
          <p class="muted">Ventas, ranking, inventario, márgenes y rotación.</p>
        </div>
      </div>

      <h3 class="section-title rb-display">Ventas de hoy</h3>
      <div id="rep-sales" class="kpi-grid">${kpiSkeleton(4)}</div>

      <h3 class="section-title rb-display">Márgenes (hoy)</h3>
      <div id="rep-margins" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div class="report-cols">
        <div class="table-card rb-card">
          <h3 class="section-title rb-display">Top ${TOP_LIMIT} productos</h3>
          <div id="rep-top">${tableSkeleton(5)}</div>
        </div>
        <div class="table-card rb-card">
          <h3 class="section-title rb-display">Inventario</h3>
          <div id="rep-inv" class="kpi-grid kpi-grid-tight">${kpiSkeleton(3)}</div>
        </div>
      </div>

      <div class="table-card rb-card">
        <h3 class="section-title rb-display">Rotación de stock</h3>
        <div id="rep-rotation">${tableSkeleton(6)}</div>
      </div>
    </section>
  `;

  void loadSales(host.querySelector<HTMLElement>("#rep-sales")!, serverUrl);
  void loadMargins(host.querySelector<HTMLElement>("#rep-margins")!, serverUrl);
  void loadTop(host.querySelector<HTMLElement>("#rep-top")!, serverUrl);
  void loadInventory(host.querySelector<HTMLElement>("#rep-inv")!, serverUrl);
  void loadRotation(host.querySelector<HTMLElement>("#rep-rotation")!, serverUrl);
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
function pickToday<T extends { date: string }>(rows: T[]): T | undefined {
  if (rows.length === 0) return undefined;
  const today = new Date().toISOString().slice(0, 10);
  return rows.find((r) => r.date === today) ?? rows[rows.length - 1];
}

/** Margins are Pro-gated. On Free the command rejects with the coded string
 *  `FEATURE_REQUIRES_UPGRADE|message`; show a soft upgrade note (no hard error,
 *  no dark pattern — one calm card) instead of blanking the panel. */
async function loadMargins(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rows: DailyMarginRow[] = await marginsDaily(serverUrl);
    const today = pickToday(rows);
    if (!today) {
      host.innerHTML = kpiCard("Margen hoy", "$0", "sin ventas registradas");
      return;
    }
    const noCost =
      today.items_without_cost > 0
        ? `${num(today.items_without_cost)} ítem(s) sin costo`
        : "todos con costo";
    host.innerHTML = [
      kpiCard("Margen hoy", clp(today.margin), `${escapeHtml(today.margin_pct)}% sobre venta`, "accent"),
      kpiCard("Ingresos", clp(today.revenue), today.date),
      kpiCard("Costo", clp(today.cost), "costo de ventas"),
      kpiCard("Sin costo", num(today.items_without_cost), noCost, today.items_without_cost > 0 ? "warn" : ""),
    ].join("");
  } catch (err) {
    const { code, message } = parseSaleError(err);
    if (code === "FEATURE_REQUIRES_UPGRADE") {
      host.innerHTML = `
        <div class="rep-upsell kpi-span">
          <span class="pill pill-warn">Plan Pro</span>
          <p class="muted">El reporte de márgenes está disponible en el plan Pro. ${escapeHtml(message)}</p>
        </div>`;
      return;
    }
    host.innerHTML = `<div class="view-error">${escapeHtml(message)}</div>`;
  }
}

async function loadTop(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rows: TopProductRow[] = await topProducts(serverUrl, TOP_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">Aún no hay ventas para rankear.</p>`;
      return;
    }
    host.innerHTML = `
      <table class="data-table rb-table">
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

async function loadRotation(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rows: StockRotationRow[] = await stockRotation(serverUrl);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">Sin datos de rotación todavía.</p>`;
      return;
    }
    host.innerHTML = `
      <table class="data-table rb-table">
        <thead>
          <tr><th>Producto</th><th class="num">Vendidas</th><th class="num">Stock</th><th class="num">Rotación</th><th class="num">Días inv.</th></tr>
        </thead>
        <tbody>${rows.slice(0, ROTATION_LIMIT).map(rotationRow).join("")}</tbody>
      </table>
    `;
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}

function rotationRow(r: StockRotationRow): string {
  const turnover = r.turnover ? `${escapeHtml(r.turnover)}×` : "—";
  const days = r.days_of_inventory ? num(Math.round(Number(r.days_of_inventory))) : "—";
  return `
    <tr>
      <td><div class="cell-main">${escapeHtml(r.product_name)}</div></td>
      <td class="num">${num(r.qty_sold)}</td>
      <td class="num">${num(r.current_stock)}</td>
      <td class="num">${turnover}</td>
      <td class="num">${days}</td>
    </tr>
  `;
}
