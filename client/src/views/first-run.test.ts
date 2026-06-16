import { describe, it, expect } from "vitest";
import {
  resolveServerConfig,
  validateServerUrl,
  connectionState,
  firstRunStep,
  dashboardReadiness,
  visibleModules,
  visibleModulesForRubro,
  rubroPreview,
  FALLBACK_SERVER,
  type FirstRunState,
} from "./first-run";
import type { HealthInfo, InventorySummary, DailySalesRow } from "../api";

// These are USER JOURNEYS, not unit ping-pong: each `it` walks a real operator
// through a slice of "install -> login -> first sale" and asserts what they'd
// see. The point is to prove a brand-new RUT can self-onboard with NO CLI, in
// Spanish, in BOTH verticals, without dead-ends. Pure logic (no DOM) on purpose
// — same shape as cashier-loop / stock-helpers tests already in the client.

const HEALTH_OK: HealthInfo = { status: "ok", db: "ok", reachable: true };
const HEALTH_DEGRADED: HealthInfo = { status: "ok", db: "down", reachable: true };
const HEALTH_DOWN: HealthInfo = { status: "down", db: "—", reachable: false };

/** A state where everything before `overrides` is satisfied — lets each journey
 *  start from the relevant step without restating the whole struct. */
function state(overrides: Partial<FirstRunState> = {}): FirstRunState {
  return {
    serverReachable: true,
    loggedIn: true,
    verticalChosen: true,
    hasData: true,
    ...overrides,
  };
}

describe("Journey 1 — brand-new install on the same machine", () => {
  it("with no remembered server and no build override, guesses loopback and opens advanced", () => {
    const r = resolveServerConfig({ stored: null, env: null });
    expect(r.value).toBe(FALLBACK_SERVER);
    expect(r.firstLaunch).toBe(true);
  });

  it("a fresh, unreachable server lands the operator on 'connect your server' (hard block, clear es)", () => {
    const guidance = firstRunStep(state({ serverReachable: false, loggedIn: false }));
    expect(guidance.step).toBe("configurar-servidor");
    expect(guidance.soft).toBe(false);
    expect(guidance.hint).toMatch(/servidor/i);
  });
});

describe("Journey 2 — LAN client points at the local server on another machine", () => {
  it("typing a valid IP:port validates and normalises (trailing slash dropped)", () => {
    const check = validateServerUrl("http://192.168.1.50:8080/");
    expect(check).toEqual({ ok: true, url: "http://192.168.1.50:8080" });
  });

  it("a reachable+healthy server reports accessible, advancing to login", () => {
    const conn = connectionState(HEALTH_OK);
    expect(conn.kind).toBe("ok");
    expect(conn.message).toContain("accesible");
    const guidance = firstRunStep(state({ loggedIn: false }));
    expect(guidance.step).toBe("iniciar-sesion");
  });

  it("a remembered server from a prior successful login is NOT a first launch", () => {
    const r = resolveServerConfig({ stored: "http://192.168.1.50:8080", env: null });
    expect(r.firstLaunch).toBe(false);
    expect(r.value).toBe("http://192.168.1.50:8080");
  });
});

describe("Journey 3 — logged in but rubro not chosen yet (soft block, no dead-end)", () => {
  it("points the operator at 'elegir-rubro' WITHOUT trapping them (soft)", () => {
    const guidance = firstRunStep(state({ verticalChosen: false }));
    expect(guidance.step).toBe("elegir-rubro");
    expect(guidance.soft).toBe(true);
    expect(guidance.hint).toMatch(/genérico/i); // ERP keeps working meanwhile
  });
});

describe("Journey 4 — pharmacist onboarding (farmacia)", () => {
  it("sees the Recetas / controlled-drug module", () => {
    expect(visibleModules("farmacia")).toContain("recetas");
  });

  it("walks the full flow to 'listo' once demo data is loaded", () => {
    expect(firstRunStep(state({ hasData: false })).step).toBe("cargar-datos");
    expect(firstRunStep(state()).step).toBe("listo");
  });
});

describe("Journey 5 — minimarket onboarding (no pharmacy modules)", () => {
  it("HIDES Recetas but keeps universal boleta/factura modules", () => {
    const mods = visibleModules("minimarket");
    expect(mods).not.toContain("recetas");
    expect(mods).toContain("boletas");
    expect(mods).toContain("facturas");
    expect(mods).toContain("pos");
  });

  it("'otro' (generic ERP) also hides Recetas", () => {
    expect(visibleModules("otro")).not.toContain("recetas");
  });
});

describe("Journey 5b — per-rubro module gate (both extremes)", () => {
  it("farmacia = full: recetas + physical-stock modules all present", () => {
    const mods = visibleModulesForRubro("farmacia");
    expect(mods).toContain("recetas");
    expect(mods).toContain("inventory");
    expect(mods).toContain("compras");
    expect(mods).toContain("pos");
  });

  it("servicios = minimal: no recetas, no inventario/compras (sells without stock)", () => {
    const mods = visibleModulesForRubro("servicios");
    expect(mods).not.toContain("recetas");
    expect(mods).not.toContain("inventory");
    expect(mods).not.toContain("compras");
    // but it still SELLS and emits DTE — the agnostic core stays
    expect(mods).toContain("pos");
    expect(mods).toContain("boletas");
    expect(mods).toContain("caja");
  });

  it("belleza mirrors servicios (service rubro, no physical stock)", () => {
    expect(visibleModulesForRubro("belleza")).toEqual(visibleModulesForRubro("servicios"));
  });

  it("café keeps inventario (perecibles) but drops recetas", () => {
    const mods = visibleModulesForRubro("cafe");
    expect(mods).toContain("inventory");
    expect(mods).not.toContain("recetas");
  });

  it("tienda keeps physical stock, drops recetas", () => {
    const mods = visibleModulesForRubro("tienda");
    expect(mods).toContain("inventory");
    expect(mods).toContain("compras");
    expect(mods).not.toContain("recetas");
  });
});

