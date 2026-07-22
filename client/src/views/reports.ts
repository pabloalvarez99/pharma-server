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
// Vendor-agnostic export: each table exports CSV+JSON, and "Exportar todo" dumps
// a combined JSON of every loaded panel — pure shaping lives in reports-helpers.
import {
  salesDaily,
  topProducts,
  inventorySummary,
  marginsDaily,
  stockRotation,
  nearExpiry,
  listProducts,
  ivaSummary,
  libroCompras,
  debtorsReport,
  type DebtorRow,
  type PurchaseBookRow,
  type DailySalesRow,
  type TopProductRow,
  type DailyMarginRow,
  type StockRotationRow,
  type NearExpiryRow,
} from "../api";
import { clp, num, fecha } from "../format";
import { emptyState, errorState } from "./ui";
import { buildInsights, priceMap, type Insight } from "./reports-insights";
import {
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  asMessage,
  escapeHtml,
} from "./view-blocks";
import { exportFilename, type ExportBundle } from "./stock-helpers";
import {
  pickTodayRow,
  classifyMarginError,
  abcToken,
  rotationDisplay,
  buildTopExport,
  buildRotationExport,
  buildLibroExport,
  buildReportsJson,
  type LoadedReports,
} from "./reports-helpers";

const TOP_LIMIT = 5;
const ROTATION_LIMIT = 15;

export function renderReports(host: HTMLElement, serverUrl: string): void {
  // Accumulates each panel's data as it resolves so the export actions can
  // serialize whatever has loaded (a still-loading/gated panel is just omitted).
  const loaded: LoadedReports = {};

  host.innerHTML = `
    <section class="view view-reports">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Reportes</h2>
          <p class="muted">Ventas, ranking, inventario, márgenes y rotación.</p>
        </div>
        <div class="view-actions">
          <button id="rep-export-all" class="btn btn-ghost" type="button">Exportar todo (JSON)</button>
        </div>
      </div>

      <h3 class="section-title rb-display">Qué pasa en tu negocio hoy</h3>
      <div id="rep-insights" class="insight-strip">${insightSkeleton(3)}</div>

      <h3 class="section-title rb-display">Ventas de hoy</h3>
      <div id="rep-sales" class="kpi-grid">${kpiSkeleton(4)}</div>

      <h3 class="section-title rb-display">Márgenes (hoy)</h3>
      <div id="rep-margins" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div class="report-cols">
        <div class="table-card rb-card">
          <div class="card-head">
            <h3 class="section-title rb-display">Top ${TOP_LIMIT} productos</h3>
            <div class="card-actions" id="rep-top-actions" hidden>
              <button class="btn btn-ghost btn-sm" type="button" data-export="top" data-fmt="csv">CSV</button>
              <button class="btn btn-ghost btn-sm" type="button" data-export="top" data-fmt="json">JSON</button>
            </div>
          </div>
          <div id="rep-top">${tableSkeleton(5)}</div>
        </div>
        <div class="table-card rb-card">
          <h3 class="section-title rb-display">Inventario</h3>
          <div id="rep-inv" class="kpi-grid kpi-grid-tight">${kpiSkeleton(3)}</div>
        </div>
      </div>

      <div class="table-card rb-card">
        <h3 class="section-title rb-display">Por cobrar (fiado)</h3>
        <div id="rep-cobrar">${tableSkeleton(4)}</div>
      </div>

      <h3 class="section-title rb-display">IVA del mes (F29)</h3>
      <div id="rep-iva" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div class="table-card rb-card">
        <div class="card-head">
          <h3 class="section-title rb-display">Libro de compras</h3>
          <div class="card-actions" id="rep-libro-actions" hidden>
            <button class="btn btn-ghost btn-sm" type="button" data-export="libro" data-fmt="csv">CSV</button>
            <button class="btn btn-ghost btn-sm" type="button" data-export="libro" data-fmt="json">JSON</button>
          </div>
        </div>
        <div id="rep-libro">${tableSkeleton(6)}</div>
      </div>

      <div class="table-card rb-card">
        <div class="card-head">
          <h3 class="section-title rb-display">Rotación de stock</h3>
          <div class="card-actions" id="rep-rotation-actions" hidden>
            <button class="btn btn-ghost btn-sm" type="button" data-export="rotation" data-fmt="csv">CSV</button>
            <button class="btn btn-ghost btn-sm" type="button" data-export="rotation" data-fmt="json">JSON</button>
          </div>
        </div>
        <div id="rep-rotation">${tableSkeleton(6)}</div>
      </div>
    </section>
  `;

  host.querySelector<HTMLButtonElement>("#rep-export-all")!.addEventListener(
    "click",
    () => {
      const stem = exportFilename("reportes");
      downloadExport(`${stem}.json`, "application/json;charset=utf-8", buildReportsJson(loaded));
    },
  );

  host.querySelectorAll<HTMLButtonElement>("[data-export]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const which = btn.dataset.export;
      const fmt = btn.dataset.fmt === "json" ? "json" : "csv";
      const bundle =
        which === "top"
          ? loaded.top && buildTopExport(loaded.top)
          : which === "libro"
            ? loaded.libro && buildLibroExport(loaded.libro)
            : loaded.rotation && buildRotationExport(loaded.rotation);
      if (bundle) downloadBundle(which!, fmt, bundle);
    });
  });

  void loadInsights(host.querySelector<HTMLElement>("#rep-insights")!, serverUrl);
  void loadSales(host.querySelector<HTMLElement>("#rep-sales")!, serverUrl, loaded);
  void loadMargins(host.querySelector<HTMLElement>("#rep-margins")!, serverUrl, loaded);
  void loadTop(host, serverUrl, loaded);
  void loadInventory(host.querySelector<HTMLElement>("#rep-inv")!, serverUrl, loaded);
  void loadRotation(host, serverUrl, loaded);
  void loadDebtors(host.querySelector<HTMLElement>("#rep-cobrar")!, serverUrl);
  void loadIva(host.querySelector<HTMLElement>("#rep-iva")!, serverUrl);
  void loadLibroCompras(host, serverUrl, loaded);
}

