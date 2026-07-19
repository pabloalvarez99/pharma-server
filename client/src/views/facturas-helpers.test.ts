import { describe, it, expect } from "vitest";
import {
  facturaTotals,
  lineMonto,
  type FacturaTotalsItem,
} from "./facturas-helpers";

// Compliance parity: the Facturas preview totals MUST equal the montos the server
// stamps on the DTE (`crates/dte/src/emit.rs::build_documento`). The server does
// per-line `trunc(cantidad * precio)` then ONE `desglose_iva` over the aggregate
// afecto — these vectors pin that exact shape so a per-line-split regression (which
// drifts by a peso on multi-line documents) fails here, not at the SII.

const item = (
  cantidad: string,
  precio: string,
  exento = false,
): FacturaTotalsItem => ({ cantidad, precio, exento });

describe("lineMonto (per-line CLP, mirrors server trunc)", () => {
  it("truncates qty * price to integer pesos", () => {
    expect(lineMonto("1", "1190")).toBe(1190);
    expect(lineMonto("3", "990")).toBe(2970);
    // 2 * 1495.5 = 2991.0 — clean; 1 * 1190.7 → trunc 1190 (no half-up on the line)
    expect(lineMonto("1", "1190.7")).toBe(1190);
    expect(lineMonto("2", "33.9")).toBe(67); // 67.8 → 67
  });

  it("skips invalid / non-positive rows (returns null, never NaN)", () => {
    expect(lineMonto("", "1190")).toBeNull(); // Number("")===0 → qty 0 → skip
    expect(lineMonto("0", "1190")).toBeNull();
    expect(lineMonto("-1", "1190")).toBeNull();
    expect(lineMonto("1", "-5")).toBeNull();
    expect(lineMonto("abc", "1190")).toBeNull();
  });

  it("treats a blank price as 0 (the emit form rejects it separately)", () => {
    // Number("")===0, so a half-typed row contributes 0 to the preview rather
    // than NaN; the POST-time validation (facturas.ts) blocks an empty precio.
    expect(lineMonto("1", "")).toBe(0);
  });
});

describe("facturaTotals (client↔server desglose parity)", () => {
  it("single affected line: 1190 → neto 1000 + IVA 190 (e2e dteLifecycle vector)", () => {
    expect(facturaTotals([item("1", "1190")])).toEqual({
      neto: 1000,
      iva: 190,
      exento: 0,
      total: 1190,
    });
  });

  it("splits IVA ONCE over the aggregate afecto, not per line", () => {
    // Per-line split would be desglose(1000)+desglose(1000) = (840+840, 160+160)
    // = neto 1680. The server sums afecto first (2000) → neto 1681, IVA 319.
    // This is THE multi-line drift the extraction guards.
    const t = facturaTotals([item("1", "1000"), item("1", "1000")]);
    expect(t.neto).toBe(1681); // desgloseIva(2000) = round(2000/1.19) = 1681
    expect(t.iva).toBe(319);
    expect(t.neto + t.iva).toBe(2000);
    expect(t.total).toBe(2000);
  });

  it("keeps neto + iva == afecto and total == afecto + exento", () => {
    const items = [item("2", "1190"), item("1", "990"), item("3", "500", true)];
    const afecto = 2380 + 990; // 3370
    const exento = 1500;
    const t = facturaTotals(items);
    expect(t.exento).toBe(exento);
    expect(t.neto + t.iva).toBe(afecto);
    expect(t.total).toBe(afecto + exento);
  });

  it("exento-only document has zero neto/iva (no afecta base)", () => {
    expect(facturaTotals([item("2", "750", true)])).toEqual({
      neto: 0,
      iva: 0,
      exento: 1500,
      total: 1500,
    });
  });

  it("empty / all-invalid rows yield a zero total (never NaN)", () => {
    expect(facturaTotals([])).toEqual({ neto: 0, iva: 0, exento: 0, total: 0 });
    expect(facturaTotals([item("", ""), item("0", "100")])).toEqual({
      neto: 0,
      iva: 0,
      exento: 0,
      total: 0,
    });
  });

  it("per-line truncation accumulates before the split (no double-round)", () => {
    // Two lines each 67.8 → trunc 67 → afecto 134; desgloseIva(134) absorbs.
    const t = facturaTotals([item("2", "33.9"), item("2", "33.9")]);
    expect(t.total).toBe(134);
    expect(t.neto + t.iva).toBe(134);
  });
});
