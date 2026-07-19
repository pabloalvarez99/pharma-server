// Tests for the ACTIONABLE INSIGHT math behind Reportes (reports-insights.ts).
// A green run means the numbers the dueño acts on — the day-over-day deltas, the
// money-at-risk by expiry, the immobilized stock, the urgency ranking — are
// correct, and each card actually states qué hacer (an action line, owner
// voice). The shaping is vertical-agnostic, so a pharmacy lote and a minimarket
// batch are both exercised where the rubro changes the data.
import { describe, it, expect } from "vitest";
import type {
  DailySalesRow,
  DailyMarginRow,
  StockRotationRow,
  TopProductRow,
  NearExpiryRow,
  Product,
} from "../api";
import {
  pctDelta,
  priceMap,
  computeExpiryExposure,
  computeStalledStock,
  computeReorder,
  computeMarginTrend,
  computePeakDay,
  salesDeltaInsight,
  marginDeltaInsight,
  marginGatedInsight,
  expiredInsight,
  nearExpiryInsight,
  stalledInsight,
  topSellerInsight,
  reorderInsight,
  marginTrendInsight,
  peakDayInsight,
  buildInsights,
} from "./reports-insights";

const TODAY = "2026-06-19";
const YESTERDAY = "2026-06-18";

// --------------------------------------------------------------------------
// pctDelta — the core trend primitive. Never divides by zero, rounds to 1dp.
// --------------------------------------------------------------------------
describe("pctDelta — variación porcentual segura", () => {
  it("alza: 88000 vs 80000 → +10.0%", () => {
    expect(pctDelta(88000, 80000)).toBe(10);
  });
  it("baja: 73600 vs 80000 → -8.0%", () => {
    expect(pctDelta(73600, 80000)).toBe(-8);
  });
  it("base 0 → null (no inventa un +∞%)", () => {
    expect(pctDelta(5000, 0)).toBeNull();
  });
  it("base negativa/garbage → null", () => {
    expect(pctDelta(100, -1)).toBeNull();
  });
});

// --------------------------------------------------------------------------
// priceMap — id → precio de venta, garbage → 0 (nunca NaN).
// --------------------------------------------------------------------------
describe("priceMap — valorización por id", () => {
  const products: Product[] = [
    { id: "p1", name: "Paracetamol", price: "1990", stock: 10, active: true, laboratory: null, active_ingredient: null },
    { id: "p2", name: "Sin precio", price: "", stock: 5, active: true, laboratory: null, active_ingredient: null },
  ];
  it("mapea precio numérico y cae a 0 en precio vacío", () => {
    const m = priceMap(products);
    expect(m.get("p1")).toBe(1990);
    expect(m.get("p2")).toBe(0);
    expect(m.get("desconocido")).toBeUndefined();
  });
});

// --------------------------------------------------------------------------
// computeExpiryExposure — el $ expuesto por vencimiento (vencido vs por vencer).
// --------------------------------------------------------------------------
describe("computeExpiryExposure — dinero en riesgo por vencer", () => {
  const prices = new Map<string, number>([
    ["p1", 1000],
    ["p2", 2000],
    ["p3", 500],
  ]);
  const rows: NearExpiryRow[] = [
    // ya vencido
    { product_id: "p1", product_name: "Amoxi", batch_id: "b1", batch_code: "L1", expiry_date: "2026-06-10", stock: 4, days_to_expiry: -9, expired: true },
    // por vencer dentro del horizonte (≤30)
    { product_id: "p2", product_name: "Ibupro", batch_id: "b2", batch_code: "L2", expiry_date: "2026-07-05", stock: 3, days_to_expiry: 16, expired: false },
    // fuera del horizonte → no cuenta
    { product_id: "p3", product_name: "Vit C", batch_id: "b3", batch_code: "L3", expiry_date: "2026-09-01", stock: 10, days_to_expiry: 74, expired: false },
    // stock 0 → ignorado
    { product_id: "p2", product_name: "Ibupro", batch_id: "b4", batch_code: "L4", expiry_date: "2026-07-02", stock: 0, days_to_expiry: 13, expired: false },
  ];

  it("separa vencido ($ y unidades) de en-riesgo dentro del horizonte", () => {
    const x = computeExpiryExposure(rows, prices, 30);
    expect(x.expiredValue).toBe(4000); // 4 × 1000
    expect(x.expiredUnits).toBe(4);
    expect(x.expiredProducts).toBe(1);
    expect(x.atRiskValue).toBe(6000); // 3 × 2000
    expect(x.atRiskUnits).toBe(3);
    expect(x.atRiskProducts).toBe(1); // p3 fuera de horizonte; p2-stock0 ignorado
    expect(x.horizonDays).toBe(30);
  });

  it("precio desconocido suma en unidades pero 0 en valor (no NaN)", () => {
    const x = computeExpiryExposure(rows, new Map(), 30);
    expect(x.expiredValue).toBe(0);
    expect(x.expiredUnits).toBe(4); // sigue visible en unidades
    expect(x.atRiskValue).toBe(0);
    expect(x.atRiskUnits).toBe(3);
  });
});

