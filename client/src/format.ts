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
