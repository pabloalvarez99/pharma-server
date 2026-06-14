// Dashboard — the landing view after login. An executive overview that COMPOSES
// existing endpoints (no new server calls): ventas de hoy (/reports/sales-daily),
// valor de inventario + low/out counts (/products/stats), próximos a vencer
// (derived from stats `expired`), and the top-5 products (/reports/top-products).
// Big KPI tiles up top, a compact top-products table below. Each region loads
// independently with its own skeleton so one slow/failed call never blanks the
// rest. Spanish throughout, CLP via ../format — same pattern as Inventario/Reportes.
import {
  salesDaily,
  topProducts,
  inventorySummary,
  type DailySalesRow,
  type TopProductRow,
  type InventorySummary,
} from "../api";
import { clp, num } from "../format";
import {
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  asMessage,
  escapeHtml,
} from "./inventory";
import { dashboardCta, type DashboardCta } from "./onboarding-ux";

const TOP_LIMIT = 5;

export function renderDashboard(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-dashboard">
      <div class="view-head">
        <div>
          <h2>Panel</h2>
          <p class="muted">Resumen ejecutivo de tu negocio hoy.</p>
        </div>
      </div>

      <div id="dash-cta" hidden></div>

      <div id="dash-kpis" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div class="table-card">
        <h3 class="section-title">Top ${TOP_LIMIT} productos</h3>
        <div id="dash-top">${tableSkeleton(5)}</div>
      </div>
    </section>
  `;

  void loadKpis(
    host.querySelector<HTMLElement>("#dash-kpis")!,
    host.querySelector<HTMLElement>("#dash-cta")!,
    serverUrl,
  );
  void loadTop(host.querySelector<HTMLElement>("#dash-top")!, serverUrl);
}

// A freshly-installed panel is mostly zeros — show a next-step banner instead of
// a wall of "$0". Maps the readiness CTA to an in-shell nav jump (clicking the
// existing sidebar button) so there's no dead-end and no new routing surface.
const CTA_NAV: Record<NonNullable<DashboardCta["cta"]>["action"], { nav: string; label: string }> = {
  "seed-demo": { nav: "importar", label: "Cargar productos" },
  "first-sale": { nav: "pos", label: "Abrir POS" },
  config: { nav: "configuracion", label: "Ir a Configuración" },
};

function renderCta(host: HTMLElement, r: DashboardCta): void {
  if (!r.cta) {
    host.hidden = true;
    host.innerHTML = "";
    return;
  }
  const dest = CTA_NAV[r.cta.action];
  host.hidden = false;
  host.innerHTML = `
    <div class="onboard-cta" role="region" aria-label="Primeros pasos">
      <div class="onboard-cta-text">
        <strong>${escapeHtml(r.cta.title)}</strong>
        <p class="muted">${escapeHtml(r.cta.body)}</p>
      </div>
      <button type="button" class="btn-primary onboard-cta-btn" data-goto="${dest.nav}">
        ${escapeHtml(dest.label)}
      </button>
    </div>
  `;
  host.querySelector<HTMLButtonElement>(".onboard-cta-btn")?.addEventListener("click", () => {
    const navBtn = document.querySelector<HTMLButtonElement>(`.nav-item[data-nav="${dest.nav}"]`);
    navBtn?.click();
  });
}

// One combined KPI strip. We fetch sales + inventory in parallel; each tile
// degrades on its own (a failed sales call still lets inventory tiles render).
async function loadKpis(
  host: HTMLElement,
  ctaHost: HTMLElement,
  serverUrl: string,
): Promise<void> {
  const [salesRes, invRes] = await Promise.allSettled([
    salesDaily(serverUrl),
    inventorySummary(serverUrl),
  ]);

  // Onboarding readiness: empty catalog → "cargar productos"; stock but no sale
  // yet → "abrir POS". Only when stats actually loaded (a transient error must
  // not nag). hasSales = any day with orders > 0.
  const productCount = invRes.status === "fulfilled" ? invRes.value.total : null;
  const hasSales =
    salesRes.status === "fulfilled" && salesRes.value.some((r) => r.orders > 0);
  renderCta(ctaHost, dashboardCta({ productCount, hasSales }));

  const tiles: string[] = [];

  // Ventas de hoy (accent — the headline number).
  if (salesRes.status === "fulfilled") {
    const today = pickToday(salesRes.value);
    tiles.push(
      today
        ? kpiCard("Ventas hoy", clp(today.revenue), `${num(today.orders)} venta(s)`, "accent")
        : kpiCard("Ventas hoy", "$0", "sin ventas registradas", "accent"),
    );
  } else {
    tiles.push(kpiCard("Ventas hoy", "—", asMessage(salesRes.reason), "danger"));
  }

  // Inventario: valorización + stock bajo + próximos a vencer.
  if (invRes.status === "fulfilled") {
    const s: InventorySummary = invRes.value;
    tiles.push(kpiCard("Valor inventario", clp(s.inventory_value), `${num(s.total)} productos`));
    tiles.push(
      kpiCard(
        "Stock crítico",
        num(s.low_stock + s.out_of_stock),
        `${num(s.out_of_stock)} agotados · ${num(s.low_stock)} bajos`,
        s.out_of_stock > 0 ? "danger" : s.low_stock > 0 ? "warn" : "",
      ),
    );
    tiles.push(
      kpiCard(
        "Por vencer",
        num(s.expired),
        "lotes vencidos / próximos",
        s.expired > 0 ? "warn" : "",
      ),
    );
  } else {
    tiles.push(kpiCard("Inventario", "—", asMessage(invRes.reason), "danger"));
  }

  host.innerHTML = tiles.join("");
}

/** Most recent row matching today (UTC YYYY-MM-DD); fall back to the last row
 *  so a TZ edge never blanks the headline tile. Mirrors reports.ts. */
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
