import { describe, it, expect, vi } from "vitest";
import {
  connRetryDelay,
  shouldRetryConn,
  connFeedback,
  withTimeout,
  TimeoutError,
  dashboardCta,
  dashboardActivation,
  loadStoredServer,
  saveStoredServer,
  loadStoredTenant,
  saveStoredTenant,
  SERVER_STORE_KEY,
  TENANT_STORE_KEY,
  DEFAULT_TENANT_SLUG,
  MAX_CONN_ATTEMPTS,
  type KeyStore,
} from "./onboarding-ux";

// validateServerUrl is single-sourced in first-run.ts (see first-run tests); this
// suite covers the retry/timeout, CTA, and persistence helpers that live here.

// A throwaway in-memory KeyStore mirroring the Web Storage subset we use.
function memStore(seed: Record<string, string> = {}): KeyStore & { dump(): Record<string, string> } {
  const m = new Map(Object.entries(seed));
  return {
    getItem: (k) => (m.has(k) ? (m.get(k) as string) : null),
    setItem: (k, v) => void m.set(k, v),
    dump: () => Object.fromEntries(m),
  };
}

describe("connection retry/timeout", () => {
  it("backoff is bounded and monotonic up to the cap", () => {
    expect(connRetryDelay(2)).toBe(400);
    expect(connRetryDelay(3)).toBe(1200);
    expect(connRetryDelay(4)).toBe(3000);
    expect(connRetryDelay(9)).toBe(3000); // clamps, never grows unbounded
  });

  it("stops retrying at MAX_CONN_ATTEMPTS (no infinite loop)", () => {
    expect(shouldRetryConn(1)).toBe(true);
    expect(shouldRetryConn(MAX_CONN_ATTEMPTS - 1)).toBe(true);
    expect(shouldRetryConn(MAX_CONN_ATTEMPTS)).toBe(false);
  });

  it("withTimeout rejects a hung promise with TimeoutError (fake timer)", async () => {
    let fire: (() => void) | null = null;
    const setTimer = (cb: () => void) => {
      fire = cb;
      return 1;
    };
    const never = new Promise<string>(() => {});
    const p = withTimeout(never, 8000, setTimer, () => {});
    fire!(); // simulate the timeout elapsing
    await expect(p).rejects.toBeInstanceOf(TimeoutError);
  });

  it("withTimeout resolves and clears the timer when work wins", async () => {
    const clear = vi.fn();
    const r = await withTimeout(Promise.resolve("ok"), 8000, () => 7, clear);
    expect(r).toBe("ok");
    expect(clear).toHaveBeenCalledWith(7);
  });

  it("feedback announces a retry mid-sequence and gives guidance at the end", () => {
    const mid = connFeedback("conexión rechazada", 1);
    expect(mid.willRetry).toBe(true);
    expect(mid.message).toMatch(/Reintentando/);
    const last = connFeedback(new TimeoutError(), MAX_CONN_ATTEMPTS);
    expect(last.willRetry).toBe(false);
    expect(last.message).toMatch(/Verifica que el servidor/);
  });
});

describe("dashboardCta", () => {
  it("fresh install (no catalog) → seed-demo CTA", () => {
    const r = dashboardCta({ productCount: 0, hasSales: false });
    expect(r.state).toBe("fresh");
    expect(r.cta?.action).toBe("seed-demo");
  });

  it("stock but no sales → first-sale CTA", () => {
    const r = dashboardCta({ productCount: 16, hasSales: false });
    expect(r.state).toBe("stock-only");
    expect(r.cta?.action).toBe("first-sale");
  });

  it("with sales → no CTA (normal panel)", () => {
    const r = dashboardCta({ productCount: 16, hasSales: true });
    expect(r.state).toBe("ready");
    expect(r.cta).toBeNull();
  });

  it("stats unavailable → no nag CTA", () => {
    const r = dashboardCta({ productCount: null, hasSales: false });
    expect(r.state).toBe("unknown");
    expect(r.cta).toBeNull();
  });
});

