// Actionable insight layer over the existing report feeds. Reportes used to be
// raw tables; the dueño opens the app daily to learn what to DO, not to read a
// grid. Each insight states qué pasa (a delta, a $ exposure, a stalled SKU) AND
// suggests an action, in owner voice (Spanish, not dev). Pure + tested here so
// the math (deltas, money-at-risk, immobilized stock) is single-sourced and the
// view (reports.ts) only renders — same discipline as reports-helpers.ts.
//
// Money fields arrive as Decimal STRINGS from the server; we parse with
// `toNumber` for arithmetic only and re-format for display with `clp`/`num`.
// Insights are vertical-agnostic (a $ at risk by expiry reads the same for a
// pharmacy lote and a minimarket batch).
import {
  clp,
  num,
  toNumber,
  pctDelta,
  signedPct,
  blendedMarginPct,
  reorderUnits,
  weekdayEs,
} from "../format";
import type {
  DailySalesRow,
  DailyMarginRow,
  StockRotationRow,
  TopProductRow,
  NearExpiryRow,
  Product,
} from "../api";

/** Visual urgency of a card. `danger` = money already lost / bleeding now,
 *  `warn` = exposure or a down trend to act on, `good` = a win to reinforce,
 *  `info` = neutral/not-enough-data, `pro` = a Pro-gated insight (soft upsell). */
export type InsightTone = "danger" | "warn" | "good" | "info" | "pro";

/** One actionable card. `title` = qué pasa (the headline number), `detail` =
 *  the amplifying context, `action` = qué hacer (owner voice). `id` is stable
 *  for keyed rendering / tests. */
export interface Insight {
  id: string;
  icon: string;
  title: string;
  detail: string;
  action: string;
  tone: InsightTone;
}

const HORIZON_DAYS = 30; // "este mes" — near-expiry exposure window.
const COVER_DAYS = 30; // reorder target: keep ~1 month of cover on movers.
const PEAK_MIN_DAYS = 5; // don't call a "peak day" from a handful of rows (noise).

// pctDelta lives in ../format now (the canonical insight-math layer the agent
// will share); re-exported here so existing importers/tests keep their path.
export { pctDelta };

/** Build a `{ id → retail price }` map from the products list, so the
 *  expiry/rotation insights can value units the report feeds only count. A
 *  missing/garbage price contributes 0 rather than NaN. */
export function priceMap(products: readonly Product[]): Map<string, number> {
  const m = new Map<string, number>();
  for (const p of products) m.set(p.id, toNumber(p.price));
  return m;
}

/** Money exposed by expiry, split into "already expired" (sunk loss to write
 *  off) and "at risk within the horizon" (still sellable — act before it
 *  expires). Value = Σ stock × retail price; units = Σ stock; products =
 *  distinct SKUs. A batch with an unknown price still counts in units/products
 *  (so the operator isn't blind) but adds 0 to value. */
export interface ExpiryExposure {
  atRiskValue: number;
  atRiskUnits: number;
  atRiskProducts: number;
  expiredValue: number;
  expiredUnits: number;
  expiredProducts: number;
  horizonDays: number;
}

export function computeExpiryExposure(
  rows: readonly NearExpiryRow[],
  prices: ReadonlyMap<string, number>,
  horizonDays: number = HORIZON_DAYS,
): ExpiryExposure {
  let atRiskValue = 0,
    atRiskUnits = 0,
    expiredValue = 0,
    expiredUnits = 0;
  const atRiskSkus = new Set<string>();
  const expiredSkus = new Set<string>();
  for (const r of rows) {
    if (r.stock <= 0) continue;
    const value = r.stock * (prices.get(r.product_id) ?? 0);
    if (r.expired) {
      expiredValue += value;
      expiredUnits += r.stock;
      expiredSkus.add(r.product_id);
    } else if (r.days_to_expiry <= horizonDays) {
      atRiskValue += value;
      atRiskUnits += r.stock;
      atRiskSkus.add(r.product_id);
    }
  }
  return {
    atRiskValue,
    atRiskUnits,
    atRiskProducts: atRiskSkus.size,
    expiredValue,
    expiredUnits,
    expiredProducts: expiredSkus.size,
    horizonDays,
  };
}