/** "¿Cuánto me deben?" — total por cobrar + quién debe, mayor primero. Es la
 *  pregunta diaria del dueño que fía; hasta ahora sólo se veía cliente por
 *  cliente en su ficha. */
async function loadDebtors(el: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const rep = await debtorsReport(serverUrl);
    if (rep.rows.length === 0) {
      el.innerHTML = emptyState({
        title: "Nadie te debe",
        hint: "Cuando fíes una venta el saldo del cliente aparece acá.",
      });
      return;
    }
    el.innerHTML = `
      <div class="kpi-grid kpi-grid-tight rep-cobrar-kpis">
        <div class="kpi"><span class="kpi-label">Total por cobrar</span><strong class="rb-num">${clp(rep.total_por_cobrar)}</strong></div>
        <div class="kpi"><span class="kpi-label">Clientes con deuda</span><strong class="rb-num">${num(rep.debtor_count)}</strong></div>
      </div>
      <table class="data-table">
        <thead><tr><th>Cliente</th><th>Teléfono</th><th>Último mov.</th><th class="num">Debe</th></tr></thead>
        <tbody>${rep.rows.map(debtorRow).join("")}</tbody>
      </table>
    `;
  } catch (err) {
    el.innerHTML = errorState(asMessage(err));
  }
}

function debtorRow(d: DebtorRow): string {
  return `
    <tr>
      <td>${escapeHtml(d.name)}</td>
      <td>${d.phone ? escapeHtml(d.phone) : "<span class=\"muted\">—</span>"}</td>
      <td>${escapeHtml(fecha(d.last_movement))}</td>
      <td class="num rb-num">${clp(d.balance)}</td>
    </tr>
  `;
}

