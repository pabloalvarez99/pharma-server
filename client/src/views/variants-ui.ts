// Light multi-SKU variants UX helpers (pure, no DOM).
//
// Server (B): product parent + children with parent_id, GET by-barcode,
// GET/POST .../variants. Full tallaxcolor matrix UI is out of scope — inventory
// shows a banner + list; POS scans child barcodes and refuses selling the parent.

/** Operator banner when a catalog product is a multi-SKU parent. */
export function variantsParentBanner(count: number): string {
  const n = Math.max(0, Math.trunc(count));
  if (n <= 0) return "";
  const label = n === 1 ? "1 variante" : `${n} variantes`;
  return `Tiene ${label} · vender por código de barras del hijo (talla/SKU). No cobres el padre en el POS.`;
}

/** Error when the cashier tries to ring the parent instead of a child SKU. */
export function parentWithVariantsError(productName: string): string {
  const name = (productName || "este producto").trim() || "este producto";
  return `«${name}» tiene variantes. Escanea el código de barras de la talla/SKU o elige la variante.`;
}

/** True when a sale-error message is the domain parent-has-variants guard. */
export function isParentHasVariantsMessage(msg: string): boolean {
  const low = (msg || "").toLowerCase();
  return low.includes("tiene variantes") || low.includes("escanee el código");
}

/** Short attrs label for a variant row (talla M · color Negro). */
export function variantAttrsLabel(attrs: Record<string, unknown> | null | undefined): string {
  if (!attrs || typeof attrs !== "object") return "";
  const parts: string[] = [];
  for (const key of ["talla", "color", "sku"]) {
    const v = attrs[key];
    if (v != null && String(v).trim() !== "") parts.push(String(v).trim());
  }
  if (parts.length === 0) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || String(v).trim() === "") continue;
      parts.push(`${k}: ${String(v).trim()}`);
      if (parts.length >= 3) break;
    }
  }
  return parts.join(" · ");
}

/**
 * Decide POS Enter path intent: prefer barcode lookup when the typed value
 * looks like a scan (mostly digits, length ≥ 4) rather than a name search hit.
 * Name searches still work via the existing first-result path when this is false
 * OR when by-barcode 404s.
 */
export function preferBarcodeLookup(raw: string): boolean {
  const s = (raw ?? "").trim();
  if (s.length < 4) return false;
  // Pure digits (EAN-8/13/UPC) or digit-heavy codes with few separators.
  const digits = s.replace(/\D/g, "");
  if (digits.length >= 8 && digits.length >= s.length - 2) return true;
  // Explicit barcode-ish: long alphanumeric without spaces (internal SKUs).
  if (s.length >= 6 && !/\s/.test(s) && /^[A-Za-z0-9\-_.]+$/.test(s)) return true;
  return false;
}