/** Capital frozen in stock that isn't selling: products with zero units sold in
 *  the rotation window but stock on hand. Value = Σ stock × retail price. This
 *  is money the dueño can free by promoting, discounting, or returning. */
export interface StalledStock {
  count: number;
  units: number;
  value: number;
}

export function computeStalledStock(
  rows: readonly StockRotationRow[],
  prices: ReadonlyMap<string, number>,
): StalledStock {
  let count = 0,
    units = 0,
    value = 0;
  for (const r of rows) {
    if (r.qty_sold === 0 && r.current_stock > 0) {
      count += 1;
      units += r.current_stock;
      value += r.current_stock * (prices.get(r.product_id) ?? 0);
    }
  }
  return { count, units, value };
}

/** Replenishment suggestion in money: movers (selling, with on-hand stock) whose
 *  days-of-inventory falls under the cover horizon are about to stock out — every
 *  unit you fail to restock is a sale you won't make. Units = Σ reorderUnits per
 *  product; value = Σ units × retail price (we value at sale price — the cost feed
 *  isn't in the rotation row; it's an order-of-magnitude "cuánto repongo", not a
 *  PO). `top` is the single biggest contributor by value, so the card can say
 *  "compra ~N de «X»" (qué + cuánto), not just a lump sum. */
export interface ReorderNeed {
  count: number;
  units: number;
  value: number;
  topName: string;
  topUnits: number;
  coverDays: number;
}

export function computeReorder(
  rows: readonly StockRotationRow[],
  prices: ReadonlyMap<string, number>,
  coverDays: number = COVER_DAYS,
): ReorderNeed {
  let count = 0,
    units = 0,
    value = 0,
    topName = "",
    topUnits = 0,
    topValue = -1;
  for (const r of rows) {
    if (r.qty_sold <= 0 || r.days_of_inventory == null) continue;
    const doi = toNumber(r.days_of_inventory);
    const buy = reorderUnits(r.current_stock, doi, coverDays);
    if (buy <= 0) continue;
    const v = buy * (prices.get(r.product_id) ?? 0);
    count += 1;
    units += buy;
    value += v;
    if (v > topValue) {
      topValue = v;
      topName = r.product_name;
      topUnits = buy;
    }
  }
  return { count, units, value, topName, topUnits, coverDays };
}

/** Month-over-month margin trend (revenue-weighted), for the latest two calendar
 *  months present in the daily-margin series. Day-over-day margin is noisy; the
 *  dueño steers on the monthly direction. `null` when fewer than two months are
 *  loaded (no honest comparison) or either month has no revenue. */
export interface MarginTrend {
  currMonth: string;
  prevMonth: string;
  currPct: number;
  prevPct: number;
  pts: number;
}

export function computeMarginTrend(rows: readonly DailyMarginRow[]): MarginTrend | null {
  const byMonth = new Map<string, { margin: number; revenue: number }>();
  for (const r of rows) {
    const m = r.date.slice(0, 7); // YYYY-MM
    const acc = byMonth.get(m) ?? { margin: 0, revenue: 0 };
    acc.margin += toNumber(r.margin);
    acc.revenue += toNumber(r.revenue);
    byMonth.set(m, acc);
  }
  const months = [...byMonth.keys()].sort(); // chronological
  if (months.length < 2) return null;
  const currMonth = months[months.length - 1];
  const prevMonth = months[months.length - 2];
  const currPct = blendedMarginPct(byMonth.get(currMonth)!.margin, byMonth.get(currMonth)!.revenue);
  const prevPct = blendedMarginPct(byMonth.get(prevMonth)!.margin, byMonth.get(prevMonth)!.revenue);
  if (currPct == null || prevPct == null) return null;
  return {
    currMonth,
    prevMonth,
    currPct,
    prevPct,
    pts: Math.round((currPct - prevPct) * 10) / 10,
  };
}

/** The weekday that brings in the most revenue across the loaded series, so the
 *  dueño can staff/stock it. Revenue is summed per `Date.getDay()` bucket; the
 *  winner's share of the week is reported too. `null` until there's enough data
 *  (`PEAK_MIN_DAYS` rows with revenue) — a peak from two days is noise, not a
 *  pattern. */
export interface PeakDay {
  weekday: string;
  value: number;
  sharePct: number;
}