// --------------------------------------------------------------------------
// computeStalledStock — capital congelado: stock sin ventas en el período.
// --------------------------------------------------------------------------
describe("computeStalledStock — stock parado", () => {
  const prices = new Map<string, number>([["s1", 3000], ["s2", 1000]]);
  const rows: StockRotationRow[] = [
    { product_id: "s1", product_name: "Parado", qty_sold: 0, current_stock: 8, turnover: null, days_of_inventory: null },
    { product_id: "s2", product_name: "Vende", qty_sold: 12, current_stock: 4, turnover: "3.0", days_of_inventory: "30" },
    { product_id: "s3", product_name: "Parado sin stock", qty_sold: 0, current_stock: 0, turnover: null, days_of_inventory: null },
  ];
  it("cuenta solo lo que tiene stock y no vendió; valoriza por precio", () => {
    const s = computeStalledStock(rows, prices);
    expect(s.count).toBe(1); // solo s1 (s2 vendió; s3 sin stock)
    expect(s.units).toBe(8);
    expect(s.value).toBe(24000); // 8 × 3000
  });
});

// --------------------------------------------------------------------------
// salesDeltaInsight — ventas hoy vs ayer, con acción.
// --------------------------------------------------------------------------
describe("salesDeltaInsight — tendencia de ventas accionable", () => {
  it("baja → tono warn + acción de revisar vitrina/promos", () => {
    const rows: DailySalesRow[] = [
      { date: YESTERDAY, orders: 8, revenue: "80000", cash: "50000", card: "30000" },
      { date: TODAY, orders: 6, revenue: "73600", cash: "40000", card: "33600" },
    ];
    const card = salesDeltaInsight(rows, TODAY)!;
    expect(card.tone).toBe("warn");
    expect(card.title).toContain("8.0%");
    expect(card.title.toLowerCase()).toContain("abajo");
    expect(card.action.length).toBeGreaterThan(0);
  });

  it("alza → tono good", () => {
    const rows: DailySalesRow[] = [
      { date: YESTERDAY, orders: 8, revenue: "80000", cash: "50000", card: "30000" },
      { date: TODAY, orders: 10, revenue: "88000", cash: "60000", card: "28000" },
    ];
    const card = salesDeltaInsight(rows, TODAY)!;
    expect(card.tone).toBe("good");
    expect(card.title).toContain("10.0%");
  });

  it("sin día previo → info (no inventa una variación)", () => {
    const rows: DailySalesRow[] = [
      { date: TODAY, orders: 5, revenue: "50000", cash: "50000", card: "0" },
    ];
    const card = salesDeltaInsight(rows, TODAY)!;
    expect(card.tone).toBe("info");
  });

  it("serie vacía → null", () => {
    expect(salesDeltaInsight([], TODAY)).toBeNull();
  });
});

