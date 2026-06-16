// Rubro-select showcase — pure preview/gating model matrix (P4, no DOM).
//
// The rubro configurator (docs/strategy/rubro-select-experience.md) is THE
// vitrina of RutBusiness multi-rubro. configuracion.ts + first-run render the
// live "Vista previa de tu ERP" from these pure functions, so locking the model
// across ALL 8 catalog rubros guarantees: the preview is correct per rubro, both
// gated verticals map to a real seed pack, service rubros sell without stock, the
// SII boleta/factura is universal, recetas stay farmacia-only, and NO rubro is a
// dead-end. (Keyboard/a11y of the actual grid is covered in the .dom test.)
import { describe, it, expect } from "vitest";
import {
  RUBRO_CATALOG,
  parseRubro,
  featuresForRubro,
  seedVerticalFor,
  type Rubro,
} from "../src/vertical";
import {
  rubroPreview,
  visibleModulesForRubro,
  ALL_MODULES,
} from "../src/views/first-run";

const ALL_RUBROS = RUBRO_CATALOG.map((r) => r.value as Rubro);
const SERVICE_RUBROS: Rubro[] = ["belleza", "servicios"];

describe("rubro catalog — the 8-card showcase grid", () => {
  it("ships exactly the documented 8 rubros, no emoji-only fallbacks missing data", () => {
    expect(ALL_RUBROS).toEqual([
      "farmacia",
      "minimarket",
      "restaurant",
      "cafe",
      "tienda",
      "belleza",
      "servicios",
      "otro",
    ]);
  });

  it("every card carries the showcase fields the preview/panel render (accent, tagline, iconId)", () => {
    for (const c of RUBRO_CATALOG) {
      expect(c.accent, `${c.value} accent`).toMatch(/^#[0-9a-f]{6}$/i);
      expect(c.tagline.length, `${c.value} tagline`).toBeGreaterThan(0);
      expect(c.iconId, `${c.value} iconId`).toBe(c.value);
      expect(c.label.length, `${c.value} label`).toBeGreaterThan(0);
    }
  });
});

describe("cero dead-end — every rubro is selectable and previews cleanly", () => {
  it("parseRubro round-trips every catalog value (no card folds away on select)", () => {
    for (const r of ALL_RUBROS) expect(parseRubro(r)).toBe(r);
  });

  it("unknown / unset values fall back to the generic ERP, never crash", () => {
    expect(parseRubro(undefined)).toBe("otro");
    expect(parseRubro("")).toBe("otro");
    expect(parseRubro("no-existe")).toBe("otro");
  });

  it("every rubro yields a non-empty, coherent live preview", () => {
    for (const r of ALL_RUBROS) {
      const p = rubroPreview(r);
      expect(p.categories.length, `${r} categories`).toBe(4);
      expect(p.totalCount).toBe(ALL_MODULES.length);
      expect(p.visibleCount).toBeGreaterThan(0);
      expect(p.visibleCount).toBeLessThanOrEqual(p.totalCount);
    }
  });

  it("a rubro with no demo pack still previews (Próximamente ≠ dead-end)", () => {
    // restaurant/cafe/tienda/belleza/servicios/otro have no seed pack today.
    const p = rubroPreview("restaurant");
    expect(seedVerticalFor("restaurant")).toBeNull();
    expect(p.hasDemo).toBe(false);
    expect(p.visibleCount).toBeGreaterThan(0); // a real, usable ERP regardless
  });
});

describe("ambos verticales gated — pharmacy + minimarket map to a seed pack", () => {
  it("farmacia → pharmacy pack, minimarket → minimarket pack", () => {
    expect(seedVerticalFor("farmacia")).toBe("pharmacy");
    expect(seedVerticalFor("minimarket")).toBe("minimarket");
    expect(rubroPreview("farmacia").hasDemo).toBe(true);
    expect(rubroPreview("minimarket").hasDemo).toBe(true);
  });

  it("farmacia preview is the maximal ERP: recetas + clínica + lotes native", () => {
    const p = rubroPreview("farmacia");
    expect(p.native).toContain("Recetas y Libro de controlados (Ley 20.000)");
    expect(p.native).toContain(
      "Ficha clínica: principio activo, laboratorio, interacciones",
    );
    expect(p.native).toContain("Lotes y vencimiento para productos perecibles");
    expect(p.hidden).toEqual([]); // farmacia hides nothing notable
    expect(featuresForRubro("farmacia")).toEqual({
      recetas: true,
      lotes: true,
      physicalStock: true,
      clinical: true,
    });
  });

  it("minimarket: lotes (perecibles) but NO recetas/clínica leak from pharmacy", () => {
    const p = rubroPreview("minimarket");
    expect(p.native).toContain("Lotes y vencimiento para productos perecibles");
    expect(p.native).not.toContain("Recetas y Libro de controlados (Ley 20.000)");
    expect(p.native.some((n) => n.includes("clínica"))).toBe(false);
    expect(p.hidden).toContain("Recetas"); // recetas is intentionally absent
    expect(featuresForRubro("minimarket").recetas).toBe(false);
    expect(featuresForRubro("minimarket").clinical).toBe(false);
  });
});

describe("recetas / controlados (Ley 20.000) — farmacia ONLY", () => {
  it("only farmacia turns recetas on; every other rubro hides it", () => {
    for (const r of ALL_RUBROS) {
      const on = featuresForRubro(r).recetas;
      expect(on, `${r} recetas`).toBe(r === "farmacia");
      const visible = visibleModulesForRubro(r);
      expect(visible.includes("recetas"), `${r} nav recetas`).toBe(r === "farmacia");
      if (r !== "farmacia") expect(rubroPreview(r).hidden).toContain("Recetas");
    }
  });
});

describe("boleta/factura SII — UNIVERSAL across every rubro", () => {
  it("every rubro shows the Compliance category on + keeps boletas/facturas nav", () => {
    for (const r of ALL_RUBROS) {
      const p = rubroPreview(r);
      const compliance = p.categories.find(
        (c) => c.label === "Boletas y facturas (SII)",
      )!;
      expect(compliance.on, `${r} SII category`).toBe(true);
      const visible = visibleModulesForRubro(r);
      expect(visible, `${r} boletas`).toContain("boletas");
      expect(visible, `${r} facturas`).toContain("facturas");
      // Reportes + POS/caja are universal too — every CL business sells + reports.
      expect(visible).toContain("pos");
      expect(visible).toContain("reports");
    }
  });
});

describe("servicio sin stock — the agnostic-core proof (belleza/servicios)", () => {
  it("service rubros sell with NO physical stock: inventario/compras hidden", () => {
    for (const r of SERVICE_RUBROS) {
      const f = featuresForRubro(r);
      expect(f.physicalStock, `${r} physicalStock`).toBe(false);
      expect(f.lotes, `${r} lotes`).toBe(false);
      const visible = visibleModulesForRubro(r);
      expect(visible, `${r} inventory hidden`).not.toContain("inventory");
      expect(visible, `${r} compras hidden`).not.toContain("compras");
      // ...but it can still SELL and emit a boleta.
      expect(visible).toContain("pos");
      expect(visible).toContain("boletas");

      const p = rubroPreview(r);
      const inv = p.categories.find((c) => c.label === "Inventario y compras")!;
      expect(inv.on, `${r} inventory category`).toBe(false);
      expect(p.native).toContain("Venta de servicios sin inventario ni lotes");
      expect(p.hidden).toContain("Inventario");
      expect(p.hidden).toContain("Compras");
    }
  });

  it("a stock rubro (tienda) keeps inventario on, so the toggle is real", () => {
    expect(featuresForRubro("tienda").physicalStock).toBe(true);
    const inv = rubroPreview("tienda").categories.find(
      (c) => c.label === "Inventario y compras",
    )!;
    expect(inv.on).toBe(true);
    expect(visibleModulesForRubro("tienda")).toContain("inventory");
  });
});

describe("preview can never drift from the real ERP nav gate", () => {
  it("visibleCount equals the nav modules the shell actually shows", () => {
    for (const r of ALL_RUBROS) {
      expect(rubroPreview(r).visibleCount).toBe(visibleModulesForRubro(r).length);
    }
  });
});