export function computePeakDay(rows: readonly DailySalesRow[]): PeakDay | null {
  const withRev = rows.filter((r) => toNumber(r.revenue) > 0);
  if (withRev.length < PEAK_MIN_DAYS) return null;
  const buckets = new Map<string, number>();
  let total = 0;
  for (const r of withRev) {
    const day = weekdayEs(r.date);
    if (!day) continue;
    const rev = toNumber(r.revenue);
    buckets.set(day, (buckets.get(day) ?? 0) + rev);
    total += rev;
  }
  if (total <= 0) return null;
  let weekday = "",
    value = -1;
  for (const [day, v] of buckets) {
    if (v > value) {
      value = v;
      weekday = day;
    }
  }
  return { weekday, value, sharePct: Math.round((value / total) * 1000) / 10 };
}

// --- card builders (pure → tested for tone + copy) ---------------------------

/** Pick today's row and the chronological row immediately before it from a
 *  daily series, so a delta compares like-for-like (today vs the prior day the
 *  server returned). `today` is injectable for deterministic tests. */
function todayAndPrev<T extends { date: string }>(
  rows: readonly T[],
  today: string,
): { curr: T | undefined; prev: T | undefined } {
  if (rows.length === 0) return { curr: undefined, prev: undefined };
  const sorted = [...rows].sort((a, b) => a.date.localeCompare(b.date));
  const i = sorted.findIndex((r) => r.date === today);
  if (i === -1) {
    // Today not in the series (TZ edge): use the last row, prev = the one before.
    return { curr: sorted[sorted.length - 1], prev: sorted[sorted.length - 2] };
  }
  return { curr: sorted[i], prev: sorted[i - 1] };
}

/** "Cuánto vendiste hoy y si vas mejor o peor que ayer." Down → nudge to act;
 *  up → reinforce. No prior day → a calm informational card (no false delta). */
export function salesDeltaInsight(
  rows: readonly DailySalesRow[],
  today: string,
): Insight | null {
  const { curr, prev } = todayAndPrev(rows, today);
  if (!curr) return null;
  const rev = toNumber(curr.revenue);
  if (!prev) {
    return {
      id: "sales-delta",
      icon: "📈",
      title: `Vendiste ${clp(rev)} hoy`,
      detail: `${num(curr.orders)} venta(s). Aún no hay un día previo para comparar.`,
      action: "Mañana verás si subes o bajas respecto a hoy.",
      tone: "info",
    };
  }
  const d = pctDelta(rev, toNumber(prev.revenue));
  if (d === null || d === 0) {
    return {
      id: "sales-delta",
      icon: "📊",
      title: `Vendiste ${clp(rev)} hoy`,
      detail: `Prácticamente igual que el día anterior (${clp(toNumber(prev.revenue))}).`,
      action: "Día estable. Empuja un producto estrella para crecer.",
      tone: "info",
    };
  }
  const up = d > 0;
  const mag = `${Math.abs(d).toFixed(1)}%`;
  return {
    id: "sales-delta",
    icon: up ? "🟢" : "🔻",
    title: up
      ? `Ventas ${mag} arriba vs el día anterior`
      : `Ventas ${mag} abajo vs el día anterior`,
    detail: `Hoy ${clp(rev)} frente a ${clp(toNumber(prev.revenue))} el día previo.`,
    action: up
      ? "Vas mejor. Repite lo que funcionó: vitrina, ofertas, horario."
      : "Revisa vitrina y promociones; ofrece los productos de mayor margen.",
    tone: up ? "good" : "warn",
  };
}

/** "Tu margen subió/bajó N puntos." Pro-only data — call this only when the
 *  margins feed loaded; the gated case is a separate upsell card. Delta is in
 *  percentage POINTS (margin_pct is already a percentage). */