// --------------------------------------------------------------------------
// marginDeltaInsight — margen en PUNTOS porcentuales (Pro).
// --------------------------------------------------------------------------
describe("marginDeltaInsight — variación del margen en puntos", () => {
  it("baja de margen → warn + acción de revisar precios/costos", () => {
    const rows: DailyMarginRow[] = [
      { date: YESTERDAY, revenue: "80000", cost: "44000", margin: "36000", margin_pct: "45.0", items_without_cost: 0 },
      { date: TODAY, revenue: "88000", cost: "55000", margin: "33000", margin_pct: "37.5", items_without_cost: 0 },
    ];
    const card = marginDeltaInsight(rows, TODAY)!;
    expect(card.tone).toBe("warn");
    expect(card.title).toContain("7.5");
    expect(card.action.toLowerCase()).toContain("costo");
  });

  it("suba de margen → good", () => {
    const rows: DailyMarginRow[] = [
      { date: YESTERDAY, revenue: "80000", cost: "48000", margin: "32000", margin_pct: "40.0", items_without_cost: 0 },
      { date: TODAY, revenue: "88000", cost: "48400", margin: "39600", margin_pct: "45.0", items_without_cost: 0 },
    ];
    expect(marginDeltaInsight(rows, TODAY)!.tone).toBe("good");
  });
});

// --------------------------------------------------------------------------
// Card builders sobre exposiciones — copy + tono + omisión cuando no aplica.
// --------------------------------------------------------------------------
describe("tarjetas de exposición — copy accionable y omisión", () => {
  it("expiredInsight: danger, $ en título, acción dar de baja", () => {
    const c = expiredInsight({ atRiskValue: 0, atRiskUnits: 0, atRiskProducts: 0, expiredValue: 4000, expiredUnits: 4, expiredProducts: 1, horizonDays: 30 })!;
    expect(c.tone).toBe("danger");
    expect(c.title).toContain("4.000"); // CLP es-CL agrupado
    expect(c.action.toLowerCase()).toContain("baja");
  });

  it("nearExpiryInsight: warn + horizonte en el detalle", () => {
    const c = nearExpiryInsight({ atRiskValue: 6000, atRiskUnits: 3, atRiskProducts: 1, expiredValue: 0, expiredUnits: 0, expiredProducts: 0, horizonDays: 30 })!;
    expect(c.tone).toBe("warn");
    expect(c.detail).toContain("30");
  });

  it("sin exposición → null (la tarjeta no se muestra)", () => {
    const zero = { atRiskValue: 0, atRiskUnits: 0, atRiskProducts: 0, expiredValue: 0, expiredUnits: 0, expiredProducts: 0, horizonDays: 30 };
    expect(expiredInsight(zero)).toBeNull();
    expect(nearExpiryInsight(zero)).toBeNull();
  });

  it("stalledInsight: warn cuando hay parado; null cuando no", () => {
    expect(stalledInsight({ count: 2, units: 12, value: 30000 })!.tone).toBe("warn");
    expect(stalledInsight({ count: 0, units: 0, value: 0 })).toBeNull();
  });

  it("topSellerInsight: good + nombre del producto estrella", () => {
    const rows: TopProductRow[] = [
      { rank: 1, product_id: "p1", product_name: "Coca-Cola 1.5L", qty_sold: 200, revenue: "300000", revenue_pct: "60.0", abc_class: "A" },
    ];
    const c = topSellerInsight(rows)!;
    expect(c.tone).toBe("good");
    expect(c.title).toContain("Coca-Cola 1.5L");
  });

  it("marginGatedInsight: tono pro (upsell), sin dead-end", () => {
    const c = marginGatedInsight();
    expect(c.tone).toBe("pro");
    expect(c.action.length).toBeGreaterThan(0);
  });
});

