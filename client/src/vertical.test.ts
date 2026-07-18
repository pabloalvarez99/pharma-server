import { describe, it, expect } from "vitest";
import {
  parseVertical,
  hasRecetas,
  hasDte,
  verticalLabel,
  DEFAULT_VERTICAL,
  VERTICAL_OPTIONS,
  parseRubro,
  featuresForRubro,
  featuresFromPack,
  activeFeatures,
  clearPackCache,
  seedVerticalFor,
  DEFAULT_RUBRO,
  RUBRO_CATALOG,
  type Vertical,
  type Rubro,
} from "./vertical";

describe("parseVertical", () => {
  it("accepts the three known values", () => {
    expect(parseVertical("farmacia")).toBe("farmacia");
    expect(parseVertical("minimarket")).toBe("minimarket");
    expect(parseVertical("otro")).toBe("otro");
  });

  it("is case/space tolerant", () => {
    expect(parseVertical("  Farmacia  ")).toBe("farmacia");
    expect(parseVertical("MINIMARKET")).toBe("minimarket");
  });

  it("defaults unknown / null / empty to the generic default (never farmacia)", () => {
    expect(parseVertical(null)).toBe(DEFAULT_VERTICAL);
    expect(parseVertical(undefined)).toBe(DEFAULT_VERTICAL);
    expect(parseVertical("")).toBe(DEFAULT_VERTICAL);
    expect(parseVertical("kiosko")).toBe(DEFAULT_VERTICAL);
    expect(DEFAULT_VERTICAL).not.toBe("farmacia");
  });
});

describe("hasRecetas — pharmacy-only Ley 20.000 gate", () => {
  it("is true only for farmacia", () => {
    expect(hasRecetas("farmacia")).toBe(true);
    expect(hasRecetas("minimarket")).toBe(false);
    expect(hasRecetas("otro")).toBe(false);
  });
});

describe("hasDte — universal in Chile", () => {
  it("is true for every rubro", () => {
    (["farmacia", "minimarket", "otro"] as Vertical[]).forEach((v) =>
      expect(hasDte(v)).toBe(true),
    );
  });
});

describe("catalog integrity", () => {
  it("every option label round-trips through verticalLabel-able values", () => {
    VERTICAL_OPTIONS.forEach((o) => {
      expect(parseVertical(o.value)).toBe(o.value);
      expect(verticalLabel(o.value).length).toBeGreaterThan(0);
    });
  });

  it("covers exactly the three verticals", () => {
    expect(VERTICAL_OPTIONS.map((o) => o.value).sort()).toEqual([
      "farmacia",
      "minimarket",
      "otro",
    ]);
  });
});

describe("parseRubro — full 8-rubro catalog (does NOT collapse extras)", () => {
  it("preserves every catalog value, unlike parseVertical", () => {
    RUBRO_CATALOG.forEach((r) => expect(parseRubro(r.value)).toBe(r.value));
    // parseVertical would fold these to 'otro'; parseRubro keeps them.
    expect(parseRubro("cafe")).toBe("cafe");
    expect(parseRubro("servicios")).toBe("servicios");
    expect(parseVertical("cafe")).toBe("otro");
  });

  it("is case/space tolerant and defaults unknown to the generic default", () => {
    expect(parseRubro("  Belleza ")).toBe("belleza");
    expect(parseRubro(null)).toBe(DEFAULT_RUBRO);
    expect(parseRubro("kiosko")).toBe(DEFAULT_RUBRO);
    expect(DEFAULT_RUBRO).not.toBe("farmacia");
  });
});

describe("featuresForRubro — per-rubro capability gate", () => {
  it("farmacia = everything on (the maximal rubro)", () => {
    expect(featuresForRubro("farmacia")).toEqual({
      recetas: true,
      lotes: true,
      physicalStock: true,
      clinical: true,
    });
  });

  it("recetas + clinical are farmacia-exclusive", () => {
    RUBRO_CATALOG.filter((r) => r.value !== "farmacia").forEach((r) => {
      const f = featuresForRubro(r.value);
      expect(f.recetas).toBe(false);
      expect(f.clinical).toBe(false);
    });
  });

  it("lotes/vencimiento only for perishables (farmacia, minimarket, cafe)", () => {
    expect(featuresForRubro("farmacia").lotes).toBe(true);
    expect(featuresForRubro("minimarket").lotes).toBe(true);
    expect(featuresForRubro("cafe").lotes).toBe(true);
    expect(featuresForRubro("tienda").lotes).toBe(false);
    expect(featuresForRubro("servicios").lotes).toBe(false);
  });

  it("service rubros (belleza/servicios) sell WITHOUT physical stock", () => {
    expect(featuresForRubro("belleza").physicalStock).toBe(false);
    expect(featuresForRubro("servicios").physicalStock).toBe(false);
    // retail/generic still track stock
    expect(featuresForRubro("tienda").physicalStock).toBe(true);
    expect(featuresForRubro("otro").physicalStock).toBe(true);
  });

  it("coerces unknown / null to the generic default profile", () => {
    expect(featuresForRubro(null)).toEqual(featuresForRubro("otro"));
    expect(featuresForRubro("kiosko")).toEqual(featuresForRubro(DEFAULT_RUBRO));
  });

  it("every catalog rubro has a defined feature set", () => {
    (RUBRO_CATALOG.map((r) => r.value) as Rubro[]).forEach((v) => {
      const f = featuresForRubro(v);
      expect(typeof f.recetas).toBe("boolean");
      expect(typeof f.physicalStock).toBe("boolean");
    });
  });
});

