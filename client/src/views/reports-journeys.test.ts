// User-journey tests for the REPORTES view presentation + export logic
// (reports.ts delegates panel shaping, the Pro-gate classification, and the
// vendor-agnostic CSV/JSON export to reports-helpers.ts). A green run means what
// the dueño/contador SEES and EXPORTS — the day's KPIs, the Pareto ranking, the
// rotation "—" fallbacks, the Pro-locked márgenes upsell, the downloadable
// files — is correct, not just the underlying numbers. Both verticals
// (pharmacy + minimarket) are exercised where the rubro changes the data; the
// shaping/export logic is vertical-agnostic by design (boleta/reports = universal).
import { describe, it, expect } from "vitest";
import type {
  DailySalesRow,
  DailyMarginRow,
  TopProductRow,
  StockRotationRow,
  InventorySummary,
} from "../api";
import {
  pickTodayRow,
  classifyMarginError,
  abcToken,
  rotationDisplay,
  buildSalesExport,
  buildTopExport,
  buildRotationExport,
  buildReportsJson,
} from "./reports-helpers";

const TODAY = "2026-06-14";

// --------------------------------------------------------------------------
// Journey 1 — VENTAS DE HOY: el panel toma la fila de HOY de la serie diaria,
//   y si no hay fila para hoy (borde TZ) cae a la última, nunca se blanquea.
// --------------------------------------------------------------------------
describe("journey · reporte ventas de hoy — fila correcta + export", () => {
  const series: DailySalesRow[] = [
    { date: "2026-06-12", orders: 3, revenue: "30000", cash: "20000", card: "10000" },
    { date: "2026-06-13", orders: 5, revenue: "55000", cash: "40000", card: "15000" },
    { date: TODAY, orders: 7, revenue: "88000", cash: "60000", card: "28000" },
  ];

  it("elige la fila de hoy, no la última por accidente", () => {
    const row = pickTodayRow(series, TODAY);
    expect(row?.date).toBe(TODAY);
    expect(row?.orders).toBe(7);
  });

  it("cae a la última fila si hoy no está en la serie (borde de zona horaria)", () => {
    const row = pickTodayRow(series, "2026-06-15");
    expect(row?.date).toBe(TODAY); // la última, no undefined
  });

  it("serie vacía → undefined (el panel muestra '$0 sin ventas')", () => {
    expect(pickTodayRow([], TODAY)).toBeUndefined();
  });

  it("export CSV de ventas: header en español, montos crudos (sin formato CLP)", () => {
    const { csv, count } = buildSalesExport(series);
    const lines = csv.split("\r\n");
    expect(lines[0]).toBe("fecha,boletas,ingresos,efectivo,tarjeta");
    expect(lines[3]).toBe(`${TODAY},7,88000,60000,28000`); // raw string, re-importable
    expect(count).toBe(3);
  });
});

// --------------------------------------------------------------------------
// Journey 2 — MÁRGENES Pro-gated: en Free el server rechaza con
//   FEATURE_REQUIRES_UPGRADE → el panel muestra upsell calmo, NO crash. En Pro
//   (entitled) ve la fila. Un error real (red caída) NO se confunde con gating.
// --------------------------------------------------------------------------
describe("journey · márgenes Pro-gated — Free ve upsell, Pro ve el dato", () => {
  it("Free: FEATURE_REQUIRES_UPGRADE → gated:true (upsell, sin crash)", () => {
    const r = classifyMarginError("FEATURE_REQUIRES_UPGRADE|Disponible en plan Pro.");
    expect(r.gated).toBe(true);
    expect(r.message).toBe("Disponible en plan Pro.");
  });

  it("error de red NO es gating → gated:false (se muestra como error real)", () => {
    const r = classifyMarginError("No se pudo conectar con el servidor.");
    expect(r.gated).toBe(false);
    expect(r.message).toBe("No se pudo conectar con el servidor.");
  });

  it("Pro/entitled: la fila de hoy se selecciona como cualquier serie diaria", () => {
    const rows: DailyMarginRow[] = [
      { date: TODAY, revenue: "88000", cost: "52000", margin: "36000", margin_pct: "40.9", items_without_cost: 0 },
    ];
    const today = pickTodayRow(rows, TODAY);
    expect(today?.margin).toBe("36000");
    expect(today?.items_without_cost).toBe(0);
  });
});

// --------------------------------------------------------------------------
// Journey 3 — TOP PRODUCTOS (Pareto ABC): el ranking normaliza la clase A/B/C
//   defensivamente y exporta el ranking. Universal a ambos verticales.
// --------------------------------------------------------------------------
describe("journey · top productos ABC — ranking + export (ambos verticales)", () => {
  it("normaliza la clase ABC (case/espacios), desconocida → C", () => {
    expect(abcToken("A")).toBe("a");
    expect(abcToken(" b ")).toBe("b");
    expect(abcToken("")).toBe("c");
    expect(abcToken("Z")).toBe("c");
  });

  it("farmacia: export CSV con tildes escapadas y porcentaje crudo", () => {
    const rows: TopProductRow[] = [
      { rank: 1, product_id: "p1", product_name: "Paracetamol 500mg", qty_sold: 120, revenue: "240000", revenue_pct: "48.0", abc_class: "A" },
      { rank: 2, product_id: "p2", product_name: "Ibuprofeno", qty_sold: 60, revenue: "120000", revenue_pct: "24.0", abc_class: "b" },
    ];
    const { csv, count } = buildTopExport(rows);
    const lines = csv.split("\r\n");
    expect(lines[0]).toBe("rank,producto,unidades,ingresos,pct_ingresos,abc");
    expect(lines[1]).toBe("1,Paracetamol 500mg,120,240000,48.0,A");
    expect(lines[2]).toBe("2,Ibuprofeno,60,120000,24.0,B");
    expect(count).toBe(2);
  });

  it("minimarket: el mismo ranking funciona sin campos clínicos", () => {
    const rows: TopProductRow[] = [
      { rank: 1, product_id: "a1", product_name: "Coca-Cola 1.5L", qty_sold: 200, revenue: "300000", revenue_pct: "60.0", abc_class: "A" },
    ];
    const { json } = buildTopExport(rows);
    expect(JSON.parse(json)[0].product_name).toBe("Coca-Cola 1.5L");
  });

  it("CSV-injection guard: nombre que empieza con '=' se neutraliza (fórmula)", () => {
    const rows: TopProductRow[] = [
      { rank: 1, product_id: "x", product_name: "=cmd()", qty_sold: 1, revenue: "100", revenue_pct: "1.0", abc_class: "C" },
    ];
    const { csv } = buildTopExport(rows);
    expect(csv).toContain("\t=cmd()"); // prefijo tab → la planilla lo trata como texto
  });
});

