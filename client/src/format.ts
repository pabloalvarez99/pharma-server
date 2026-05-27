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
