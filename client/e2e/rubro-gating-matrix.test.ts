import { describe, it, expect } from "vitest";
import {
  RUBRO_CATALOG,
  featuresForRubro,
  parseRubro,
  parseVertical,
  hasDte,
  type Rubro,
  type RubroFeatures,
} from "../src/vertical";
import {
  visibleModulesForRubro,
  ALL_MODULES,
  type ModuleId,
} from "../src/views/first-run";

// Per-rubro gating MATRIX — the single coverage net that proves all 8 rubros of
// the onboarding catalog (docs/strategy/rubro-catalog.md) show/hide the right
// sections. `vertical.test.ts` (ye) and `first-run.test.ts` (ye) already cover
// slices; this file is exhaustive + locks the two cross-cutting contracts those
// slices never assert TOGETHER:
//
//   1. featuresForRubro is a FULL spec table (every rubro × every flag), with the
//      structural invariants a future bad row would violate.
//   2. the NAV gate (visibleModulesForRubro) and the CAPABILITY gate
//      (featuresForRubro) must AGREE for every rubro — they drive different UI
//      (shell nav vs. product/POS features) off the same intent and must not
//      drift apart. Nothing tested this agreement before.
//   3. DTE/boleta is UNIVERSAL: every CL business emits, so boleta+factura stay
//      visible for ALL 8 rubros (RutBusiness pivot — farmacia is the beachhead,
//      not the boundary).
//
// Pure logic, no DOM, no server — runs under `npm test` (vitest). The live
// boleta-universal / minimarket-no-receta contracts are exercised over real HTTP
// in flows.mjs (golden path + noPrescriptionFlow).

/** The 8 catalog rubros as their typed keys. */
const RUBROS = RUBRO_CATALOG.map((r) => r.value as Rubro);

/** The exact capability set each rubro must turn on. Literal so this table reads
 *  like the spec and ANY silent flag flip in vertical.ts fails a test. */
const EXPECTED: Readonly<Record<Rubro, RubroFeatures>> = {
  farmacia: { recetas: true, lotes: true, physicalStock: true, clinical: true },
  minimarket: { recetas: false, lotes: true, physicalStock: true, clinical: false },
  cafe: { recetas: false, lotes: true, physicalStock: true, clinical: false },
  restaurant: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  tienda: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  otro: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  // Service rubros sell WITHOUT physical inventory — the agnostic-core proof.
  belleza: { recetas: false, lotes: false, physicalStock: false, clinical: false },
  servicios: { recetas: false, lotes: false, physicalStock: false, clinical: false },
};

/** Modules visibleModulesForRubro gates; everything else is universal. Mirrors
 *  the gate in first-run.ts (recetas←recetas, inventory/compras/sucursales←
 *  physicalStock: mover stock entre locales no tiene sentido en un rubro que no
 *  lleva inventario físico). */
const GATED_MODULES: ReadonlyArray<ModuleId> = [
  "recetas",
  "inventory",
  "compras",
  "sucursales",
];
const UNIVERSAL_MODULES: ReadonlyArray<ModuleId> = ALL_MODULES.filter(
  (m) => !GATED_MODULES.includes(m),
);
const SERVICE_RUBROS: ReadonlyArray<Rubro> = ["belleza", "servicios"];

describe("rubro catalog completeness", () => {
  it("is the full 8-rubro catalog (forces this matrix to grow with it)", () => {
    expect(RUBROS.length).toBe(8);
  });

  it("the EXPECTED table covers exactly the catalog rubros (no rubro left ungated)", () => {
    expect(Object.keys(EXPECTED).sort()).toEqual([...RUBROS].sort());
  });
});

describe("featuresForRubro — full spec table (every rubro × every flag)", () => {
  it.each(RUBROS)("featuresForRubro(%s) matches the locked spec", (r) => {
    expect(featuresForRubro(r)).toEqual(EXPECTED[r]);
  });

  // Structural invariants — these catch a *future* bad row even if EXPECTED is
  // edited to match it, because they encode WHY the flags relate.
  it.each(RUBROS)("%s: clinical fields imply recetas (clinical ⇒ recetas)", (r) => {
    const f = featuresForRubro(r);
    if (f.clinical) expect(f.recetas).toBe(true);
  });

  it.each(RUBROS)("%s: recetas is farmacia-exclusive (Ley 20.000)", (r) => {
    expect(featuresForRubro(r).recetas).toBe(r === "farmacia");
  });

  it.each(RUBROS)("%s: lotes/vencimiento requires physical stock (lotes ⇒ physicalStock)", (r) => {
    const f = featuresForRubro(r);
    if (f.lotes) expect(f.physicalStock).toBe(true);
  });
});

