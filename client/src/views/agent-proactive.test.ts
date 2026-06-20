import { describe, it, expect } from "vitest";
import {
  buildProactiveCards,
  filterDismissed,
  isDismissed,
  markDismissed,
  dismissKey,
  shouldShowAgentIntro,
  dismissAgentIntro,
  localDay,
  MAX_CARDS,
  AGENT_INTRO_KEY,
  type KeyStore,
  type ProactiveInput,
} from "./agent-proactive";
import type { CashSession, InventorySummary, NearExpiryRow } from "../api";

const NOW = new Date("2026-06-20T15:00:00"); // local
const TODAY = "2026-06-20";

function session(opened: string, status = "open"): CashSession {
  return {
    id: `cash:${opened}`,
    user: "user:1",
    register_name: "Caja 1",
    opening_cash: "10000",
    opening_notes: null,
    closing_cash_counted: null,
    closing_cash_expected: null,
    discrepancia: null,
    closing_notes: null,
    opened_at: opened,
    closed_at: null,
    status,
  };
}

function expiryRow(product: string, days: number, expired: boolean): NearExpiryRow {
  return {
    product_id: product,
    product_name: product,
    batch_id: `${product}-b`,
    batch_code: "L1",
    expiry_date: "2026-06-25",
    stock: 5,
    days_to_expiry: days,
    expired,
  };
}

const INV = (low: number, out: number): InventorySummary => ({
  total: 100,
  active: 100,
  low_stock: low,
  out_of_stock: out,
  inventory_value: "0",
  expired: 0,
});

const base = (over: Partial<ProactiveInput>): ProactiveInput => ({
  rubro: "farmacia",
  openSessions: null,
  expiry: null,
  inventory: null,
  now: NOW,
  ...over,
});

// memory-backed KeyStore for dismissal tests
function memStore(init: Record<string, string> = {}): KeyStore {
  const m = new Map(Object.entries(init));
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => {
      m.set(k, v);
    },
  };
}

describe("localDay", () => {
  it("formats a local calendar day", () => {
    expect(localDay(new Date("2026-01-05T23:30:00"))).toBe("2026-01-05");
  });
});

describe("buildProactiveCards — caja", () => {
  it("flags a register opened on a prior day as danger", () => {
    const cards = buildProactiveCards(
      base({ openSessions: [session("2026-06-19T20:00:00")] }),
    );
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({ id: "caja-stale", severity: "danger" });
    expect(cards[0].action).toEqual({ kind: "nav", nav: "caja", label: "Revisar caja" });
  });

  it("does NOT flag a register opened today", () => {
    const cards = buildProactiveCards(
      base({ openSessions: [session(`${TODAY}T09:00:00`)] }),
    );
    expect(cards).toHaveLength(0);
  });

  it("ignores a closed session and pluralises the count", () => {
    const cards = buildProactiveCards(
      base({
        openSessions: [
          session("2026-06-18T09:00:00"),
          session("2026-06-19T09:00:00"),
          session("2026-06-17T09:00:00", "closed"),
        ],
      }),
    );
    expect(cards[0].body).toContain("2 cajas");
  });

  it("null openSessions contributes no card (failed fetch ≠ error)", () => {
    expect(buildProactiveCards(base({ openSessions: null }))).toHaveLength(0);
  });
});

