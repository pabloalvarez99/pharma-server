import { describe, it, expect } from "vitest";
import {
  toRfc3339Noon,
  stockLevel,
  expiryStatus,
  pharmaFieldsVisible,
  csvField,
  toCsv,
  buildInventoryExport,
  exportFilename,
  inventoryEmpty,
  comprasEmpty,
  gastosEmpty,
  classifyFetchError,
  type ExportProduct,
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

// --- CSV primitives ---------------------------------------------------------

describe("csvField (RFC-4180 + injection guard)", () => {
  it("passes plain values through untouched", () => {
    expect(csvField("Paracetamol")).toBe("Paracetamol");
    expect(csvField(1990)).toBe("1990");
    expect(csvField(true)).toBe("true");
  });

  it("renders null/undefined as an empty field", () => {
    expect(csvField(null)).toBe("");
    expect(csvField(undefined)).toBe("");
  });

  it("quotes and escapes embedded comma / quote / newline", () => {
    expect(csvField("Gasa, estéril")).toBe('"Gasa, estéril"');
    expect(csvField('Jarabe "forte"')).toBe('"Jarabe ""forte"""');
    expect(csvField("línea1\nlínea2")).toBe('"línea1\nlínea2"');
  });

  it("neutralizes a spreadsheet formula-injection prefix with a tab", () => {
    // A product named "=cmd()" must NOT be a live formula when opened in Excel.
    expect(csvField("=SUM(A1)")).toBe("\t=SUM(A1)");
    expect(csvField("+1")).toBe("\t+1");
    expect(csvField("@x")).toBe("\t@x");
  });
});

describe("toCsv", () => {
  it("joins header + rows with CRLF and comma fields", () => {
    const csv = toCsv(["a", "b"], [
      [1, 2],
      ["x", "y"],
    ]);
    expect(csv).toBe("a,b\r\n1,2\r\nx,y");
  });
});

// --- inventory export (vendor-agnostic: owner keeps their data) -------------

const PHARMA: ExportProduct[] = [
  { id: "product:1", name: "Paracetamol 500mg", price: "1990", stock: 42, active: true, laboratory: "Lab Chile", active_ingredient: "Paracetamol" },
  { id: "product:2", name: "Gasa, estéril", price: "990", stock: 0, active: false, laboratory: null, active_ingredient: null },
];

const MINIMARKET: ExportProduct[] = [
  { id: "product:9", name: "Pan amasado", price: "1200", stock: 30, active: true },
];

describe("buildInventoryExport — JOURNEY: pharmacy owner exports CSV", () => {
  it("includes pharma columns + Spanish header for a pharmacy vertical", () => {
    const b = buildInventoryExport(PHARMA, true);
    const [header, row1] = b.csv.split("\r\n");
    expect(header).toBe("id,nombre,precio,stock,activo,laboratorio,principio_activo");
    expect(row1).toBe("product:1,Paracetamol 500mg,1990,42,sí,Lab Chile,Paracetamol");
    expect(b.count).toBe(2);
  });

  it("keeps money as the raw Decimal string (re-imports losslessly, no locale)", () => {
    const b = buildInventoryExport(PHARMA, true);
    expect(b.csv).toContain(",1990,"); // not "$1.990"
  });

  it("escapes a product name with a comma so columns don't shift", () => {
    const b = buildInventoryExport(PHARMA, true);
    expect(b.csv).toContain('"Gasa, estéril"');
  });
});

describe("buildInventoryExport — JOURNEY: minimarket owner exports", () => {
  it("omits pharma columns for a non-pharmacy vertical", () => {
    const b = buildInventoryExport(MINIMARKET, false);
    const [header] = b.csv.split("\r\n");
    expect(header).toBe("id,nombre,precio,stock,activo");
    expect(b.csv).not.toContain("laboratorio");
  });

  it("produces JSON without pharma keys, round-trippable", () => {
    const b = buildInventoryExport(MINIMARKET, false);
    const parsed = JSON.parse(b.json);
    expect(parsed).toEqual([
      { id: "product:9", name: "Pan amasado", price: "1200", stock: 30, active: true },
    ]);
  });
});

describe("buildInventoryExport — edge cases", () => {
  it("an empty catalog yields a header-only CSV and []", () => {
    const b = buildInventoryExport([], true);
    expect(b.csv).toBe("id,nombre,precio,stock,activo,laboratorio,principio_activo");
    expect(JSON.parse(b.json)).toEqual([]);
    expect(b.count).toBe(0);
    expect(b.truncated).toBe(false);
  });

  it("flags truncated when the list fills the page cap exactly", () => {
    expect(buildInventoryExport(MINIMARKET, false, 1).truncated).toBe(true);
    expect(buildInventoryExport(MINIMARKET, false, 2).truncated).toBe(false);
    expect(buildInventoryExport(MINIMARKET, false).truncated).toBe(false);
  });

  it("JSON keeps null pharma fields (not dropped) for a pharmacy export", () => {
    const parsed = JSON.parse(buildInventoryExport(PHARMA, true).json);
    expect(parsed[1].laboratory).toBeNull();
    expect(parsed[1].active_ingredient).toBeNull();
  });
});

describe("exportFilename", () => {
  it("stems a prefix with the local YYYY-MM-DD date", () => {
    expect(exportFilename("inventario", new Date(2026, 5, 14))).toBe("inventario-2026-06-14");
  });
});

// --- empty states (never a blank screen) ------------------------------------

describe("empty-state copy — JOURNEY: fresh install vs active filter", () => {
  it("inventory: no data offers a create CTA; a search shows 'no matches' w/o CTA", () => {
    expect(inventoryEmpty(false).cta).toBe("+ Nuevo producto");
    expect(inventoryEmpty(true).cta).toBeUndefined();
    expect(inventoryEmpty(true).title).toBe("Sin coincidencias");
  });

  it("compras: no data offers '+ Nueva OC'; filtered does not", () => {
    expect(comprasEmpty(false).cta).toBe("+ Nueva OC");
    expect(comprasEmpty(true).cta).toBeUndefined();
  });

  it("gastos: no data offers 'Nuevo gasto'; filtered does not", () => {
    expect(gastosEmpty(false).cta).toBe("Nuevo gasto");
    expect(gastosEmpty(true).cta).toBeUndefined();
  });

  it("every empty copy is Spanish and non-blank", () => {
    for (const c of [inventoryEmpty(false), comprasEmpty(true), gastosEmpty(false)]) {
      expect(c.title.length).toBeGreaterThan(0);
      expect(c.hint.length).toBeGreaterThan(0);
    }
  });
});

// --- error classification (server down / no permission) ---------------------

describe("classifyFetchError — JOURNEY: server caído / sin permiso", () => {
  it("maps the Tauri offline string to a retry hint, not a raw crash", () => {
    const c = classifyFetchError(
      "No se pudo conectar al servidor. Verifica la URL y que pharma-server esté corriendo.",
    );
    expect(c.kind).toBe("offline");
    expect(c.title).toBe("Sin conexión al servidor");
  });

  it("maps a 403 / 'denegado' to a 'sin acceso' message with the resource", () => {
    expect(classifyFetchError("403 Permiso denegado", "las compras")).toMatchObject({
      kind: "forbidden",
      hint: expect.stringContaining("las compras"),
    });
  });

  it("keeps an unclassified message verbatim (nothing real is swallowed)", () => {
    const c = classifyFetchError("Respuesta inválida del servidor");
    expect(c.kind).toBe("generic");
    expect(c.hint).toBe("Respuesta inválida del servidor");
  });

  it("never throws on a non-string rejection", () => {
    const c = classifyFetchError(new Error("boom"));
    expect(c.kind).toBe("generic");
    expect(c.hint.length).toBeGreaterThan(0);
  });
});
