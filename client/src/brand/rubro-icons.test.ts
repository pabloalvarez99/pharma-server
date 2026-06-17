import { describe, it, expect } from "vitest";
import { RUBRO_ICONS, rubroIconSvg } from "./rubro-icons";
import { RUBRO_CATALOG } from "../vertical";

// The rubro icon set replaces the OS emoji of the onboarding grid with custom,
// self-hosted line-art (ULTRA-PLAN §4). These are pure-string tests: the markup
// must be inline SVG (offline, no CDN), themeable via `currentColor` so the
// per-rubro accent lights it up, and a11y-hidden (the label names the rubro).

describe("rubroIconSvg — inline, self-hosted, themeable", () => {
  it("returns an inline <svg> for a known icon id", () => {
    const svg = rubroIconSvg("farmacia");
    expect(svg.startsWith("<svg")).toBe(true);
    expect(svg.trim().endsWith("</svg>")).toBe(true);
    expect(svg).toContain('viewBox="0 0 24 24"');
  });

  it("inherits color via currentColor so the rubro accent can theme it", () => {
    const svg = rubroIconSvg("minimarket");
    expect(svg).toContain('stroke="currentColor"');
    // never bake a literal color — theming must flow from CSS `color`.
    expect(svg).not.toMatch(/stroke="#/);
    expect(svg).not.toMatch(/fill="#/);
  });

  it("is decorative: aria-hidden + focus-inert (the visible label names the rubro)", () => {
    const svg = rubroIconSvg("belleza");
    expect(svg).toContain('aria-hidden="true"');
    expect(svg).toContain('focusable="false"');
  });

  it("is offline-safe: no remote refs, scripts or external hrefs (ADR-0005 no CDN)", () => {
    for (const id of Object.keys(RUBRO_ICONS)) {
      const svg = rubroIconSvg(id);
      expect(svg).not.toMatch(/https?:\/\//);
      expect(svg).not.toMatch(/<script/i);
      expect(svg).not.toMatch(/xlink:href|<image|<use/i);
    }
  });

  it("falls back to the generic 'otro' glyph for an unknown/empty id (never empty)", () => {
    const unknown = rubroIconSvg("kiosko");
    expect(unknown.startsWith("<svg")).toBe(true);
    expect(unknown).toBe(rubroIconSvg("otro"));
    expect(rubroIconSvg(null).startsWith("<svg")).toBe(true);
    expect(rubroIconSvg(undefined).startsWith("<svg")).toBe(true);
  });

  it("honours an explicit size override", () => {
    const svg = rubroIconSvg("cafe", { size: 32 });
    expect(svg).toContain('width="32"');
    expect(svg).toContain('height="32"');
  });

  it("ships one glyph per coarse concept, all non-trivial paths", () => {
    // every registered icon must carry real geometry, not an empty stub.
    for (const inner of Object.values(RUBRO_ICONS)) {
      expect(inner.length).toBeGreaterThan(20);
      expect(inner).toMatch(/<(path|circle|rect|line|polyline|polygon)/);
    }
  });
});

describe("every catalog rubro resolves to a real (non-fallback) icon", () => {
  it("RUBRO_ICONS has a glyph for each card.iconId", () => {
    RUBRO_CATALOG.forEach((r) => {
      expect(Object.keys(RUBRO_ICONS)).toContain(r.iconId);
    });
  });

  it("only 'otro' renders the fallback glyph (no accidental collisions)", () => {
    const fallback = rubroIconSvg("__definitely_missing__");
    RUBRO_CATALOG.filter((r) => r.value !== "otro").forEach((r) => {
      expect(rubroIconSvg(r.iconId)).not.toBe(fallback);
    });
  });
});