describe("dashboardActivation (rubro-aware first value)", () => {
  // Product rubros flow: fresh catalog → import → first sale → ready.
  it("product rubro, empty catalog → teaches importing the catalog", () => {
    const r = dashboardActivation({
      productCount: 0,
      hasSales: false,
      salesKnown: true,
      rubro: "minimarket",
    });
    expect(r.state).toBe("fresh");
    expect(r.physicalStock).toBe(true);
    expect(r.cta?.nav).toBe("importar");
    // demo stays reachable but secondary, not the primary teaching action.
    expect(r.cta?.secondary?.nav).toBe("configuracion");
  });

  it("product rubro, catalog but no sales → first sale", () => {
    const r = dashboardActivation({
      productCount: 24,
      hasSales: false,
      salesKnown: true,
      rubro: "farmacia",
    });
    expect(r.state).toBe("stock-only");
    expect(r.cta?.nav).toBe("pos");
    expect(r.cta?.secondary).toBeUndefined();
  });

  it("product rubro with sales → ready, no CTA", () => {
    const r = dashboardActivation({
      productCount: 24,
      hasSales: true,
      salesKnown: true,
      rubro: "farmacia",
    });
    expect(r.state).toBe("ready");
    expect(r.cta).toBeNull();
  });

  // Service rubros (physicalStock:false) never see "import products" — a
  // peluquero sells services, not stock. Activation is sales-driven only.
  it("service rubro, no sales → first SERVICE (never import products)", () => {
    const r = dashboardActivation({
      productCount: 0,
      hasSales: false,
      salesKnown: true,
      rubro: "belleza",
    });
    expect(r.physicalStock).toBe(false);
    expect(r.cta?.nav).toBe("pos");
    expect(r.cta?.label).not.toMatch(/producto/i);
    expect(r.cta?.secondary).toBeUndefined();
  });

  it("service rubro with sales → ready", () => {
    const r = dashboardActivation({
      productCount: 0,
      hasSales: true,
      salesKnown: true,
      rubro: "servicios",
    });
    expect(r.state).toBe("ready");
    expect(r.cta).toBeNull();
  });

  it("service rubro, sales unknown → no nag", () => {
    const r = dashboardActivation({
      productCount: null,
      hasSales: false,
      salesKnown: false,
      rubro: "belleza",
    });
    expect(r.state).toBe("unknown");
    expect(r.cta).toBeNull();
  });

  it("generic 'otro' is treated as a product rubro (has stock)", () => {
    const r = dashboardActivation({
      productCount: 0,
      hasSales: false,
      salesKnown: true,
      rubro: "otro",
    });
    expect(r.physicalStock).toBe(true);
    expect(r.cta?.nav).toBe("importar");
  });
});

describe("server URL persistence (survives restart)", () => {
  it("round-trips a chosen URL through the store in canonical form", () => {
    const store = memStore();
    expect(saveStoredServer(store, "http://192.168.1.50:8080/")).toBe(true);
    // canonical (no trailing slash) is what's persisted
    expect(store.dump()[SERVER_STORE_KEY]).toBe("http://192.168.1.50:8080");
    // a fresh "launch" reading the same store gets it back
    expect(loadStoredServer(store)).toBe("http://192.168.1.50:8080");
  });

  it("ignores a corrupt stored value instead of poisoning the field", () => {
    expect(loadStoredServer(memStore({ [SERVER_STORE_KEY]: "::garbage::" }))).toBeUndefined();
    expect(loadStoredServer(memStore())).toBeUndefined();
  });

  it("refuses to persist an invalid URL", () => {
    const store = memStore();
    expect(saveStoredServer(store, "ftp://x")).toBe(false);
    expect(store.dump()[SERVER_STORE_KEY]).toBeUndefined();
  });

  it("survives a storage that throws (privacy mode) without crashing", () => {
    const boom: KeyStore = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadStoredServer(boom)).toBeUndefined();
    expect(saveStoredServer(boom, "http://127.0.0.1:8080")).toBe(false);
  });
});

describe("branch slug persistence (no lock-out on second launch)", () => {
  it("round-trips a server-derived slug so Sucursal pre-fills it next launch", () => {
    const store = memStore();
    // First-run setup derived "almacen-don-jose" from the business name.
    saveStoredTenant(store, "almacen-don-jose");
    expect(store.dump()[TENANT_STORE_KEY]).toBe("almacen-don-jose");
    // A fresh launch reading the same store gets it back — NOT "principal".
    expect(loadStoredTenant(store)).toBe("almacen-don-jose");
  });

  it("falls back to the generic default when nothing is stored", () => {
    expect(loadStoredTenant(memStore())).toBe(DEFAULT_TENANT_SLUG);
    expect(loadStoredTenant(memStore({ [TENANT_STORE_KEY]: "   " }))).toBe(DEFAULT_TENANT_SLUG);
  });

  it("trims the slug and never overwrites a good value with a blank one", () => {
    const store = memStore({ [TENANT_STORE_KEY]: "minimarket-la-esquina" });
    saveStoredTenant(store, "   ");
    expect(loadStoredTenant(store)).toBe("minimarket-la-esquina");
    saveStoredTenant(store, "  nueva-sucursal  ");
    expect(store.dump()[TENANT_STORE_KEY]).toBe("nueva-sucursal");
  });

  it("survives a storage that throws (privacy mode) without crashing", () => {
    const boom: KeyStore = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadStoredTenant(boom)).toBe(DEFAULT_TENANT_SLUG);
    expect(() => saveStoredTenant(boom, "x")).not.toThrow();
  });
});