// --------------------------------------------------------------------------
// Journey 4 — ROTACIÓN DE STOCK: turnover/días nulos se muestran como "—" y
//   se exportan vacíos (no "null"). Días se redondea a entero.
// --------------------------------------------------------------------------
describe("journey · rotación — fallbacks '—' + export limpio", () => {
  const rows: StockRotationRow[] = [
    { product_id: "p1", product_name: "Amoxicilina", qty_sold: 40, current_stock: 10, turnover: "4.0", days_of_inventory: "91.4" },
    { product_id: "p2", product_name: "Producto nuevo", qty_sold: 0, current_stock: 5, turnover: null, days_of_inventory: null },
  ];

  it("display: turnover 'N×' y días redondeado; nulos → '—'", () => {
    expect(rotationDisplay(rows[0])).toEqual({ turnover: "4.0×", days: "91" });
    expect(rotationDisplay(rows[1])).toEqual({ turnover: "—", days: "—" });
  });

  it("export: nulos quedan vacíos (no la cadena 'null') para re-importar limpio", () => {
    const { csv } = buildRotationExport(rows);
    const lines = csv.split("\r\n");
    expect(lines[0]).toBe("producto,vendidas,stock,rotacion,dias_inventario");
    expect(lines[1]).toBe("Amoxicilina,40,10,4.0,91.4");
    expect(lines[2]).toBe("Producto nuevo,0,5,,"); // turnover/días vacíos
  });
});

// --------------------------------------------------------------------------
// Journey 5 — INVENTARIO (reusa /products/stats) + EXPORTAR TODO: el bundle JSON
//   combina solo los paneles cargados; un panel gated/no-cargado se omite.
// --------------------------------------------------------------------------
describe("journey · exportar todo — bundle combinado vendor-agnostic", () => {
  const now = new Date("2026-06-14T12:00:00Z");
  const sales: DailySalesRow[] = [{ date: TODAY, orders: 7, revenue: "88000", cash: "60000", card: "28000" }];
  const inventory: InventorySummary = {
    total: 16, active: 16, low_stock: 2, out_of_stock: 1, inventory_value: "1250000", expired: 0,
  };

  it("incluye solo lo cargado; márgenes Pro-locked se marca gated (no error)", () => {
    const json = buildReportsJson({ sales, inventory, margins_gated: true }, now);
    const b = JSON.parse(json);
    expect(b.generated_at).toBe("2026-06-14T12:00:00.000Z");
    expect(b.sales_daily).toHaveLength(1);
    expect(b.inventory.total).toBe(16);
    expect(b.margins_daily).toEqual({ gated: true }); // ambiguo no: locked, no fallo
    expect(b.top_products).toBeUndefined(); // panel no cargado → omitido
    expect(b.stock_rotation).toBeUndefined();
  });

  it("Pro entitled: márgenes cargado se exporta como datos, no como gated", () => {
    const margins: DailyMarginRow[] = [
      { date: TODAY, revenue: "88000", cost: "52000", margin: "36000", margin_pct: "40.9", items_without_cost: 0 },
    ];
    const b = JSON.parse(buildReportsJson({ sales, margins }, now));
    expect(b.margins_daily).toHaveLength(1);
    expect(b.margins_daily[0].margin).toBe("36000");
  });

  it("bundle vacío (nada cargado todavía) → solo el timestamp, sin crash", () => {
    const b = JSON.parse(buildReportsJson({}, now));
    expect(Object.keys(b)).toEqual(["generated_at"]);
  });
});

// --------------------------------------------------------------------------
// Journey 6 — ESTADOS VACÍOS: cada reporte tabular maneja el feed vacío sin
//   romper el export (count 0, CSV solo-header).
// --------------------------------------------------------------------------
describe("journey · estados vacíos — export no rompe con feed vacío", () => {
  it("top vacío → CSV solo header, count 0", () => {
    const { csv, count } = buildTopExport([]);
    expect(csv).toBe("rank,producto,unidades,ingresos,pct_ingresos,abc");
    expect(count).toBe(0);
  });

  it("rotación vacía → CSV solo header, count 0", () => {
    const { csv, count } = buildRotationExport([]);
    expect(csv).toBe("producto,vendidas,stock,rotacion,dias_inventario");
    expect(count).toBe(0);
  });

  it("ventas vacías → CSV solo header (el panel muestra '$0' aparte)", () => {
    const { csv } = buildSalesExport([]);
    expect(csv).toBe("fecha,boletas,ingresos,efectivo,tarjeta");
  });
});