// --------------------------------------------------------------------------
// buildInsights — orquesta y ORDENA por urgencia; ambos verticales.
// --------------------------------------------------------------------------
describe("buildInsights — orquestación + ranking de urgencia", () => {
  const prices = new Map<string, number>([["p1", 1000], ["p2", 2000]]);
  const sales: DailySalesRow[] = [
    { date: YESTERDAY, orders: 8, revenue: "80000", cash: "50000", card: "30000" },
    { date: TODAY, orders: 6, revenue: "73600", cash: "40000", card: "33600" },
  ];
  const nearExpiry: NearExpiryRow[] = [
    { product_id: "p1", product_name: "Amoxi", batch_id: "b1", batch_code: "L1", expiry_date: "2026-06-10", stock: 4, days_to_expiry: -9, expired: true },
    { product_id: "p2", product_name: "Ibupro", batch_id: "b2", batch_code: "L2", expiry_date: "2026-07-05", stock: 3, days_to_expiry: 16, expired: false },
  ];
  const rotation: StockRotationRow[] = [
    { product_id: "p2", product_name: "Ibupro", qty_sold: 0, current_stock: 5, turnover: null, days_of_inventory: null },
  ];
  const top: TopProductRow[] = [
    { rank: 1, product_id: "p2", product_name: "Ibupro", qty_sold: 30, revenue: "60000", revenue_pct: "50.0", abc_class: "A" },
  ];

  it("Free (márgenes gated): vencido primero, upsell incluido, orden por urgencia", () => {
    const cards = buildInsights({
      sales, margins: null, marginsGated: true, nearExpiry, rotation, top, prices, today: TODAY,
    });
    const ids = cards.map((c) => c.id);
    expect(ids[0]).toBe("expired"); // dinero perdido primero
    expect(ids).toContain("near-expiry");
    expect(ids).toContain("sales-delta");
    expect(ids).toContain("margin-gated"); // Free ve upsell
    expect(ids).toContain("stalled");
    expect(ids).toContain("top-seller");
    // ranking: vencido < por-vencer < ventas < margen < parado < estrella
    expect(ids.indexOf("expired")).toBeLessThan(ids.indexOf("near-expiry"));
    expect(ids.indexOf("near-expiry")).toBeLessThan(ids.indexOf("sales-delta"));
  });

  it("Pro (márgenes con dato): aparece margin-delta, no el upsell", () => {
    const margins: DailyMarginRow[] = [
      { date: YESTERDAY, revenue: "80000", cost: "44000", margin: "36000", margin_pct: "45.0", items_without_cost: 0 },
      { date: TODAY, revenue: "73600", cost: "47000", margin: "26600", margin_pct: "36.1", items_without_cost: 0 },
    ];
    const ids = buildInsights({
      sales, margins, marginsGated: false, nearExpiry, rotation, top, prices, today: TODAY,
    }).map((c) => c.id);
    expect(ids).toContain("margin-delta");
    expect(ids).not.toContain("margin-gated");
  });

  it("minimarket sin datos → strip vacío (el view muestra placeholder calmo)", () => {
    const cards = buildInsights({
      sales: [], margins: null, marginsGated: false, nearExpiry: [], rotation: [], top: [], prices: new Map(), today: TODAY,
    });
    expect(cards).toEqual([]);
  });
});

// --------------------------------------------------------------------------
// computeReorder — sugerencia de reposición en $ (cuánto comprar y de qué).
// --------------------------------------------------------------------------
describe("computeReorder — reposición de los que se venden y se agotan", () => {
  const prices = new Map<string, number>([["a1", 1000], ["a2", 3000], ["a3", 500]]);
  const rows: StockRotationRow[] = [
    // mover por agotarse: stock 5 dura 5 días → rate 1/día; cubrir 30 → comprar 25.
    { product_id: "a1", product_name: "Mover rápido", qty_sold: 30, current_stock: 5, turnover: "6.0", days_of_inventory: "5" },
    // mover por agotarse, mayor $: stock 4 dura 8 días → rate 0.5; objetivo 15 → comprar 11; 11×3000=33000.
    { product_id: "a2", product_name: "Caro por agotarse", qty_sold: 10, current_stock: 4, turnover: "2.5", days_of_inventory: "8" },
    // ya cubierto (60 días) → no se repone.
    { product_id: "a3", product_name: "Bien surtido", qty_sold: 8, current_stock: 80, turnover: "0.1", days_of_inventory: "60" },
    // no vende → no es reposición (es stock parado).
    { product_id: "z", product_name: "Parado", qty_sold: 0, current_stock: 9, turnover: null, days_of_inventory: null },
  ];

  it("suma unidades y $ de los movers por agotarse; ignora cubiertos y parados", () => {
    const n = computeReorder(rows, prices, 30);
    expect(n.count).toBe(2); // a1 + a2
    expect(n.units).toBe(25 + 11);
    expect(n.value).toBe(25 * 1000 + 11 * 3000); // 58000
    expect(n.coverDays).toBe(30);
  });

  it("el «de qué» = el mayor contribuyente en $ (a2), no el de más unidades", () => {
    const n = computeReorder(rows, prices, 30);
    expect(n.topName).toBe("Caro por agotarse");
    expect(n.topUnits).toBe(11);
  });

  it("sin movers por agotarse → todo cero (no inventa una compra)", () => {
    const n = computeReorder([rows[2], rows[3]], prices, 30);
    expect(n).toEqual({ count: 0, units: 0, value: 0, topName: "", topUnits: 0, coverDays: 30 });
  });
});

