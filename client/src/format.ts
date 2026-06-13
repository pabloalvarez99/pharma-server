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