/** Resumen IVA del mes: débito (ventas) − crédito (compras) = a pagar. Panel
 *  independiente: si el server no lo expone (o el rol no alcanza) se muestra el
 *  error del server sin tumbar el resto de Reportes. */
async function loadIva(el: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const s = await ivaSummary(serverUrl);
    const aPagar = Number(s.iva_a_pagar);
    // Positivo = le toca pagar; negativo = remanente a favor del negocio.
    const saldoLabel = aPagar >= 0 ? "IVA a pagar" : "Remanente a favor";
    const saldoValue = clp(aPagar >= 0 ? s.iva_a_pagar : String(Math.abs(aPagar)));
    el.innerHTML = `
      <div class="kpi"><span class="kpi-label">Débito (ventas)</span><strong class="rb-num">${clp(s.iva_debito)}</strong></div>
      <div class="kpi"><span class="kpi-label">Crédito (compras)</span><strong class="rb-num">${clp(s.iva_credito)}</strong></div>
      <div class="kpi"><span class="kpi-label">${saldoLabel}</span><strong class="rb-num">${saldoValue}</strong></div>
      <div class="kpi"><span class="kpi-label">Ventas netas</span><strong class="rb-num">${clp(s.ventas_neto)}</strong></div>
    `;
  } catch (err) {
    el.innerHTML = errorState(asMessage(err));
  }
}

/** Libro de compras del mes. Las filas sin factura capturada se marcan para que
 *  el dueño sepa qué completar antes de llevar el F29. */
async function loadLibroCompras(
  host: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  const el = host.querySelector<HTMLElement>("#rep-libro")!;
  try {
    const book = await libroCompras(serverUrl);
    loaded.libro = book;
    if (book.rows.length === 0) {
      el.innerHTML = emptyState({
        title: "Sin compras este mes",
        hint: "Cuando recepciones una orden de compra aparecerá acá.",
      });
      return;
    }
    const pending =
      book.pending_declaration > 0
        ? `<p class="muted rep-libro-note">${book.pending_declaration} documento(s) sin factura capturada — el neto y el IVA están estimados del total.</p>`
        : "";
    el.innerHTML = `
      ${pending}
      <table class="data-table">
        <thead>
          <tr><th>Fecha</th><th>Proveedor</th><th>Folio</th><th class="num">Neto</th><th class="num">IVA</th><th class="num">Total</th></tr>
        </thead>
        <tbody>${book.rows.map(libroRow).join("")}</tbody>
        <tfoot>
          <tr>
            <td colspan="3"><strong>Totales</strong></td>
            <td class="num rb-num"><strong>${clp(book.total_neto)}</strong></td>
            <td class="num rb-num"><strong>${clp(book.total_iva)}</strong></td>
            <td class="num rb-num"><strong>${clp(book.total)}</strong></td>
          </tr>
        </tfoot>
      </table>
    `;
    host.querySelector<HTMLElement>("#rep-libro-actions")?.removeAttribute("hidden");
  } catch (err) {
    el.innerHTML = errorState(asMessage(err));
  }
}

function libroRow(r: PurchaseBookRow): string {
  return `
    <tr>
      <td>${escapeHtml(fecha(r.date))}</td>
      <td>${escapeHtml(r.supplier_name)}${
        r.supplier_rut ? `<span class="cell-sub muted">${escapeHtml(r.supplier_rut)}</span>` : ""
      }</td>
      <td>${
        r.folio
          ? escapeHtml(r.folio)
          : `<span class="pill pill-warn">estimado</span>`
      }</td>
      <td class="num rb-num">${clp(r.neto)}</td>
      <td class="num rb-num">${clp(r.iva)}</td>
      <td class="num rb-num">${clp(r.total)}</td>
    </tr>
  `;
}

/** The headline strip: the reasons the dueño opens the app daily. Loads its own
 *  feeds in parallel (independent of the panels below, mirroring the file's
 *  "each panel loads alone" design) and renders actionable cards. Margins is
 *  Pro-gated — a rejection is classified so a Free user gets a soft upsell card,
 *  never an error. A total feed failure degrades to a calm placeholder, never a
 *  blank or a crash. */