export function marginDeltaInsight(
  rows: readonly DailyMarginRow[],
  today: string,
): Insight | null {
  const { curr, prev } = todayAndPrev(rows, today);
  if (!curr) return null;
  const pct = toNumber(curr.margin_pct);
  if (!prev) {
    return {
      id: "margin-delta",
      icon: "💎",
      title: `Margen de hoy: ${pct.toFixed(1)}%`,
      detail: `${clp(toNumber(curr.margin))} de utilidad sobre la venta.`,
      action: "Aún sin día previo para comparar el margen.",
      tone: "info",
    };
  }
  const prevPct = toNumber(prev.margin_pct);
  const pts = Math.round((pct - prevPct) * 10) / 10;
  if (pts === 0) {
    return {
      id: "margin-delta",
      icon: "💎",
      title: `Margen estable en ${pct.toFixed(1)}%`,
      detail: `Igual que el día anterior (${prevPct.toFixed(1)}%).`,
      action: "Margen sano y constante. Cuida los costos de compra.",
      tone: "info",
    };
  }
  const up = pts > 0;
  return {
    id: "margin-delta",
    icon: up ? "💎" : "⚠️",
    title: up
      ? `Margen subió ${Math.abs(pts).toFixed(1)} pts`
      : `Margen bajó ${Math.abs(pts).toFixed(1)} pts`,
    detail: `Hoy ${pct.toFixed(1)}% vs ${prevPct.toFixed(1)}% el día anterior.`,
    action: up
      ? "Buen trabajo de precios. Mantén la disciplina de costos."
      : "Revisa precios y costos de compra; algo está comiendo tu utilidad.",
    tone: up ? "good" : "warn",
  };
}

/** Soft upsell when márgenes is Pro-locked: no dead-end, one calm card that
 *  names the value the dueño would unlock. */
export function marginGatedInsight(): Insight {
  return {
    id: "margin-gated",
    icon: "🔒",
    title: "Descubre tu margen real con el plan Pro",
    detail: "El análisis de utilidad y su tendencia diaria está en el plan Pro.",
    action: "Activa Pro para saber cuánto ganas de verdad en cada venta.",
    tone: "pro",
  };
}

/** Money already lost to expiry — a sunk write-off the dueño must record. */
export function expiredInsight(x: ExpiryExposure): Insight | null {
  if (x.expiredUnits <= 0) return null;
  const money = x.expiredValue > 0 ? clp(x.expiredValue) : `${num(x.expiredUnits)} unidad(es)`;
  return {
    id: "expired",
    icon: "🛑",
    title: `${money} ya vencido`,
    detail: `${num(x.expiredUnits)} unidad(es) en ${num(x.expiredProducts)} producto(s) pasaron su vencimiento.`,
    action: "Da de baja el stock vencido y ajústalo en inventario.",
    tone: "danger",
  };
}

/** Money still sellable but at risk inside the horizon — act before it expires. */
export function nearExpiryInsight(x: ExpiryExposure): Insight | null {
  if (x.atRiskUnits <= 0) return null;
  const money = x.atRiskValue > 0 ? clp(x.atRiskValue) : `${num(x.atRiskUnits)} unidad(es)`;
  return {
    id: "near-expiry",
    icon: "⏳",
    title: `${money} en riesgo por vencer`,
    detail: `${num(x.atRiskUnits)} unidad(es) en ${num(x.atRiskProducts)} producto(s) vencen dentro de ${num(x.horizonDays)} días.`,
    action: "Liquida con oferta o devuelve al proveedor antes de perderlo.",
    tone: "warn",
  };
}

/** Capital frozen in stock that doesn't move. */
export function stalledInsight(s: StalledStock): Insight | null {
  if (s.count <= 0) return null;
  const money = s.value > 0 ? clp(s.value) : `${num(s.units)} unidad(es)`;
  return {
    id: "stalled",
    icon: "🧊",
    title: `${money} en stock sin rotación`,
    detail: `${num(s.count)} producto(s) con stock no registraron ventas en el período.`,
    action: "Ofértalos, reubícalos en vitrina o deja de reponerlos.",
    tone: "warn",
  };
}

/** Positive reinforcement: the product carrying the day. */
export function topSellerInsight(rows: readonly TopProductRow[]): Insight | null {
  const first = rows.find((r) => r.rank === 1) ?? rows[0];
  if (!first) return null;
  return {
    id: "top-seller",
    icon: "⭐",
    title: `Tu estrella: ${first.product_name}`,
    detail: `${num(first.qty_sold)} unidad(es) · ${clp(toNumber(first.revenue))} (${first.revenue_pct}% de tus ingresos).`,
    action: "Asegura su stock y dale el mejor lugar en la vitrina.",
    tone: "good",
  };
}

/** Replenishment suggestion: how much money to put back into stock, and the one
 *  product to start with. Actionable money the dueño leaves on the table by
 *  stocking out. */
