// Money + number formatting. Server money arrives as a STRING (Decimal,
// `rust_decimal::serde::str`) — we parse only for display/arithmetic in the
// POS cart and NEVER round-trip a float back to the server (the cart re-emits
// the original product `price` string verbatim as `unit_price`).

const CLP = new Intl.NumberFormat("es-CL", {
  style: "currency",
  currency: "CLP",
  maximumFractionDigits: 0,
});

/** Format a CLP amount. Accepts a Decimal-as-string (from the server) or a
 *  number (from local cart math). Non-numeric input renders as `$0`. */
export function clp(value: string | number): string {
  const n = typeof value === "number" ? value : Number(value);
  return CLP.format(Number.isFinite(n) ? n : 0);
}

/** Parse a server Decimal string to a JS number for local arithmetic. Returns
 *  0 on garbage rather than NaN so cart totals never read "NaN". */
export function toNumber(value: string | number): number {
  const n = typeof value === "number" ? value : Number(value);
  return Number.isFinite(n) ? n : 0;
}

const NUM = new Intl.NumberFormat("es-CL");

/** Thousands-separated integer (es-CL grouping). */
export function num(value: number): string {
  return NUM.format(value);
}

// --- dates -------------------------------------------------------------------
// Server timestamps arrive as RFC3339 strings (`DateTime<Utc>`). Every list/row
// that shows a date — inventory lotes, compras OC + pagos, gastos egresos —
// renders through ONE formatter so the operator never sees `16-06-2026` in one
// view and `16-06-26` in the next. es-CL with a full 4-digit year (`dd-mm-yyyy`):
// unambiguous on compliance-adjacent docs and matches the CL convention.
const FECHA = new Intl.DateTimeFormat("es-CL", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});

/** Canonical operator-facing date (`dd-mm-yyyy`, es-CL) from an RFC3339/parseable
 *  string. Unparseable input is echoed verbatim rather than rendering "Invalid
 *  Date" — never hide a real value behind a formatter failure. */
export function fecha(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : FECHA.format(d);
}

// --- POS cash math -----------------------------------------------------------
// Pure helpers behind the POS checkout. Extracted so the vuelto/quick-cash logic
// (the cashier-loop money path) is regression-tested without a DOM. CLP has no
// minor unit, so everything here is integer pesos.

/** Parse a free-typed CLP amount ("$10.000", "10000") to a non-negative integer.
 *  Strips every non-digit (so a stray "-" can't yield a negative tender) and
 *  floors garbage/empty to 0 — the cart total never reads "NaN". */
export function parseCash(raw: string): number {
  const n = Number(raw.replace(/[^\d]/g, ""));
  return Number.isFinite(n) && n > 0 ? Math.trunc(n) : 0;
}

/** Cash to hand the server for a single-tender cash sale: the tendered amount
 *  when it covers the total, otherwise the exact total — so the server's balance
 *  check passes and it computes a non-negative authoritative vuelto. Never sends
 *  less than the total (a short tender would be rejected). */
export function effectiveTender(received: number, total: number): number {
  return received >= total ? received : total;
}

/** Live vuelto preview for a cash sale.
 *  - `ok`    → received covers total; `amount` is the change due.
 *  - `short` → received falls short; `amount` is the shortfall (positive).
 *  - `none`  → nothing to show yet (no cash typed, or empty cart).
 *  `amount` is always non-negative; the sign lives in `kind`, never in the
 *  number — so the UI can't accidentally render a negative vuelto. */
export function vuelto(
  received: number,
  total: number,
): { kind: "ok" | "short" | "none"; amount: number } {
  if (received <= 0 || total <= 0) return { kind: "none", amount: 0 };
  return received >= total
    ? { kind: "ok", amount: received - total }
    : { kind: "short", amount: total - received };
}