async function loadInsights(host: HTMLElement, serverUrl: string): Promise<void> {
  const [sales, margins, expiry, rotation, top, products] = await Promise.allSettled([
    salesDaily(serverUrl),
    marginsDaily(serverUrl),
    nearExpiry(serverUrl),
    stockRotation(serverUrl),
    topProducts(serverUrl, TOP_LIMIT),
    listProducts(serverUrl),
  ]);

  const ok = <T>(r: PromiseSettledResult<T>, fallback: T): T =>
    r.status === "fulfilled" ? r.value : fallback;

  // Margins: a fulfilled value feeds the real delta; a FEATURE_REQUIRES_UPGRADE
  // rejection flips the soft upsell; any other rejection just drops the card.
  let marginsData: DailyMarginRow[] | null = null;
  let marginsGated = false;
  if (margins.status === "fulfilled") {
    marginsData = margins.value;
  } else {
    marginsGated = classifyMarginError(margins.reason).gated;
  }

  const insights = buildInsights({
    sales: ok(sales, [] as DailySalesRow[]),
    margins: marginsData,
    marginsGated,
    nearExpiry: ok(expiry, [] as NearExpiryRow[]),
    rotation: ok(rotation, [] as StockRotationRow[]),
    top: ok(top, [] as TopProductRow[]),
    prices: priceMap(ok(products, [])),
  });

  if (insights.length === 0) {
    host.innerHTML = `<p class="empty insight-empty">Aún no hay suficientes datos. Registra ventas y stock para ver tus alertas y oportunidades del día.</p>`;
    return;
  }
  host.innerHTML = insights.map(insightCard).join("");
}

/** Render one insight as a produced card (icon + headline + context + the
 *  suggested action), toned by urgency. Aligned with the vitrina bar
 *  (rubro-select-experience §9): cards, not raw rows. */
function insightCard(i: Insight): string {
  return `
    <article class="insight-card insight-${i.tone}">
      <div class="insight-icon" aria-hidden="true">${i.icon}</div>
      <div class="insight-body">
        <h4 class="insight-title">${escapeHtml(i.title)}</h4>
        <p class="insight-detail">${escapeHtml(i.detail)}</p>
        <p class="insight-action">${escapeHtml(i.action)}</p>
      </div>
    </article>
  `;
}

/** Shimmer placeholders while the insight feeds resolve. */
function insightSkeleton(n: number): string {
  return Array.from({ length: n })
    .map(() => `<div class="insight-card insight-skeleton"><div class="sk-line"></div><div class="sk-line sk-short"></div></div>`)
    .join("");
}

async function loadSales(
  host: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  try {
    const rows: DailySalesRow[] = await salesDaily(serverUrl);
    loaded.sales = rows;
    const today = pickTodayRow(rows);
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
    host.innerHTML = errorState(asMessage(err));
  }
}

/** Margins are Pro-gated. On Free the command rejects with the coded string
 *  `FEATURE_REQUIRES_UPGRADE|message`; show a soft upgrade note (no hard error,
 *  no dark pattern — one calm card) instead of blanking the panel. */
async function loadMargins(
  host: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  try {
    const rows: DailyMarginRow[] = await marginsDaily(serverUrl);
    loaded.margins = rows;
    const today = pickTodayRow(rows);
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
    const { gated, message } = classifyMarginError(err);
    if (gated) {
      loaded.margins_gated = true;
      host.innerHTML = `
        <div class="rep-upsell kpi-span">
          <span class="pill pill-warn">Plan Pro</span>
          <p class="muted">El reporte de márgenes está disponible en el plan Pro. ${escapeHtml(message)}</p>
        </div>`;
      return;
    }
    host.innerHTML = errorState(message);
  }
}

