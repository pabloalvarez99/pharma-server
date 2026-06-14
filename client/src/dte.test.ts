import { describe, it, expect } from "vitest";
import {
  cssKey,
  fmtDateCl,
  cafTone,
  estadoBadge,
  computeDocTotals,
  upgradeNote,
  pickToday,
} from "./dte";

describe("cssKey", () => {
  it("strips record-id punctuation to a selector-safe key", () => {
    expect(cssKey("dte:abc123")).toBe("dte-abc123");
    expect(cssKey("dte:⚡weird/id")).toBe("dte--weird-id");
    expect(cssKey("plain")).toBe("plain");
  });
});

describe("fmtDateCl", () => {
  it("echoes the raw string on an unparseable date (never 'Invalid Date')", () => {
    expect(fmtDateCl("not-a-date")).toBe("not-a-date");
    expect(fmtDateCl("")).toBe("");
  });
  it("formats a valid ISO timestamp to a non-empty es-CL string", () => {
    const out = fmtDateCl("2026-06-14T13:45:00Z");
    expect(out).not.toBe("2026-06-14T13:45:00Z");
    expect(out.length).toBeGreaterThan(0);
  });
});

describe("cafTone (CAF/folio supply)", () => {
  it("depleted (<=0) is danger", () => {
    expect(cafTone(0, 50)).toBe("pill-danger");
    expect(cafTone(-3, 50)).toBe("pill-danger");
  });
  it("at or below the low threshold is warn", () => {
    expect(cafTone(50, 50)).toBe("pill-warn");
    expect(cafTone(1, 50)).toBe("pill-warn");
    expect(cafTone(20, 20)).toBe("pill-warn"); // factura threshold
  });
  it("above the threshold is ok", () => {
    expect(cafTone(51, 50)).toBe("pill-ok");
    expect(cafTone(9999, 50)).toBe("pill-ok");
  });
});

describe("estadoBadge", () => {
  it("genders the label per kind (boleta=fem, documento=masc)", () => {
    expect(estadoBadge("signed", "boleta").label).toBe("Firmada");
    expect(estadoBadge("signed", "documento").label).toBe("Firmado");
    expect(estadoBadge("sent", "boleta").label).toBe("Enviada SII");
    expect(estadoBadge("sent", "documento").label).toBe("Enviado SII");
  });
  it("maps the tone class per estado", () => {
    expect(estadoBadge("accepted").cls).toBe("pill pill-ok");
    expect(estadoBadge("rejected").cls).toBe("pill pill-danger");
    expect(estadoBadge("sent").cls).toBe("pill pill-warn");
    expect(estadoBadge("cancelled").cls).toBe("pill");
  });
  it("falls back to the raw value + neutral pill for an unknown estado", () => {
    expect(estadoBadge("nuevo_estado_servidor")).toEqual({
      label: "nuevo_estado_servidor",
      cls: "pill",
    });
  });
});

describe("computeDocTotals (neto/IVA parity with server desglose_iva)", () => {
  it("splits an exact affected total (11900 → 10000 neto + 1900 IVA)", () => {
    const t = computeDocTotals([{ cantidad: "1", precio: "11900", exento: false }]);
    expect(t).toEqual({ neto: 10000, iva: 1900, exento: 0, total: 11900 });
  });
  it("makes IVA absorb the rounding (1000 → 840 + 160)", () => {
    const t = computeDocTotals([{ cantidad: "1", precio: "1000", exento: false }]);
    expect(t.neto + t.iva).toBe(1000);
    expect(t).toEqual({ neto: 840, iva: 160, exento: 0, total: 1000 });
  });
  it("keeps exento out of the IVA base and in the total", () => {
    const t = computeDocTotals([
      { cantidad: "2", precio: "1190", exento: false }, // afecto 2380
      { cantidad: "1", precio: "5000", exento: true }, // exento 5000
    ]);
    expect(t.exento).toBe(5000);
    expect(t.neto + t.iva).toBe(2380);
    expect(t.total).toBe(7380);
  });
  it("truncates each line to integer CLP before summing", () => {
    const t = computeDocTotals([{ cantidad: "3", precio: "333.99", exento: false }]);
    expect(t.total).toBe(Math.trunc(3 * 333.99)); // 1001
  });
  it("skips half-typed / garbage / non-positive lines (no NaN leak)", () => {
    const t = computeDocTotals([
      { cantidad: "", precio: "1000", exento: false },
      { cantidad: "abc", precio: "1000", exento: false },
      { cantidad: "0", precio: "1000", exento: false },
      { cantidad: "1", precio: "-5", exento: false },
      { cantidad: "1", precio: "1190", exento: false },
    ]);
    expect(t.neto + t.iva).toBe(1190);
    expect(Number.isNaN(t.total)).toBe(false);
  });
  it("empty cart yields all zeros", () => {
    expect(computeDocTotals([])).toEqual({ neto: 0, iva: 0, exento: 0, total: 0 });
  });
});

describe("upgradeNote (402 upsell — no crash, no dark pattern)", () => {
  it("prefixes a calm plan upsell on FEATURE_REQUIRES_UPGRADE", () => {
    const r = upgradeNote("FEATURE_REQUIRES_UPGRADE|Activa el plan.", "Pro");
    expect(r.isUpsell).toBe(true);
    expect(r.message).toBe("Plan Pro requerido para envío automático. Activa el plan.");
  });
  it("uses the requested plan name (Business for facturas)", () => {
    expect(upgradeNote("FEATURE_REQUIRES_UPGRADE|x", "Business").message).toContain("Plan Business");
  });
  it("passes a non-gated coded error through untouched", () => {
    const r = upgradeNote("CONFLICT|Folio ya usado.", "Pro");
    expect(r.isUpsell).toBe(false);
    expect(r.message).toBe("Folio ya usado.");
  });
  it("handles a bare (non-coded) error string without crashing", () => {
    const r = upgradeNote("boom", "Pro");
    expect(r.isUpsell).toBe(false);
    expect(r.message).toBe("boom");
  });
});

describe("pickToday (reports empty / TZ edge / large data)", () => {
  const today = new Date().toISOString().slice(0, 10);
  it("returns undefined for no rows (empty data → caller shows $0 card)", () => {
    expect(pickToday([])).toBeUndefined();
  });
  it("picks the row dated today when present", () => {
    const rows = [{ date: "2020-01-01", v: 1 }, { date: today, v: 2 }];
    expect(pickToday(rows)?.v).toBe(2);
  });
  it("falls back to the last row when none match today (TZ edge never blanks)", () => {
    const rows = [{ date: "2020-01-01", v: 1 }, { date: "2020-01-02", v: 9 }];
    expect(pickToday(rows)?.v).toBe(9);
  });
  it("scans a large series without error", () => {
    const rows = Array.from({ length: 5000 }, (_, i) => ({ date: `d${i}`, v: i }));
    rows.push({ date: today, v: -1 });
    expect(pickToday(rows)?.v).toBe(-1);
  });
});
