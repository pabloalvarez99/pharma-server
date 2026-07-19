// Unit tests for the shared, view-agnostic export builder (sin lock-in plumbing).
// Pure (no DOM) — assert the CSV/JSON the owner downloads is correct, open, and
// re-importable: es-CL headers, RFC-4180 escaping, money as raw STRING, CSV
// injection neutralised. The download trigger itself is a no-op outside a browser
// (guarded in export-helpers) so these run under plain vitest.
import { describe, it, expect } from "vitest";
import { buildExport, exportRows, type ExportColumn } from "./export-helpers";
import { GASTO_EXPORT_COLUMNS } from "./gastos";
import type { Expense } from "../api";

interface Row {
  name: string;
  amount: string;
  note: string;
}

const COLS: ExportColumn<Row>[] = [
  { header: "nombre", key: "name", value: (r) => r.name },
  { header: "monto", key: "amount", value: (r) => r.amount },
  { header: "nota", key: "note", value: (r) => r.note },
];

describe("buildExport", () => {
  it("writes es-CL headers then one CSV line per row (CRLF, RFC-4180)", () => {
    const { csv, count } = buildExport(COLS, [
      { name: "Arriendo", amount: "150000", note: "mayo" },
      { name: "Luz", amount: "32990", note: "" },
    ]);
    expect(count).toBe(2);
    const lines = csv.split("\r\n");
    expect(lines[0]).toBe("nombre,monto,nota");
    expect(lines[1]).toBe("Arriendo,150000,mayo");
    expect(lines[2]).toBe("Luz,32990,");
  });

  it("keeps money as the raw Decimal STRING (no locale formatting → re-imports)", () => {
    const { csv, json } = buildExport(COLS, [{ name: "X", amount: "1234567", note: "" }]);
    expect(csv).toContain("1234567");
    expect(csv).not.toContain("1.234.567");
    expect(JSON.parse(json)[0].amount).toBe("1234567");
  });

  it("escapes commas, quotes and newlines per RFC-4180", () => {
    const { csv } = buildExport(COLS, [
      { name: 'Pérez, Juan', amount: "10", note: 'dijo "hola"\nchao' },
    ]);
    expect(csv).toContain('"Pérez, Juan"');
    expect(csv).toContain('"dijo ""hola""\nchao"');
  });

  it("neutralises CSV/formula injection (leading = + - @)", () => {
    const { csv } = buildExport(COLS, [{ name: "=SUM(A1:A9)", amount: "0", note: "" }]);
    // csvField prefixes a tab so a spreadsheet treats it as text, not a formula.
    expect(csv).toContain("\t=SUM(A1:A9)");
  });

  it("JSON round-trips under stable machine keys (not the es-CL headers)", () => {
    const { json } = buildExport(COLS, [{ name: "Y", amount: "5", note: "n" }]);
    const parsed = JSON.parse(json);
    expect(parsed).toEqual([{ name: "Y", amount: "5", note: "n" }]);
  });

  it("an empty list still produces a header-only CSV and []", () => {
    const { csv, json, count } = buildExport(COLS, []);
    expect(count).toBe(0);
    expect(csv).toBe("nombre,monto,nota");
    expect(JSON.parse(json)).toEqual([]);
  });
});

describe("exportRows (no-DOM environment)", () => {
  it("returns the built data and does not throw without a document", () => {
    const data = exportRows("test", COLS, [{ name: "A", amount: "1", note: "" }]);
    expect(data.count).toBe(1);
    expect(data.csv.split("\r\n")[0]).toBe("nombre,monto,nota");
  });
});

describe("gastos export columns", () => {
  const expense = (over: Partial<Expense>): Expense =>
    ({
      id: "expense:1",
      category: "rent",
      description: "Arriendo local",
      amount: "150000",
      payment_method: "cash",
      incurred_at: "2026-06-20T12:00:00Z",
      ...over,
    }) as Expense;

  it("maps category/payment to es-CL labels and money to the raw string", () => {
    const { csv } = buildExport(GASTO_EXPORT_COLUMNS, [expense({})]);
    const [header, row] = csv.split("\r\n");
    expect(header).toBe("categoria,descripcion,medio_pago,monto,fecha");
    expect(row).toBe("Arriendo,Arriendo local,Efectivo,150000,2026-06-20T12:00:00Z");
  });

  it("falls back to the raw code for an unknown category/method", () => {
    const { csv } = buildExport(GASTO_EXPORT_COLUMNS, [
      expense({ category: "weird", payment_method: "crypto", description: "" }),
    ]);
    expect(csv.split("\r\n")[1]).toBe("weird,,crypto,150000,2026-06-20T12:00:00Z");
  });
});