/** Quick-cash chip denominations for a total: the exact amount first, then the
 *  next round 1k / 5k / 10k bill at or above it. Deduped, positive, ascending,
 *  capped at 4. Empty for a non-positive total. */
export function quickCashAmounts(total: number): number[] {
  if (total <= 0) return [];
  return Array.from(
    new Set([
      Math.round(total),
      Math.ceil(total / 1000) * 1000,
      Math.ceil(total / 5000) * 5000,
      Math.ceil(total / 10000) * 10000,
    ]),
  )
    .filter((a) => a > 0)
    .slice(0, 4);
}

// --- IVA CL 19% --------------------------------------------------------------
// Mirrors the server's `crates/dte/src/emit.rs::desglose_iva` EXACTLY so the
// cashier's live preview matches the amounts the server will stamp on the DTE:
// from an IVA-included affected total, neto = round(afecto / 1.19) half-away-
// from-zero and the IVA absorbs the rounding (neto + iva == afecto). Computed as
// `afecto * 100 / 119` to dodge the float representation of 1.19; afecto is a
// non-negative integer CLP amount so `Math.round` (ties toward +∞) equals the
// server's half-away-from-zero. The cross-repo parity is locked by tests that
// reuse the server's own vectors.

/** Split an IVA-included affected total (integer CLP) into `{ neto, iva }`. */
export function desgloseIva(afecto: number): { neto: number; iva: number } {
  const neto = Math.round((afecto * 100) / 119);
  return { neto, iva: afecto - neto };
}

// --- RUT chileno (módulo 11) -------------------------------------------------
// El SII identifica al receptor por RUT con dígito verificador mód-11. Validamos
// en el cliente para no emitir un DTE con un RUT mal tipeado (el server lo
// almacena verbatim en <RUTRecep>). Formato canónico de envío: `NNNNNNNN-D`
// (sin puntos, guion, K mayúscula); el eco visual usa puntos `NN.NNN.NNN-D`.

/** Strip dots/dash/spaces and upper-case the verifier (`76.123.456-k` → `76123456K`). */
export function cleanRut(raw: string): string {
  return raw.replace(/[.\-\s]/g, "").toUpperCase();
}

/** Mód-11 verifier digit for a numeric body (`"76123456"` → `"7"`). */
export function rutDigitVerifier(body: string): string {
  let sum = 0;
  let mul = 2;
  for (let i = body.length - 1; i >= 0; i--) {
    sum += Number(body[i]) * mul;
    mul = mul === 7 ? 2 : mul + 1;
  }
  const res = 11 - (sum % 11);
  if (res === 11) return "0";
  if (res === 10) return "K";
  return String(res);
}

/** True when `raw` is a structurally valid Chilean RUT with a matching DV.
 *  Accepts a 7–8 digit body (the practical range for personas/empresas). */
export function isValidRut(raw: string): boolean {
  const c = cleanRut(raw);
  if (!/^\d{7,8}[\dK]$/.test(c)) return false;
  const body = c.slice(0, -1);
  return rutDigitVerifier(body) === c.slice(-1);
}

/** Canonical SII form `NNNNNNNN-D` (no dots) for the wire. Returns the cleaned
 *  input unchanged when it is not a parseable body+DV. */
export function canonicalRut(raw: string): string {
  const c = cleanRut(raw);
  if (!/^\d{1,8}[\dK]$/.test(c)) return c;
  return `${c.slice(0, -1)}-${c.slice(-1)}`;
}

/** Pretty form `NN.NNN.NNN-D` for visual echo. Returns the trimmed input when
 *  it cannot be split into body+DV. */
export function formatRut(raw: string): string {
  const c = cleanRut(raw);
  if (!/^\d{1,8}[\dK]$/.test(c)) return raw.trim();
  const body = c.slice(0, -1);
  const dv = c.slice(-1);
  return `${num(Number(body))}-${dv}`;
}