export function reorderInsight(n: ReorderNeed): Insight | null {
  if (n.count <= 0 || n.units <= 0) return null;
  const money = n.value > 0 ? clp(n.value) : `${num(n.units)} unidad(es)`;
  const lead =
    n.topName && n.topUnits > 0
      ? `Empieza por ${num(n.topUnits)} de «${n.topName}».`
      : "Prioriza los de mayor venta.";
  return {
    id: "reorder",
    icon: "🛒",
    title: `Repón ${money} para no quebrar stock`,
    detail: `${num(n.count)} producto(s) que se venden se agotan en menos de ${num(n.coverDays)} días.`,
    action: `${lead} Cada quiebre es una venta perdida.`,
    tone: "warn",
  };
}

/** Monthly margin direction (Pro): the trend the dueño steers on, mes vs mes. */
export function marginTrendInsight(t: MarginTrend | null): Insight | null {
  if (!t || t.pts === 0) return null;
  const up = t.pts > 0;
  return {
    id: "margin-trend",
    icon: up ? "📈" : "📉",
    title: `Margen mensual ${signedPct(t.pts)} vs el mes pasado`,
    detail: `Este mes ${t.currPct.toFixed(1)}% frente a ${t.prevPct.toFixed(1)}% el mes anterior.`,
    action: up
      ? "Buena tendencia. Mantén la disciplina de precios y compras."
      : "Tu utilidad mensual cae: renegocia costos o ajusta precios de los productos de más venta.",
    tone: up ? "good" : "warn",
  };
}

/** Peak sales day of the week — where to concentrate personal y stock. */
export function peakDayInsight(p: PeakDay | null): Insight | null {
  if (!p || !p.weekday) return null;
  return {
    id: "peak-day",
    icon: "🗓️",
    title: `Tu día más fuerte: ${p.weekday}`,
    detail: `Concentra ${p.sharePct.toFixed(1)}% de tus ventas (${clp(p.value)}).`,
    action: "Refuerza personal, caja y stock ese día; programa ahí tus promos.",
    tone: "good",
  };
}

/** Everything the insight strip needs. `margins=null` + `marginsGated=true`
 *  renders the Pro upsell; `margins` present renders the real margin delta. */
export interface InsightInputs {
  sales: readonly DailySalesRow[];
  margins: readonly DailyMarginRow[] | null;
  marginsGated: boolean;
  nearExpiry: readonly NearExpiryRow[];
  rotation: readonly StockRotationRow[];
  top: readonly TopProductRow[];
  prices: ReadonlyMap<string, number>;
  today?: string;
  horizonDays?: number;
}

/** Compose the ordered insight strip: most urgent first (money lost → money at
 *  risk → trends → frozen capital → wins). Cards that don't apply (no data)
 *  are omitted. An empty result means "not enough data yet" — the view shows a
 *  single calm placeholder rather than a wall of nothing. */
export function buildInsights(inp: InsightInputs): Insight[] {
  const today = inp.today ?? new Date().toISOString().slice(0, 10);
  const exposure = computeExpiryExposure(inp.nearExpiry, inp.prices, inp.horizonDays);
  const stalled = computeStalledStock(inp.rotation, inp.prices);
  const reorder = computeReorder(inp.rotation, inp.prices);
  const margin = inp.margins
    ? marginDeltaInsight(inp.margins, today)
    : inp.marginsGated
      ? marginGatedInsight()
      : null;
  // Monthly margin trend only with real (Pro) data — never from the gated stub.
  const marginTrend = inp.margins ? marginTrendInsight(computeMarginTrend(inp.margins)) : null;
  const cards = [
    expiredInsight(exposure), // money already lost
    nearExpiryInsight(exposure), // money at risk by expiry
    reorderInsight(reorder), // money lost to stock-outs (act: buy)
    salesDeltaInsight(inp.sales, today), // day-over-day pulse
    margin, // today's margin (or Pro upsell)
    marginTrend, // monthly margin direction (Pro)
    stalledInsight(stalled), // frozen capital (dead stock)
    peakDayInsight(computePeakDay(inp.sales)), // when to push
    topSellerInsight(inp.top), // the win to reinforce
  ];
  return cards.filter((c): c is Insight => c !== null);
}
