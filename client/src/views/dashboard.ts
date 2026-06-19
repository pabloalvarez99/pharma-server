// Dashboard — the owner's home, the first thing they see after login. It must
// read as a produced product surface (not "de dev"): a personal hero header, a
// guided next-step band when the business is fresh, and a KPI strip + top list
// that COMPOSE existing endpoints (no new server calls):
//   ventas de hoy (/reports/sales-daily), valor de inventario + low/out/expiry
//   (/products/stats), top productos (/reports/top-products).
//
// MULTI-RUBRO native: a service rubro (belleza/servicios, physicalStock:false)
// sells without inventory, so it never sees "valor inventario / stock crítico /
// por vencer" — it gets sales-shaped KPIs instead, and its empty state guides
// to the first sale, not to importing products it will never have.
//
// Each region loads independently with its own skeleton so one slow/failed call
// never blanks the rest. Spanish throughout, CLP via ../format. Activation logic
// is single-sourced in onboarding-ux.ts (no divergent copy of the flow here).
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
import { dashboardActivation, type ActivationNav } from "./onboarding-ux";
import {
  loadRubro,
  loadBusinessName,
  featuresForRubro,
  rubroCard,
  type Rubro,
} from "../vertical";
import { rubroIconSvg } from "../brand/rubro-icons";

const TOP_LIMIT = 5;
/** Window (days) for the service-rubro "ventas del período" tile. */
const PERIOD_DAYS = 30;

