// Rubro icon set — custom, self-hosted line-art for the onboarding "elige tu
// rubro" showcase (docs/strategy/rubro-select-experience.md §4). Replaces the OS
// emoji of `RubroCard.icon`, which depend on the platform font, render
// inconsistently and read as "dev placeholder". These are inline SVG strings:
//
//   - OFFLINE-FIRST (ADR-0005): no CDN, no <use>/sprite fetch, no web fonts. The
//     markup is the whole icon; it works with the network unplugged.
//   - THEMEABLE: stroke="currentColor" so the per-rubro `--rubro-accent` (set on
//     the card) lights the glyph up on hover/focus/selection via plain CSS color.
//   - DECORATIVE: aria-hidden + focusable=false; the visible label names the
//     rubro, so the icon must stay out of the a11y tree and the tab order.
//
// Every glyph lives on a 24×24 grid, 1.75px stroke, round caps/joins, no fill —
// one consistent line-art family so the grid reads as a designed set, not a
// rummage of clip-art. The map key is the card's `iconId` (see vertical.ts).

/** Inner SVG markup (no wrapper) for each rubro icon id. */
export const RUBRO_ICONS: Readonly<Record<string, string>> = {
  // Farmacia — a tilted capsule with its center seam. Reads as "medicamento".
  farmacia:
    '<g transform="rotate(-45 12 12)"><rect x="3" y="8.5" width="18" height="7" rx="3.5"/><line x1="12" y1="8.5" x2="12" y2="15.5"/></g>',
  // Minimarket — shopping cart (POS por código de barras, abarrotes).
  minimarket:
    '<circle cx="9" cy="20" r="1.25"/><circle cx="17.5" cy="20" r="1.25"/><path d="M2.5 3.5H5l2.3 11.1a1.4 1.4 0 0 0 1.4 1.1h8.2a1.4 1.4 0 0 0 1.4-1.1L21 6.5H6"/>',
  // Restaurant — fork + knife (insumos de cocina).
  restaurant:
    '<line x1="7" y1="3" x2="7" y2="21"/><line x1="4.5" y1="3" x2="4.5" y2="7.5"/><line x1="9.5" y1="3" x2="9.5" y2="7.5"/><path d="M4.5 7.5a2.5 2.5 0 0 0 5 0"/><path d="M17 3c-1.6 2-1.6 7.5 0 9.5"/><line x1="17" y1="12.5" x2="17" y2="21"/>',
  // Café — taza con vapor (pastelería, perecibles).
  cafe:
    '<path d="M5 8h11v5a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4z"/><path d="M16 9h2.5a2.5 2.5 0 0 1 0 5H16"/><line x1="8" y1="2.5" x2="8" y2="4.5"/><line x1="12" y1="2.5" x2="12" y2="4.5"/>',
  // Tienda — bolsa de compras con asa (retail, POS + inventario).
  tienda:
    '<path d="M6 8h12l-1 11.2a1 1 0 0 1-1 .8H8a1 1 0 0 1-1-.8z"/><path d="M9 8V6a3 3 0 0 1 6 0v2"/>',
  // Belleza — destello/sparkle (servicios de estética).
  belleza:
    '<path d="M12 3c.45 4.6 1.4 5.55 6 6-4.6.45-5.55 1.4-6 6-.45-4.6-1.4-5.55-6-6 4.6-.45 5.55-1.4 6-6z"/><path d="M18.7 14c.2 1.55.55 1.9 2.1 2.1-1.55.2-1.9.55-2.1 2.1-.2-1.55-.55-1.9-2.1-2.1 1.55-.2 1.9-.55 2.1-2.1z"/>',
  // Servicios — llave (oficios, venta sin inventario).
  servicios:
    '<path d="M15.6 4.4a4.5 4.5 0 0 1-5.85 5.85l-5.2 5.2a2.05 2.05 0 1 0 2.9 2.9l5.2-5.2A4.5 4.5 0 0 0 18.5 7.3l-2.55 2.55-2.2-.6-.6-2.2z"/>',
  // Otro — grilla 2×2 (ERP genérico, "cualquier negocio"). Also the fallback.
  otro:
    '<rect x="4" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5"/>',
};

/** The id used when a rubro has no icon of its own — the generic ERP glyph. */
const FALLBACK_ICON_ID = "otro";

/** Render a rubro icon as an inline, self-contained `<svg>` string. Unknown or
 *  empty ids fall back to the generic {@link FALLBACK_ICON_ID} glyph so a card
 *  never renders blank. The glyph inherits `currentColor`; size defaults to the
 *  24-grid unit and can be overridden for the larger preview header. */
export function rubroIconSvg(
  iconId: string | null | undefined,
  opts: { size?: number; className?: string } = {},
): string {
  const size = opts.size ?? 24;
  const cls = opts.className ? ` class="${opts.className}"` : "";
  const inner = RUBRO_ICONS[(iconId ?? "").trim()] ?? RUBRO_ICONS[FALLBACK_ICON_ID];
  return (
    `<svg${cls} width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" ` +
    `stroke="currentColor" stroke-width="1.75" stroke-linecap="round" ` +
    `stroke-linejoin="round" aria-hidden="true" focusable="false">${inner}</svg>`
  );
}