describe("Journey 5c — live ERP preview per rubro (rubro select 'mostrar, no contar')", () => {
  it("farmacia preview surfaces its native pharmacy capabilities + demo pack", () => {
    const p = rubroPreview("farmacia");
    expect(p.native.join(" ")).toMatch(/Recetas/i);
    expect(p.native.join(" ")).toMatch(/Ley 20\.000/);
    expect(p.native.join(" ")).toMatch(/clínica/i);
    expect(p.hasDemo).toBe(true);
    // categories all on for the maximal rubro
    expect(p.categories.every((c) => c.on)).toBe(true);
    // nothing notable hidden
    expect(p.hidden).toEqual([]);
    expect(p.visibleCount).toBe(p.totalCount);
  });

  it("minimarket shows lotes (perecibles), hides Recetas, has a demo pack", () => {
    const p = rubroPreview("minimarket");
    expect(p.native.join(" ")).toMatch(/Lotes/i);
    expect(p.native.join(" ")).not.toMatch(/Recetas/i);
    expect(p.hidden).toContain("Recetas");
    expect(p.hasDemo).toBe(true);
    // still tracks physical stock
    expect(p.categories.find((c) => c.label === "Inventario y compras")?.on).toBe(true);
  });

  it("belleza (service rubro) honestly shows NO inventory and no demo yet", () => {
    const p = rubroPreview("belleza");
    expect(p.native.join(" ")).toMatch(/sin inventario/i);
    expect(p.categories.find((c) => c.label === "Inventario y compras")?.on).toBe(false);
    expect(p.hidden).toContain("Inventario");
    expect(p.hidden).toContain("Compras");
    expect(p.hasDemo).toBe(false);
    expect(p.visibleCount).toBeLessThan(p.totalCount);
  });

  it("otro (generic ERP) has no rubro-native bullets but still sells + emits", () => {
    const p = rubroPreview("otro");
    expect(p.native).toEqual([]);
    expect(p.categories.find((c) => c.label === "Ventas y caja")?.on).toBe(true);
    expect(p.categories.find((c) => c.label === "Boletas y facturas (SII)")?.on).toBe(true);
  });
});

describe("Journey 6 — server unreachable edge case (clear es error, no crash)", () => {
  it("a thrown health probe yields a Spanish 'unreachable' message, not an exception", () => {
    const conn = connectionState(null, true);
    expect(conn.kind).toBe("unreachable");
    expect(conn.message).toMatch(/no se pudo contactar/i);
  });

  it("a health payload that says not-reachable maps to the same clear message", () => {
    expect(connectionState(HEALTH_DOWN).kind).toBe("unreachable");
  });

  it("the operator stays on 'configurar-servidor' (can't dead-end into a blank app)", () => {
    expect(firstRunStep(state({ serverReachable: false })).step).toBe("configurar-servidor");
  });
});

describe("Journey 7 — server URL typos are caught inline in Spanish", () => {
  it("empty", () => {
    const c = validateServerUrl("   ");
    expect(c.ok).toBe(false);
    if (!c.ok) expect(c.error).toMatch(/no puede estar vacía/i);
  });

  it("missing scheme", () => {
    const c = validateServerUrl("192.168.1.50:8080");
    expect(c.ok).toBe(false);
    if (!c.ok) expect(c.error).toMatch(/http:\/\//);
  });

  it("garbage", () => {
    const c = validateServerUrl("http://");
    expect(c.ok).toBe(false);
  });
});

describe("Journey 8 — dashboard after loading demo data (no blank dead-end)", () => {
  const empty: InventorySummary = {
    total: 0,
    active: 0,
    low_stock: 0,
    out_of_stock: 0,
    inventory_value: "0",
    expired: 0,
  };
  const seeded: InventorySummary = { ...empty, total: 16, active: 16, inventory_value: "1250000" };
  const salesToday: DailySalesRow[] = [
    { date: new Date().toISOString().slice(0, 10), orders: 3, revenue: "30000", cash: "30000", card: "0" },
  ];

  it("before seeding: friendly empty state guiding to load data", () => {
    const r = dashboardReadiness(empty, []);
    expect(r.populated).toBe(false);
    expect(r.headline).toMatch(/datos demo|catálogo/i);
  });

  it("after seeding (16 demo products): dashboard is populated", () => {
    const r = dashboardReadiness(seeded, []);
    expect(r.populated).toBe(true);
    expect(r.headline).toContain("16");
  });

  it("seeded + a sale today: headline reflects activity", () => {
    const r = dashboardReadiness(seeded, salesToday);
    expect(r.populated).toBe(true);
    expect(r.headline).toMatch(/ventas registradas/i);
  });

  it("a totally failed load (null summary) is treated as not-populated, never throws", () => {
    expect(dashboardReadiness(null, null).populated).toBe(false);
  });
});

describe("Journey 9 — degraded server (db down) is reported, not hidden", () => {
  it("reachable but db down -> 'degradado' message naming the db", () => {
    const conn = connectionState(HEALTH_DEGRADED);
    expect(conn.kind).toBe("degraded");
    expect(conn.message).toMatch(/degradado/i);
    expect(conn.message).toMatch(/base de datos/i);
  });
});