export function renderDashboard(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-dashboard dash-showcase">
      <header class="dash-hero" id="dash-hero">
        <div class="dash-hero-main">
          <span class="dash-hero-rubro" id="dash-hero-rubro" hidden></span>
          <h2 class="dash-hero-title" id="dash-hero-title">Tu negocio</h2>
          <p class="dash-hero-sub" id="dash-hero-sub">${escapeHtml(todayLine())}</p>
        </div>
        <span class="dash-hero-glow" aria-hidden="true"></span>
      </header>

      <div id="dash-activate" class="dash-activate" hidden></div>

      <div id="dash-kpis" class="kpi-grid dash-kpis">${kpiSkeleton(4)}</div>

      <div class="table-card dash-top-card">
        <div class="card-head">
          <h3 class="section-title" id="dash-top-title">Top ${TOP_LIMIT} productos</h3>
        </div>
        <div id="dash-top">${tableSkeleton(5)}</div>
      </div>
    </section>
  `;

  void hydrate(host, serverUrl);
}

// One orchestrator so the rubro (which decides KPI shape + activation framing)
// is known before we render the data regions. The settings + business-name
// reads are cheap and local; the heavy reports load right after, in parallel.
async function hydrate(host: HTMLElement, serverUrl: string): Promise<void> {
  const [rubro, name] = await Promise.all([
    loadRubro(serverUrl),
    loadBusinessName(serverUrl),
  ]);
  renderHero(host, rubro, name);

  const f = featuresForRubro(rubro);
  await Promise.all([
    loadKpis(
      host.querySelector<HTMLElement>("#dash-kpis")!,
      host.querySelector<HTMLElement>("#dash-activate")!,
      serverUrl,
      rubro,
      f.physicalStock,
    ),
    loadTop(host.querySelector<HTMLElement>("#dash-top")!, serverUrl, f.physicalStock),
  ]);

  // Re-title the top list for service rubros (they rank services, not products).
  const topTitle = host.querySelector<HTMLElement>("#dash-top-title");
  if (topTitle) {
    topTitle.textContent = f.physicalStock
      ? `Top ${TOP_LIMIT} productos`
      : `Top ${TOP_LIMIT} servicios`;
  }
}

// --- Hero header ------------------------------------------------------------

function renderHero(host: HTMLElement, rubro: Rubro, name: string | null): void {
  const card = rubroCard(rubro);
  const rubroEl = host.querySelector<HTMLElement>("#dash-hero-rubro");
  if (rubroEl && card && rubro !== "otro") {
    rubroEl.hidden = false;
    rubroEl.style.setProperty("--rubro-accent", card.accent);
    rubroEl.innerHTML = `${rubroIconSvg(card.iconId, { size: 16, className: "dash-hero-ico" })}<span>${escapeHtml(card.label)}</span>`;
  }
  const titleEl = host.querySelector<HTMLElement>("#dash-hero-title");
  if (titleEl) {
    const who = name ? escapeHtml(name) : "tu negocio";
    titleEl.innerHTML = `${escapeHtml(greeting())}, <span class="dash-hero-name">${who}</span>.`;
  }
}

/** Time-of-day greeting (pure, local clock). */
export function greeting(at: Date = new Date()): string {
  const h = at.getHours();
  if (h < 12) return "Buenos días";
  if (h < 20) return "Buenas tardes";
  return "Buenas noches";
}

/** "Resumen de hoy · martes 17 de junio" — localized, no CDN. */
function todayLine(at: Date = new Date()): string {
  try {
    const f = new Intl.DateTimeFormat("es-CL", {
      weekday: "long",
      day: "numeric",
      month: "long",
    });
    return `Resumen de hoy · ${f.format(at)}`;
  } catch {
    return "Resumen de hoy";
  }
}

// --- Activation band (guided first value) -----------------------------------

function renderActivation(
  host: HTMLElement,
  serverUrl: string,
  rubro: Rubro,
  productCount: number | null,
  hasSales: boolean,
  salesKnown: boolean,
): void {
  void serverUrl; // nav jumps reuse the in-shell sidebar; no fetch here.
  const act = dashboardActivation({ productCount, hasSales, salesKnown, rubro });
  if (!act.cta) {
    host.hidden = true;
    host.innerHTML = "";
    return;
  }
  const cta = act.cta;
  const card = rubroCard(rubro);
  const accent = card && rubro !== "otro" ? card.accent : "";
  const secondary = cta.secondary
    ? `<button type="button" class="dash-activate-link" data-goto="${cta.secondary.nav}">${escapeHtml(cta.secondary.label)}</button>`
    : "";

  host.hidden = false;
  if (accent) host.style.setProperty("--rubro-accent", accent);
  host.innerHTML = `
    <div class="dash-activate-card" role="region" aria-label="Primeros pasos">
      <span class="dash-activate-ico" aria-hidden="true">${activationIcon(act.physicalStock, cta.nav)}</span>
      <div class="dash-activate-text">
        <strong>${escapeHtml(cta.title)}</strong>
        <p>${escapeHtml(cta.body)}</p>
      </div>
      <div class="dash-activate-actions">
        <button type="button" class="btn-primary dash-activate-btn" data-goto="${cta.nav}">
          ${escapeHtml(cta.label)}
        </button>
        ${secondary}
      </div>
    </div>
  `;

  host.querySelectorAll<HTMLButtonElement>("[data-goto]").forEach((btn) => {
    btn.addEventListener("click", () => navJump(btn.dataset.goto as ActivationNav));
  });
}

/** Jump to a sidebar nav item by clicking it (no new routing surface). A target
 *  hidden for the rubro simply no-ops via optional chaining. */
function navJump(nav: ActivationNav): void {
  document.querySelector<HTMLButtonElement>(`.nav-item[data-nav="${nav}"]`)?.click();
}

/** A small contextual glyph for the activation card. */
function activationIcon(physicalStock: boolean, nav: ActivationNav): string {
  const path =
    nav === "pos"
      ? // cart — "haz tu primera venta"
        '<circle cx="9" cy="20" r="1.4"/><circle cx="17.5" cy="20" r="1.4"/><path d="M2.5 3.5H5l2.3 11.1a1.4 1.4 0 0 0 1.4 1.1h8.2a1.4 1.4 0 0 0 1.4-1.1L21 6.5H6"/>'
      : // upload tray — "importa tu catálogo"
        '<path d="M4 16v2.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V16"/><path d="M12 3.5v11"/><path d="m7.5 8 4.5-4.5L16.5 8"/>';
  void physicalStock;
  return `<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${path}</svg>`;
}

// --- KPI strip --------------------------------------------------------------

async function loadKpis(
  host: HTMLElement,
  activateHost: HTMLElement,
  serverUrl: string,
  rubro: Rubro,
  physicalStock: boolean,
): Promise<void> {
  // Product rubros need inventory stats; service rubros never do (no stock).
  const [salesRes, invRes] = await Promise.all([
    settle(salesDaily(serverUrl)),
    physicalStock ? settle(inventorySummary(serverUrl)) : Promise.resolve(null),
  ]);

  const salesKnown = salesRes.ok;
  const hasSales = salesRes.ok && salesRes.value.some((r) => r.orders > 0);
  const productCount =
    invRes && invRes.ok ? invRes.value.total : physicalStock ? null : 0;

  renderActivation(activateHost, serverUrl, rubro, productCount, hasSales, salesKnown);

  host.innerHTML = (physicalStock
    ? productTiles(salesRes, invRes)
    : serviceTiles(salesRes)
  ).join("");
}

/** Product-rubro KPIs: ventas hoy (accent) + inventory health. */
function productTiles(
  salesRes: Settled<DailySalesRow[]>,
  invRes: Settled<InventorySummary> | null,
): string[] {
  const tiles = [salesTodayTile(salesRes)];

  if (invRes && invRes.ok) {
    const s = invRes.value;
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
      kpiCard("Por vencer", num(s.expired), "lotes vencidos / próximos", s.expired > 0 ? "warn" : ""),
    );
  } else if (invRes) {
    tiles.push(kpiCard("Inventario", "—", asMessage(invRes.reason), "danger"));
  }
  return tiles;
}

/** Service-rubro KPIs: pure sales shape — no stock anywhere. */
function serviceTiles(salesRes: Settled<DailySalesRow[]>): string[] {
  const tiles = [salesTodayTile(salesRes, "Ventas hoy")];
  if (salesRes.ok) {
    const rows = salesRes.value;
    const today = pickToday(rows);
    const period = rows.slice(-PERIOD_DAYS);
    // Money fields arrive as STRINGS (see api.DailySalesRow) — coerce to add.
    const periodRevenue = period.reduce((a, r) => a + Number(r.revenue), 0);
    const todayRev = today ? Number(today.revenue) : 0;
    const avg = today && today.orders > 0 ? Math.round(todayRev / today.orders) : 0;
    tiles.push(kpiCard(`Ventas ${PERIOD_DAYS} días`, clp(periodRevenue), `${num(period.length)} día(s)`));
    tiles.push(kpiCard("Servicios hoy", num(today?.orders ?? 0), "cobros del día"));
    tiles.push(kpiCard("Ticket promedio", today && today.orders > 0 ? clp(avg) : "—", "por servicio hoy"));
  } else {
    tiles.push(kpiCard("Ventas", "—", asMessage(salesRes.reason), "danger"));
  }
  return tiles;
}

function salesTodayTile(salesRes: Settled<DailySalesRow[]>, label = "Ventas hoy"): string {
  if (!salesRes.ok) return kpiCard(label, "—", asMessage(salesRes.reason), "danger");
  const today = pickToday(salesRes.value);
  return today
    ? kpiCard(label, clp(today.revenue), `${num(today.orders)} venta(s)`, "accent")
    : kpiCard(label, "$0", "sin ventas registradas", "accent");
}

/** Most recent row matching today (UTC YYYY-MM-DD); fall back to the last row so
 *  a TZ edge never blanks the headline tile. Mirrors reports.ts. */
function pickToday(rows: DailySalesRow[]): DailySalesRow | undefined {
  if (rows.length === 0) return undefined;
  const today = new Date().toISOString().slice(0, 10);
  return rows.find((r) => r.date === today) ?? rows[rows.length - 1];
}

// --- Top list ---------------------------------------------------------------

async function loadTop(
  host: HTMLElement,
  serverUrl: string,
  physicalStock: boolean,
): Promise<void> {
  try {
    const rows: TopProductRow[] = await topProducts(serverUrl, TOP_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">${
        physicalStock ? "Aún no hay ventas para rankear." : "Aún no registras servicios."
      }</p>`;
      return;
    }
    const unitHead = physicalStock ? "Unid." : "Cant.";
    host.innerHTML = `
      <table class="data-table">
        <thead>
          <tr><th>#</th><th>${physicalStock ? "Producto" : "Servicio"}</th><th class="num">${unitHead}</th><th class="num">Ingresos</th><th>ABC</th></tr>
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

// --- tiny Promise.allSettled helper (typed, no `reason: any` leak) -----------

type Settled<T> = { ok: true; value: T } | { ok: false; reason: unknown };

async function settle<T>(p: Promise<T>): Promise<Settled<T>> {
  try {
    return { ok: true, value: await p };
  } catch (reason) {
    return { ok: false, reason };
  }
}