async function loadTop(
  root: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  const host = root.querySelector<HTMLElement>("#rep-top")!;
  try {
    const rows: TopProductRow[] = await topProducts(serverUrl, TOP_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = emptyState({
        title: "Aún no hay ventas para rankear",
        hint: "Cuando registres ventas en el POS, aquí verás tu ranking ABC de productos.",
      });
      return;
    }
    loaded.top = rows;
    root.querySelector<HTMLElement>("#rep-top-actions")!.hidden = false;
    host.innerHTML = `
      <table class="data-table rb-table">
        <thead>
          <tr><th>#</th><th>Producto</th><th class="num">Unid.</th><th class="num">Ingresos</th><th>ABC</th></tr>
        </thead>
        <tbody>${rows.map(topRow).join("")}</tbody>
      </table>
    `;
  } catch (err) {
    host.innerHTML = errorState(asMessage(err));
  }
}

function topRow(r: TopProductRow): string {
  const cls = abcToken(r.abc_class);
  const badge = `<span class="abc abc-${cls}" title="Clase ${cls.toUpperCase()} · ${escapeHtml(r.revenue_pct)}%">${cls.toUpperCase()}</span>`;
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

async function loadInventory(
  host: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  try {
    const s = await inventorySummary(serverUrl);
    loaded.inventory = s;
    host.innerHTML = [
      kpiCard("Valorización", clp(s.inventory_value), `${num(s.total)} productos`),
      kpiCard("Stock bajo", num(s.low_stock), "bajo el mínimo", s.low_stock > 0 ? "warn" : ""),
      kpiCard("Sin stock", num(s.out_of_stock), "agotados", s.out_of_stock > 0 ? "danger" : ""),
    ].join("");
  } catch (err) {
    host.innerHTML = errorState(asMessage(err));
  }
}

async function loadRotation(
  root: HTMLElement,
  serverUrl: string,
  loaded: LoadedReports,
): Promise<void> {
  const host = root.querySelector<HTMLElement>("#rep-rotation")!;
  try {
    const rows: StockRotationRow[] = await stockRotation(serverUrl);
    if (rows.length === 0) {
      host.innerHTML = emptyState({
        title: "Sin datos de rotación todavía",
        hint: "La rotación aparece cuando hay ventas y stock que medir.",
      });
      return;
    }
    loaded.rotation = rows;
    root.querySelector<HTMLElement>("#rep-rotation-actions")!.hidden = false;
    host.innerHTML = `
      <table class="data-table rb-table">
        <thead>
          <tr><th>Producto</th><th class="num">Vendidas</th><th class="num">Stock</th><th class="num">Rotación</th><th class="num">Días inv.</th></tr>
        </thead>
        <tbody>${rows.slice(0, ROTATION_LIMIT).map(rotationRow).join("")}</tbody>
      </table>
    `;
  } catch (err) {
    host.innerHTML = errorState(asMessage(err));
  }
}

function rotationRow(r: StockRotationRow): string {
  const { turnover, days } = rotationDisplay(r);
  const daysCell = days === "—" ? "—" : num(Number(days));
  return `
    <tr>
      <td><div class="cell-main">${escapeHtml(r.product_name)}</div></td>
      <td class="num">${num(r.qty_sold)}</td>
      <td class="num">${num(r.current_stock)}</td>
      <td class="num">${turnover}</td>
      <td class="num">${daysCell}</td>
    </tr>
  `;
}

/** Download one report's bundle in the chosen format (CSV gets a UTF-8 BOM so
 *  Excel es-CL reads tildes/ñ; mirrors inventory.ts). */
function downloadBundle(which: string, fmt: "csv" | "json", bundle: ExportBundle): void {
  const stem = exportFilename(which === "top" ? "top-productos" : "rotacion");
  if (fmt === "json") {
    downloadExport(`${stem}.json`, "application/json;charset=utf-8", bundle.json);
  } else {
    downloadExport(`${stem}.csv`, "text/csv;charset=utf-8", `﻿${bundle.csv}`);
  }
}

/** Trigger a client-side file download (mirrors inventory.ts `downloadExport`). */
function downloadExport(filename: string, mime: string, content: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