// --------------------------------------------------------------------------
// computeMarginTrend — tendencia del margen MES vs MES (ponderado por ingreso).
// --------------------------------------------------------------------------
describe("computeMarginTrend — margen mensual ponderado", () => {
  const may: DailyMarginRow[] = [
    { date: "2026-05-10", revenue: "50000", cost: "30000", margin: "20000", margin_pct: "40.0", items_without_cost: 0 },
    { date: "2026-05-20", revenue: "50000", cost: "30000", margin: "20000", margin_pct: "40.0", items_without_cost: 0 },
  ];
  const jun: DailyMarginRow[] = [
    { date: "2026-06-10", revenue: "60000", cost: "42000", margin: "18000", margin_pct: "30.0", items_without_cost: 0 },
    { date: "2026-06-15", revenue: "40000", cost: "28000", margin: "12000", margin_pct: "30.0", items_without_cost: 0 },
  ];

  it("compara el mes más reciente vs el anterior (junio 30% vs mayo 40% → -10 pts)", () => {
    const t = computeMarginTrend([...may, ...jun])!;
    expect(t.currMonth).toBe("2026-06");
    expect(t.prevMonth).toBe("2026-05");
    expect(t.currPct).toBe(30);
    expect(t.prevPct).toBe(40);
    expect(t.pts).toBe(-10);
  });

  it("un solo mes → null (no hay comparación honesta)", () => {
    expect(computeMarginTrend(jun)).toBeNull();
  });
});

// --------------------------------------------------------------------------
// computePeakDay — día de la semana con más ventas (cuándo reforzar).
// --------------------------------------------------------------------------
describe("computePeakDay — día pico de ventas", () => {
  const rows: DailySalesRow[] = [
    { date: "2026-06-15", orders: 5, revenue: "10000", cash: "10000", card: "0" }, // lunes
    { date: "2026-06-16", orders: 5, revenue: "10000", cash: "10000", card: "0" }, // martes
    { date: "2026-06-17", orders: 5, revenue: "10000", cash: "10000", card: "0" }, // miércoles
    { date: "2026-06-18", orders: 5, revenue: "10000", cash: "10000", card: "0" }, // jueves
    { date: "2026-06-19", orders: 20, revenue: "50000", cash: "50000", card: "0" }, // viernes (pico)
    { date: "2026-06-20", orders: 5, revenue: "10000", cash: "10000", card: "0" }, // sábado
  ];

  it("elige el viernes y reporta su participación (50%)", () => {
    const p = computePeakDay(rows)!;
    expect(p.weekday).toBe("viernes");
    expect(p.value).toBe(50000);
    expect(p.sharePct).toBe(50);
  });

  it("menos de 5 días con venta → null (un pico de 2 días es ruido)", () => {
    expect(computePeakDay(rows.slice(0, 4))).toBeNull();
  });
});

