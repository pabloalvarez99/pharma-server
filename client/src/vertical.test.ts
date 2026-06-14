import { describe, it, expect } from "vitest";
import {
  parseVertical,
  hasRecetas,
  hasDte,
  verticalLabel,
  DEFAULT_VERTICAL,
  VERTICAL_OPTIONS,
  type Vertical,
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
