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
import { clp, num, toNumber } from "../format";
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

/** Signed percentage change `(curr-prev)/prev*100`, rounded to 1 decimal.
 *  `null` when `prev <= 0` (no meaningful base — never divide by zero, never
 *  report a "+∞%" jump from nothing). */
export function pctDelta(curr: number, prev: number): number | null {
  if (!(prev > 0)) return null;
  return Math.round(((curr - prev) / prev) * 1000) / 10;
}

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
  const margin = inp.margins
    ? marginDeltaInsight(inp.margins, today)
    : inp.marginsGated
      ? marginGatedInsight()
      : null;
  const cards = [
    expiredInsight(exposure),
    nearExpiryInsight(exposure),
    salesDeltaInsight(inp.sales, today),
    margin,
    stalledInsight(stalled),
    topSellerInsight(inp.top),
  ];
  return cards.filter((c): c is Insight => c !== null);
}