describe("nav gate ↔ capability gate must AGREE for every rubro", () => {
  it.each(RUBROS)("%s: visibleModulesForRubro is consistent with featuresForRubro", (r) => {
    const visible = new Set<ModuleId>(visibleModulesForRubro(r));
    const f = featuresForRubro(r);
    // recetas module visible iff the rubro has recetas capability.
    expect(visible.has("recetas")).toBe(f.recetas);
    // inventario + compras + sucursales visibles iff the rubro tracks physical
    // stock (V2: transferir stock entre locales exige stock físico).
    expect(visible.has("inventory")).toBe(f.physicalStock);
    expect(visible.has("compras")).toBe(f.physicalStock);
    expect(visible.has("sucursales")).toBe(f.physicalStock);
  });
});

describe("universal modules — every rubro keeps the core ERP + DTE", () => {
  it.each(RUBROS)("%s: all non-gated modules stay visible", (r) => {
    const visible = new Set<ModuleId>(visibleModulesForRubro(r));
    for (const m of UNIVERSAL_MODULES) {
      expect(visible.has(m)).toBe(true);
    }
  });

  it.each(RUBROS)("%s: boleta + factura SII are visible (DTE is universal in CL)", (r) => {
    const visible = visibleModulesForRubro(r);
    expect(visible).toContain("boletas");
    expect(visible).toContain("facturas");
  });

  it.each(RUBROS)("%s: hasDte allows emission (boleta funciona en todos los rubros)", (r) => {
    // hasDte keys off the coarse Vertical; map the full rubro down first.
    expect(hasDte(parseVertical(r))).toBe(true);
  });

  it.each(RUBROS)("%s: the daily POS loop (pos/caja/clientes/devoluciones) stays visible", (r) => {
    const visible = new Set<ModuleId>(visibleModulesForRubro(r));
    for (const m of ["pos", "caja", "clientes", "devoluciones"] as ModuleId[]) {
      expect(visible.has(m)).toBe(true);
    }
  });
});

describe("recetas/controlados — visible iff farmacia (all 8 rubros)", () => {
  it.each(RUBROS)("%s: Recetas in the nav iff farmacia", (r) => {
    expect(visibleModulesForRubro(r).includes("recetas")).toBe(r === "farmacia");
  });
});

describe("service rubros sell WITHOUT stock yet still sell (agnostic core)", () => {
  it.each(SERVICE_RUBROS)("%s: no physical stock, no lotes", (r) => {
    const f = featuresForRubro(r);
    expect(f.physicalStock).toBe(false);
    expect(f.lotes).toBe(false);
  });

  it.each(SERVICE_RUBROS)("%s: inventario/compras hidden, but POS + boleta + caja stay", (r) => {
    const visible = new Set<ModuleId>(visibleModulesForRubro(r));
    expect(visible.has("inventory")).toBe(false);
    expect(visible.has("compras")).toBe(false);
    // it still SELLS and emits a boleta — that is the whole point.
    expect(visible.has("pos")).toBe(true);
    expect(visible.has("caja")).toBe(true);
    expect(visible.has("boletas")).toBe(true);
  });

  it("belleza and servicios gate identically (both are pure-service rubros)", () => {
    expect(visibleModulesForRubro("belleza")).toEqual(visibleModulesForRubro("servicios"));
    expect(featuresForRubro("belleza")).toEqual(featuresForRubro("servicios"));
  });
});

describe("rubro parsing — full key preserved vs. coarse vertical folded", () => {
  it.each(RUBROS)("parseRubro(%s) round-trips the full catalog key", (r) => {
    expect(parseRubro(r)).toBe(r);
  });

  it("the 5 non-core rubros fold to 'otro' under the coarse parseVertical", () => {
    // parseVertical only knows farmacia/minimarket/otro; the extras collapse.
    for (const r of ["restaurant", "cafe", "tienda", "belleza", "servicios"]) {
      expect(parseVertical(r)).toBe("otro");
      // …but parseRubro keeps them distinct, so featuresForRubro can differ.
      expect(parseRubro(r)).toBe(r);
    }
  });

  it("unknown / null / empty default to a generic, non-farmacia profile", () => {
    for (const junk of [null, undefined, "", "  ", "kiosko", "🛒"]) {
      const f = featuresForRubro(junk as unknown as string);
      expect(f).toEqual(featuresForRubro("otro"));
      expect(f.recetas).toBe(false);
    }
  });
});