// --------------------------------------------------------------------------
// Nuevas tarjetas — copy accionable + omisión cuando no aplica.
// --------------------------------------------------------------------------
describe("tarjetas nuevas — reposición, tendencia mensual, día pico", () => {
  it("reorderInsight: warn, $ en título, nombra qué comprar", () => {
    const c = reorderInsight({ count: 2, units: 36, value: 58000, topName: "Caro por agotarse", topUnits: 11, coverDays: 30 })!;
    expect(c.tone).toBe("warn");
    expect(c.title).toContain("58.000");
    expect(c.action).toContain("Caro por agotarse");
    expect(c.action).toContain("11");
  });

  it("reorderInsight: sin necesidad → null", () => {
    expect(reorderInsight({ count: 0, units: 0, value: 0, topName: "", topUnits: 0, coverDays: 30 })).toBeNull();
  });

  it("marginTrendInsight: baja mensual → warn + acción de costos/precios", () => {
    const c = marginTrendInsight({ currMonth: "2026-06", prevMonth: "2026-05", currPct: 30, prevPct: 40, pts: -10 })!;
    expect(c.tone).toBe("warn");
    expect(c.title).toContain("−10,0%"); // signo menos real
    expect(c.action.toLowerCase()).toContain("costo");
  });

  it("marginTrendInsight: sin cambio o sin dato → null", () => {
    expect(marginTrendInsight(null)).toBeNull();
    expect(marginTrendInsight({ currMonth: "2026-06", prevMonth: "2026-05", currPct: 40, prevPct: 40, pts: 0 })).toBeNull();
  });

  it("peakDayInsight: good + nombra el día", () => {
    const c = peakDayInsight({ weekday: "viernes", value: 50000, sharePct: 50 })!;
    expect(c.tone).toBe("good");
    expect(c.title.toLowerCase()).toContain("viernes");
    expect(c.detail).toContain("50");
  });

  it("peakDayInsight: sin dato → null", () => {
    expect(peakDayInsight(null)).toBeNull();
  });
});

// --------------------------------------------------------------------------
// buildInsights — las tarjetas nuevas entran sin romper el orden de urgencia.
// --------------------------------------------------------------------------
describe("buildInsights — integra reposición/tendencia/día pico", () => {
  const prices = new Map<string, number>([["m1", 2000]]);
  // 6 días de ventas → habilita día pico; el viernes es el más fuerte.
  const sales: DailySalesRow[] = [
    { date: "2026-06-15", orders: 5, revenue: "10000", cash: "10000", card: "0" },
    { date: "2026-06-16", orders: 5, revenue: "10000", cash: "10000", card: "0" },
    { date: "2026-06-17", orders: 5, revenue: "10000", cash: "10000", card: "0" },
    { date: "2026-06-18", orders: 5, revenue: "10000", cash: "10000", card: "0" },
    { date: "2026-06-19", orders: 20, revenue: "50000", cash: "50000", card: "0" },
    { date: "2026-06-20", orders: 6, revenue: "12000", cash: "12000", card: "0" },
  ];
  // un mover por agotarse → habilita reposición.
  const rotation: StockRotationRow[] = [
    { product_id: "m1", product_name: "Mover", qty_sold: 30, current_stock: 5, turnover: "6.0", days_of_inventory: "5" },
  ];
  // dos meses de margen → habilita tendencia mensual.
  const margins: DailyMarginRow[] = [
    { date: "2026-05-15", revenue: "100000", cost: "60000", margin: "40000", margin_pct: "40.0", items_without_cost: 0 },
    { date: "2026-06-15", revenue: "100000", cost: "70000", margin: "30000", margin_pct: "30.0", items_without_cost: 0 },
  ];

  it("Pro: incluye reorder, margin-trend y peak-day, manteniendo el orden", () => {
    const cards = buildInsights({
      sales, margins, marginsGated: false, nearExpiry: [], rotation, top: [], prices, today: "2026-06-20",
    });
    const ids = cards.map((c) => c.id);
    expect(ids).toContain("reorder");
    expect(ids).toContain("margin-trend");
    expect(ids).toContain("peak-day");
    // reposición antes del pulso diario; tendencia mensual tras el margen diario.
    expect(ids.indexOf("reorder")).toBeLessThan(ids.indexOf("sales-delta"));
    expect(ids.indexOf("margin-delta")).toBeLessThan(ids.indexOf("margin-trend"));
  });

  it("Free (margins gated): NO aparece la tendencia mensual (dato Pro)", () => {
    const ids = buildInsights({
      sales, margins: null, marginsGated: true, nearExpiry: [], rotation, top: [], prices, today: "2026-06-20",
    }).map((c) => c.id);
    expect(ids).not.toContain("margin-trend");
    expect(ids).toContain("reorder"); // reposición es core (Free)
    expect(ids).toContain("peak-day"); // día pico es core (Free)
  });
});
