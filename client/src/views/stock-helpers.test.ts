import { describe, it, expect } from "vitest";
import {
  toRfc3339Noon,
  stockLevel,
  expiryStatus,
  pharmaFieldsVisible,
} from "./stock-helpers";

describe("toRfc3339Noon", () => {
  it("anchors a date-input value at noon UTC", () => {
    expect(toRfc3339Noon("2026-05-01")).toBe("2026-05-01T12:00:00Z");
  });

  it("returns undefined for blank input (server defaults to now)", () => {
    expect(toRfc3339Noon("")).toBeUndefined();
    expect(toRfc3339Noon("   ")).toBeUndefined();
  });

  // Regression for the batch-expiry date-slip: midnight-UTC storage rendered
  // one day early in Chile's TZ. Noon-UTC must render the SAME calendar day in
  // any negative UTC offset (CL is UTC-3/-4).
  it("does not slip a day when rendered west of UTC", () => {
    const noon = toRfc3339Noon("2026-05-01")!;
    // Simulate es-CL (UTC-4) by reading the date parts at a -4h offset.
    const d = new Date(noon);
    const shifted = new Date(d.getTime() - 4 * 3_600_000);
    expect(shifted.getUTCFullYear()).toBe(2026);
    expect(shifted.getUTCMonth()).toBe(4); // May (0-based)
    expect(shifted.getUTCDate()).toBe(1); // still the 1st, not 30-Apr
    // The old midnight anchor WOULD have slipped:
    const midnight = new Date("2026-05-01T00:00:00Z");
    const oldShifted = new Date(midnight.getTime() - 4 * 3_600_000);
    expect(oldShifted.getUTCDate()).toBe(30); // proves the bug it fixes
  });
});

describe("stockLevel", () => {
  it("buckets out / low / ok against the default threshold", () => {
    expect(stockLevel(0)).toBe("out");
    expect(stockLevel(-3)).toBe("out");
    expect(stockLevel(5)).toBe("low");
    expect(stockLevel(1)).toBe("low");
    expect(stockLevel(6)).toBe("ok");
    expect(stockLevel(9999)).toBe("ok");
  });

  it("respects a custom low threshold", () => {
    expect(stockLevel(10, 10)).toBe("low");
    expect(stockLevel(11, 10)).toBe("ok");
  });

  it("treats NaN as out (never a false 'ok')", () => {
    expect(stockLevel(Number.NaN)).toBe("out");
  });
});

describe("expiryStatus", () => {
  const now = new Date("2026-05-01T12:00:00Z");

  it("flags an already-past date as caducado", () => {
    const s = expiryStatus("2026-04-25T12:00:00Z", now);
    expect(s.expired).toBe(true);
    expect(s.tone).toBe("danger");
    expect(s.days).toBeLessThan(0);
  });

  it("reads a same-day expiry as 0 days (por vencer, not expired)", () => {
    const s = expiryStatus("2026-05-01T00:00:00Z", now);
    expect(s.days).toBe(0);
    expect(s.expired).toBe(false);
    expect(s.tone).toBe("warn");
  });

  it("warns within the 30-day window and is ok beyond it", () => {
    expect(expiryStatus("2026-05-20T12:00:00Z", now).tone).toBe("warn");
    expect(expiryStatus("2026-08-01T12:00:00Z", now).tone).toBe("ok");
    expect(expiryStatus("2026-08-01T12:00:00Z", now).label).toBe("Vigente");
  });

  it("returns a muted dash for an unparseable date", () => {
    const s = expiryStatus("not-a-date", now);
    expect(s.tone).toBe("muted");
    expect(s.label).toBe("—");
  });
});

describe("pharmaFieldsVisible (multi-rubro)", () => {
  it("shows pharma fields by default (unset / null / pharmacy)", () => {
    expect(pharmaFieldsVisible(null)).toBe(true);
    expect(pharmaFieldsVisible(undefined)).toBe(true);
    expect(pharmaFieldsVisible("")).toBe(true);
    expect(pharmaFieldsVisible("pharmacy")).toBe(true);
    expect(pharmaFieldsVisible("farmacia")).toBe(true);
  });

  it("hides pharma fields for non-pharmacy verticals", () => {
    expect(pharmaFieldsVisible("minimarket")).toBe(false);
    expect(pharmaFieldsVisible("general")).toBe(false);
    expect(pharmaFieldsVisible("MARKET")).toBe(false);
    expect(pharmaFieldsVisible("  Almacén ")).toBe(false);
    expect(pharmaFieldsVisible("retail")).toBe(false);
  });
});