describe("buildProactiveCards — expiry", () => {
  it("emits an expired (danger) and a soon (warn, agent ask) card", () => {
    const cards = buildProactiveCards(
      base({ expiry: [expiryRow("p1", -2, true), expiryRow("p2", 3, false)] }),
    );
    const ids = cards.map((c) => c.id);
    expect(ids).toContain("expiry-expired");
    expect(ids).toContain("expiry-soon");
    const soon = cards.find((c) => c.id === "expiry-soon")!;
    expect(soon.action).toEqual({
      kind: "ask",
      question: "¿Qué se está por vencer?",
      label: "Preguntar al agente",
    });
  });

  it("counts DISTINCT products, not rows", () => {
    const cards = buildProactiveCards(
      base({ expiry: [expiryRow("p1", 2, false), expiryRow("p1", 4, false)] }),
    );
    const soon = cards.find((c) => c.id === "expiry-soon")!;
    expect(soon.body).toContain("1 producto");
  });

  it("a non-perishable rubro (tienda) gets NO expiry cards even with data", () => {
    const cards = buildProactiveCards(
      base({ rubro: "tienda", expiry: [expiryRow("p1", 2, false)] }),
    );
    expect(cards.filter((c) => c.id.startsWith("expiry"))).toHaveLength(0);
  });
});

describe("buildProactiveCards — stock", () => {
  it("agotado = danger; bajo alone = warn; agotado suppresses the bajo card", () => {
    expect(buildProactiveCards(base({ inventory: INV(3, 2) }))[0]).toMatchObject({
      id: "stock-out",
      severity: "danger",
    });
    const lowOnly = buildProactiveCards(base({ inventory: INV(4, 0) }));
    expect(lowOnly.map((c) => c.id)).toEqual(["stock-low"]);
  });

  it("a service rubro (belleza) gets NO stock cards", () => {
    const cards = buildProactiveCards(base({ rubro: "belleza", inventory: INV(5, 5) }));
    expect(cards).toHaveLength(0);
  });
});

describe("buildProactiveCards — ordering & cap", () => {
  it("danger floats above warn and the list is capped at MAX_CARDS", () => {
    const cards = buildProactiveCards(
      base({
        openSessions: [session("2026-06-18T09:00:00")], // danger
        expiry: [expiryRow("p1", -1, true), expiryRow("p2", 2, false)], // danger + warn
        inventory: INV(3, 2), // danger
      }),
    );
    expect(cards).toHaveLength(MAX_CARDS);
    // every card kept after the cap is danger (warns were lower priority)
    expect(cards.every((c) => c.severity === "danger")).toBe(true);
  });
});

describe("dismissal (per-day)", () => {
  it("dismissKey is date-stamped", () => {
    expect(dismissKey("caja-stale", NOW)).toBe(`caja-stale:${TODAY}`);
  });

  it("isDismissed only honours TODAY's dismissal", () => {
    const store = memStore();
    markDismissed(store, "caja-stale", NOW);
    expect(isDismissed(store, "caja-stale", NOW)).toBe(true);
    // next day → re-surfaces
    const tomorrow = new Date("2026-06-21T08:00:00");
    expect(isDismissed(store, "caja-stale", tomorrow)).toBe(false);
  });

  it("markDismissed prunes keys from other days (bounded storage)", () => {
    const store = memStore({
      "pharma:agent-cards-dismissed": JSON.stringify(["old:2026-06-01"]),
    });
    markDismissed(store, "stock-out", NOW);
    const raw = store.getItem("pharma:agent-cards-dismissed")!;
    expect(raw).not.toContain("2026-06-01");
    expect(raw).toContain(`stock-out:${TODAY}`);
  });

  it("filterDismissed removes only today-dismissed cards", () => {
    const store = memStore();
    const cards = buildProactiveCards(
      base({ openSessions: [session("2026-06-18T09:00:00")], inventory: INV(0, 3) }),
    );
    markDismissed(store, "caja-stale", NOW);
    const kept = filterDismissed(cards, store, NOW);
    expect(kept.map((c) => c.id)).toEqual(["stock-out"]);
  });
});

describe("agent intro (one-time)", () => {
  it("shows until dismissed, then never again", () => {
    const store = memStore();
    expect(shouldShowAgentIntro(store)).toBe(true);
    dismissAgentIntro(store);
    expect(shouldShowAgentIntro(store)).toBe(false);
    expect(store.getItem(AGENT_INTRO_KEY)).toBe("1");
  });
});