// --- insight math (canonical) ------------------------------------------------
// The ONE source of the numbers behind any "qué hacer" surface. Reportes
// (reports-insights.ts) computes its cards from here TODAY; the agent (milton/ye)
// will answer "¿cuánto compro?", "¿cómo va mi margen?", "¿qué día vendo más?"
// from the SAME functions TOMORROW — so a card and a chat reply never disagree by
// a rounding rule. Pure number-in/number-out (no row types, no DOM): generic,
// vertical-agnostic, trivially testable. Money arrives as Decimal strings → parse
// with `toNumber` at the edge, never round-trip a float to the server.

/** Signed percentage change `(curr-prev)/prev*100`, rounded to 1 decimal.
 *  `null` when `prev <= 0` (no meaningful base — never divide by zero, never
 *  report a "+∞%" jump from nothing). The canonical trend primitive. */
export function pctDelta(curr: number, prev: number): number | null {
  if (!(prev > 0)) return null;
  return Math.round(((curr - prev) / prev) * 1000) / 10;
}

/** Render a delta as a signed percent string (`10` → `"+10,0%"`, `-8` →
 *  `"−8,0%"`), es-CL decimal comma + a real minus sign (U+2212, not a hyphen) so
 *  it reads cleanly in a sentence. The sign lives in the string, the magnitude in
 *  the number — a card never has to re-derive the direction. */
export function signedPct(d: number): string {
  const sign = d >= 0 ? "+" : "−";
  return `${sign}${Math.abs(d).toFixed(1).replace(".", ",")}%`;
}

/** Blended (revenue-weighted) gross-margin percent across a set of days/lines:
 *  `Σmargin / Σrevenue * 100`, rounded to 1 decimal. Weighting by revenue (not a
 *  naïve average of daily percentages) is the only honest way to roll a margin up
 *  to a month — a $1M day at 30% and a $1k day at 60% blend near 30%, not 45%.
 *  `null` when total revenue ≤ 0 (no base to take a percentage of). */
export function blendedMarginPct(totalMargin: number, totalRevenue: number): number | null {
  if (!(totalRevenue > 0)) return null;
  return Math.round((totalMargin / totalRevenue) * 1000) / 10;
}

/** Units to BUY now so on-hand stock covers `coverDays` more days at the current
 *  sell-through rate. The run-rate is read off `daysOfInventory` (how many days
 *  the current stock lasts) so no external window has to be threaded through:
 *  rate = currentStock / daysOfInventory, target = rate × coverDays, buy =
 *  ceil(target − currentStock). Returns 0 when the product isn't selling
 *  (`daysOfInventory ≤ 0`, not computable) or already covers the horizon — we
 *  never suggest buying what isn't moving (that's the dead-stock surface, not
 *  this one). Always a non-negative integer (you can't buy a fraction of a box). */
export function reorderUnits(
  currentStock: number,
  daysOfInventory: number,
  coverDays: number,
): number {
  if (!(daysOfInventory > 0) || !(currentStock > 0) || !(coverDays > 0)) return 0;
  const runRate = currentStock / daysOfInventory;
  const buy = Math.ceil(runRate * coverDays - currentStock);
  return buy > 0 ? buy : 0;
}

/** Day-of-week names (es-CL, 0 = domingo … 6 = sábado) — the index `Date.getDay()`
 *  returns, so a "tu día más fuerte" insight can name the peak day. */
export const WEEKDAYS_ES = [
  "domingo",
  "lunes",
  "martes",
  "miércoles",
  "jueves",
  "viernes",
  "sábado",
] as const;

/** Weekday name for an `YYYY-MM-DD` date, parsed at local midnight so the day
 *  doesn't shift under a UTC offset. Returns `""` for an unparseable date rather
 *  than "Invalid Date". */
export function weekdayEs(date: string): string {
  const d = new Date(`${date}T00:00:00`);
  return Number.isNaN(d.getTime()) ? "" : WEEKDAYS_ES[d.getDay()];
}