describe("seedVerticalFor — rubro → demo seed pack (es→en)", () => {
  it("maps the four rubros that have a demo pack today", () => {
    expect(seedVerticalFor("farmacia")).toBe("pharmacy");
    expect(seedVerticalFor("minimarket")).toBe("minimarket");
    expect(seedVerticalFor("cafe")).toBe("cafe");
    expect(seedVerticalFor("tienda")).toBe("tienda");
  });

  it("returns null for rubros without a pack yet", () => {
    expect(seedVerticalFor("restaurant")).toBeNull();
    expect(seedVerticalFor("belleza")).toBeNull();
    expect(seedVerticalFor("servicios")).toBeNull();
    expect(seedVerticalFor("otro")).toBeNull();
  });

  it("returns null for unknown / empty values", () => {
    expect(seedVerticalFor("kiosko")).toBeNull();
    expect(seedVerticalFor(null)).toBeNull();
    expect(seedVerticalFor(undefined)).toBeNull();
  });

  it("every catalog seedVertical is a valid backend pack name or null", () => {
    const valid = new Set(["pharmacy", "minimarket", "cafe", "tienda", null]);
    RUBRO_CATALOG.forEach((r) => expect(valid.has(r.seedVertical)).toBe(true));
  });
});

describe("rubro visual identity (P2 showcase: accent + custom icon)", () => {
  it("every card declares a custom icon id (not just an emoji)", () => {
    RUBRO_CATALOG.forEach((r) => {
      expect(typeof r.iconId).toBe("string");
      expect(r.iconId.length).toBeGreaterThan(0);
    });
  });

  it("every card declares a 6-digit hex accent color", () => {
    RUBRO_CATALOG.forEach((r) => {
      expect(r.accent).toMatch(/^#[0-9a-fA-F]{6}$/);
    });
  });

  it("accents are distinct so each rubro reads as its own (no two share a color)", () => {
    const accents = RUBRO_CATALOG.map((r) => r.accent.toLowerCase());
    expect(new Set(accents).size).toBe(accents.length);
  });

  it("keeps the emoji `icon` for back-compat (login <option> still uses it)", () => {
    RUBRO_CATALOG.forEach((r) => expect(r.icon.length).toBeGreaterThan(0));
  });
});

describe("rubro depth (P3 showcase: native value copy + graceful roadmap)", () => {
  it("every card declares valueLines + comingSoon arrays", () => {
    RUBRO_CATALOG.forEach((r) => {
      expect(Array.isArray(r.valueLines)).toBe(true);
      expect(Array.isArray(r.comingSoon)).toBe(true);
    });
  });

  it("the generic 'otro' carries no rubro-native copy nor roadmap", () => {
    const otro = RUBRO_CATALOG.find((r) => r.value === "otro")!;
    expect(otro.valueLines).toEqual([]);
    expect(otro.comingSoon).toEqual([]);
  });

  it("regulatory claims (Ley 20.000 / controlados) appear ONLY for farmacia", () => {
    RUBRO_CATALOG.forEach((r) => {
      const copy = r.valueLines.join(" ");
      if (r.value === "farmacia") {
        expect(copy).toMatch(/Ley 20\.000/);
      } else {
        expect(copy).not.toMatch(/Ley 20\.000|controlados/i);
      }
    });
  });

  it("service rubros (no physical stock) state 'sin inventario' honestly", () => {
    ["belleza", "servicios"].forEach((v) => {
      const r = RUBRO_CATALOG.find((c) => c.value === v)!;
      expect(featuresForRubro(v).physicalStock).toBe(false);
      expect(r.valueLines.join(" ")).toMatch(/sin inventario/i);
    });
  });

  it("fully-built rubros have no roadmap; not-yet-built rubros document direction", () => {
    const built = ["farmacia", "minimarket", "otro"];
    RUBRO_CATALOG.forEach((r) => {
      if (built.includes(r.value)) {
        expect(r.comingSoon).toEqual([]);
      } else {
        expect(r.comingSoon.length).toBeGreaterThan(0);
      }
    });
  });
});

describe("activeFeatures / pack cache", () => {
  it("falls back to local constants when no pack is cached", () => {
    clearPackCache();
    expect(activeFeatures("farmacia")).toEqual(featuresForRubro("farmacia"));
    expect(activeFeatures("belleza").physicalStock).toBe(false);
  });

  it("featuresFromPack maps snake_case wire flags", () => {
    expect(
      featuresFromPack({
        recetas: true,
        lotes: false,
        physical_stock: true,
        clinical: true,
      }),
    ).toEqual({
      recetas: true,
      lotes: false,
      physicalStock: true,
      clinical: true,
    });
  });
});
