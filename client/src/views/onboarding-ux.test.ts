import { describe, it, expect, vi } from "vitest";
import {
  connRetryDelay,
  shouldRetryConn,
  connFeedback,
  withTimeout,
  TimeoutError,
  dashboardCta,
  loadStoredServer,
  saveStoredServer,
  SERVER_STORE_KEY,
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
