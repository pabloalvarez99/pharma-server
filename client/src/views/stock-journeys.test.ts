// User-journey tests for the stock / money-out views (inventory · compras ·
// gastos). Each test drives a full operator flow through the REAL view helpers
// (the views delegate their inline validation/aggregation/status logic to
// stock-helpers — see the wiring in compras.ts / inventory.ts / gastos.ts), so
// a green run means the journey the químico/dueño walks every day still holds.
//
// Money is a Decimal STRING on the wire (CLP). WAC is the server contract,
// mirrored faithfully in weightedAverageCost (regression guard). Both verticals
// (pharmacy + minimarket) are exercised where the rubro changes behaviour.
import { describe, it, expect } from "vitest";
import {
  // compras
  parsePoLines,
  poKpis,
  poPending,
  poIsReceivable,
  poStatusMeta,
  validateReceiveQty,
  weightedAverageCost,
  type PoStatus,
  // inventario
  validateStockAdjust,
  stockLevel,
  expiryStatus,
  pharmaFieldsVisible,
  // gastos
  parseExpense,
  expenseTotal,
  cashEgresos,
  // lane B
  fefoOrder,
  abcClassify,
} from "./stock-helpers";

// --------------------------------------------------------------------------
// Journey 1 — Compras: crear OC multilínea → recibir parcial → recibir total.
//   Costo promedio ponderado actualiza, stock sube. (charter LANE A #1)
// --------------------------------------------------------------------------
describe("journey · compra → recepción parcial → total → WAC + stock", () => {
  it("acumula stock y recalcula el costo promedio paso a paso", () => {
    // El dueño arma una OC de 2 líneas del mismo producto a costos distintos.
    const parsed = parsePoLines([
      { name: "Paracetamol 500mg", qty: "100", cost: "90" },
      { name: "Paracetamol 500mg", qty: "50", cost: "120" },
      { name: "  ", qty: "1", cost: "" }, // fila en blanco → ignorada
    ]);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.value).toHaveLength(2);

    // Producto sin stock ni costo previo. Primera recepción = parcial (100u @90).
    // Base rule: sin costo previo el WAC se SIEMBRA con el promedio de línea.
    let wac = weightedAverageCost(0, null, [{ qty: 100, unitCost: 90 }]);
    expect(wac.stock).toBe(100);
    expect(wac.cost).toBe(90);

    // Pendiente tras la recepción parcial: 50u de la 2ª línea (100/100 + 0/50).
    const pendItems = [
      { quantity: 100, qty_received: 100 },
      { quantity: 50, qty_received: 0 },
    ];
    expect(poPending(pendItems)).toBe(50);
    expect(poIsReceivable("partially_received")).toBe(true);

    // Segunda recepción (50u @120) sobre stock 100 @90 → WAC ponderado.
    wac = weightedAverageCost(wac.stock, wac.cost, [{ qty: 50, unitCost: 120 }]);
    expect(wac.stock).toBe(150);
    // (100·90 + 50·120) / 150 = (9000 + 6000)/150 = 100
    expect(wac.cost).toBe(100);

    // Todo recibido → ya no quedan pendientes.
    expect(poPending([
      { quantity: 100, qty_received: 100 },
      { quantity: 50, qty_received: 50 },
    ])).toBe(0);
  });

  it("agrega varias líneas del mismo producto ANTES del promedio (como el server)", () => {
    // Dos líneas del mismo SKU en una sola recepción: el server las bucketea y
    // promedia juntas. (100·90 + 50·120)/150 = 100 — idéntico a recibirlas por
    // separado partiendo de cero.
    const wac = weightedAverageCost(0, null, [
      { qty: 100, unitCost: 90 },
      { qty: 50, unitCost: 120 },
    ]);
    expect(wac).toEqual({ stock: 150, cost: 100 });
  });

  it("no deja recibir más de lo pendiente ni cantidades no enteras", () => {
    expect(validateReceiveQty(5, 10)).toBeNull();
    expect(validateReceiveQty(0, 10)).toBeNull(); // 0 = no recibir esta línea
    expect(validateReceiveQty(11, 10)).toMatch(/más de lo pendiente/);
    expect(validateReceiveQty(2.5, 10)).toMatch(/enteros/);
    expect(validateReceiveQty(-1, 10)).toMatch(/enteros/);
  });
});

// --------------------------------------------------------------------------
// Journey 2 — WAC base rule: primer ingreso siembra el costo, stock≤0 reinicia.
// --------------------------------------------------------------------------
describe("journey · WAC base rule (primer ingreso / stock agotado)", () => {
  it("primer ingreso (sin costo previo) NO se diluye con un cero fantasma", () => {
    // Si tratáramos old_cost como 0, el WAC sería (0·0 + 100·90)/100 = 90 igual,
    // pero con old_cost=null y old_stock=0 el server usa el promedio de línea
    // directo — mismo número, distinto camino. Verificamos el contrato.
    expect(weightedAverageCost(0, null, [{ qty: 100, unitCost: 90 }]).cost).toBe(90);
  });

  it("producto con stock 0 pero costo histórico → reinicia al costo de línea", () => {
    // old_stock ≤ 0 ignora el costo viejo (no hay unidades que ponderar).
    const wac = weightedAverageCost(0, 999, [{ qty: 10, unitCost: 50 }]);
    expect(wac.cost).toBe(50);
  });

  it("recepción vacía no cambia nada", () => {
    expect(weightedAverageCost(40, 75, [])).toEqual({ stock: 40, cost: 75 });
  });
});

// --------------------------------------------------------------------------
// Journey 3 — Inventario: alta SKU → lote/vencimiento → alerta mínimo → ajuste
//   de stock auditado. (charter LANE A #2)
// --------------------------------------------------------------------------
describe("journey · alta producto → lote → alerta stock bajo → ajuste auditado", () => {
  it("recorre el ciclo de stock con sus pills y validaciones", () => {
    // Producto nuevo arranca con stock alto → OK.
    expect(stockLevel(40)).toBe("ok");

    // Tras varias ventas baja al umbral → alerta "bajo".
    expect(stockLevel(5)).toBe("low");
    expect(stockLevel(0)).toBe("out");

    // Lote con vencimiento a 20 días → "por vencer" (warn).
    const now = new Date("2026-06-14T12:00:00Z");
    const lote = expiryStatus("2026-07-01T12:00:00Z", now);
    expect(lote.tone).toBe("warn");
    expect(lote.expired).toBe(false);

    // Ajuste auditado: el operador hace un recuento y FIJA el stock a 30.
    expect(validateStockAdjust("set", 30)).toBeNull();
    // Un ajuste delta de 0 no tiene sentido (no genera movimiento auditable).
    expect(validateStockAdjust("delta", 0)).toMatch(/no puede ser cero/);
    // Fijar a negativo es imposible.
    expect(validateStockAdjust("set", -5)).toMatch(/no puede ser negativo/);
    // Merma de 3 unidades (delta) sí es válida y deja rastro.
    expect(validateStockAdjust("delta", -3)).toBeNull();
    expect(validateStockAdjust("delta", 2.5)).toMatch(/entera/);
  });
});

// --------------------------------------------------------------------------
// Journey 4 — Gastos: registrar gasto → refleja en total y en la caja (egreso
//   en efectivo toca el cajón; transferencia no). (charter LANE A #3)
// --------------------------------------------------------------------------
describe("journey · gasto → total del período → impacto en caja", () => {
  it("valida, normaliza y suma; sólo el efectivo golpea la caja", () => {
    // El operador registra el arriendo (transferencia) y un flete (efectivo).
    const arriendo = parseExpense("450000.50", "Arriendo local junio");
    expect(arriendo.ok).toBe(true);
    if (!arriendo.ok) return;
    // CLP entero: 450000.50 → "450000" (trunc), descripción trim.
    expect(arriendo.value.amount).toBe("450000");

    const flete = parseExpense("12000", "Flete proveedor");
    expect(flete.ok).toBe(true);

    // Monto inválido / sin descripción son rechazados con copy en español.
    expect(parseExpense("0", "x")).toEqual({ ok: false, error: expect.stringMatching(/mayor a 0/) });
    expect(parseExpense("abc", "x")).toEqual({ ok: false, error: expect.stringMatching(/monto válido/) });
    expect(parseExpense("100", "   ")).toEqual({ ok: false, error: expect.stringMatching(/descripción/) });

    const rows = [
      { amount: "450000", payment_method: "transfer" },
      { amount: "12000", payment_method: "cash" },
      { amount: "8000", payment_method: "cash" },
    ];
    // KPI "Total" suma todo lo del período.
    expect(expenseTotal(rows)).toBe(470000);
    // En la caja sólo aparecen los egresos en efectivo (12000 + 8000).
    expect(cashEgresos(rows)).toBe(20000);
  });
});

// --------------------------------------------------------------------------
// Journey 5 — Multi-rubro: minimarket NO usa principio activo/laboratorio/receta
//   pero SÍ usa lote/vencimiento para perecibles (pan, leche). (charter LANE A #4)
// --------------------------------------------------------------------------
describe("journey · multi-rubro (farmacia vs minimarket)", () => {
  it("oculta campos clínicos fuera de farmacia, conserva lote/vencimiento", () => {
    // Farmacia (o sin setting / desconocido) → campos clínicos visibles.
    expect(pharmaFieldsVisible("farmacia")).toBe(true);
    expect(pharmaFieldsVisible(null)).toBe(true);
    expect(pharmaFieldsVisible(undefined)).toBe(true);

    // Minimarket / almacén → ocultos.
    expect(pharmaFieldsVisible("minimarket")).toBe(false);
    expect(pharmaFieldsVisible("almacén")).toBe(false);

    // PERO el vencimiento sirve a perecibles igual: un pan a 2 días vence pronto.
    const now = new Date("2026-06-14T12:00:00Z");
    const pan = expiryStatus("2026-06-16T12:00:00Z", now);
    expect(pan.tone).toBe("warn");
    // Y la leche caducada se marca igual que un fármaco vencido.
    const lecheVencida = expiryStatus("2026-06-10T12:00:00Z", now);
    expect(lecheVencida.expired).toBe(true);
    expect(lecheVencida.tone).toBe("danger");
  });

  it("el flujo de compra/recepción es agnóstico al rubro", () => {
    // Una OC de abarrotes (minimarket) valida y promedia igual que fármacos.
    const oc = parsePoLines([{ name: "Coca-Cola 1.5L", qty: "24", cost: "950" }]);
    expect(oc.ok).toBe(true);
    const wac = weightedAverageCost(0, null, [{ qty: 24, unitCost: 950 }]);
    expect(wac).toEqual({ stock: 24, cost: 950 });
  });
});

// --------------------------------------------------------------------------
// Journey 6 — Estados de OC en español, sin fuga de inglés. Regresión del bug
//   `partial` (estado fantasma) → KPI sub-contaba y el filtro venía vacío.
// --------------------------------------------------------------------------
describe("journey · estados de OC traducidos + KPIs correctos (BUG-marvin-001)", () => {
  const ALL: PoStatus[] = [
    "draft",
    "sent",
    "approved",
    "partially_received",
    "received",
    "cancelled",
  ];

  it("cada estado real del server tiene etiqueta es-CL (no fuga inglés)", () => {
    for (const s of ALL) {
      const meta = poStatusMeta(s);
      // No debe contener guion bajo ni quedar en inglés crudo.
      expect(meta.label).not.toMatch(/_/);
      expect(meta.label).not.toBe(s);
    }
    expect(poStatusMeta("partially_received").label).toBe("Parcial");
    expect(poStatusMeta("approved").label).toBe("Aprobada");
  });

  it("Pendientes cuenta draft·sent·approved·partially_received (no received/cancelled)", () => {
    const rows = [
      { status: "draft", total: "1000" },
      { status: "sent", total: "2000" },
      { status: "approved", total: "3000" },
      { status: "partially_received", total: "4000" },
      { status: "received", total: "5000" },
      { status: "cancelled", total: "6000" },
    ];
    const k = poKpis(rows);
    expect(k.total).toBe(6);
    // 4 abiertas (las 2 últimas no). El bug viejo contaba 2 (sólo draft+sent).
    expect(k.pending).toBe(4);
    expect(k.totalValue).toBe(21000);
  });

  it("partially_received es recibible; received/cancelled no", () => {
    expect(poIsReceivable("partially_received")).toBe(true);
    expect(poIsReceivable("approved")).toBe(true);
    expect(poIsReceivable("received")).toBe(false);
    expect(poIsReceivable("cancelled")).toBe(false);
    expect(poIsReceivable("draft")).toBe(false); // hay que emitirla primero
  });
});

// --------------------------------------------------------------------------
// Journey 7 (LANE B) — Vencimientos próximos en orden FEFO.
// --------------------------------------------------------------------------
describe("journey · FEFO — el lote que vence primero se consume/alerta primero", () => {
  it("ordena por vencimiento ascendente, fechas inválidas al final", () => {
    const lotes = [
      { batch_code: "C", expiry_date: "2026-09-01T12:00:00Z", stock: 10 },
      { batch_code: "A", expiry_date: "2026-06-20T12:00:00Z", stock: 5 },
      { batch_code: "B", expiry_date: "2026-07-15T12:00:00Z", stock: 8 },
      { batch_code: "X", expiry_date: "sin-fecha", stock: 3 },
    ];
    expect(fefoOrder(lotes).map((l) => l.batch_code)).toEqual(["A", "B", "C", "X"]);
  });

  it("no muta el arreglo original", () => {
    const input = [
      { expiry_date: "2026-08-01T12:00:00Z", stock: 1 },
      { expiry_date: "2026-06-01T12:00:00Z", stock: 1 },
    ];
    const before = input.map((b) => b.expiry_date);
    fefoOrder(input);
    expect(input.map((b) => b.expiry_date)).toEqual(before);
  });
});

// --------------------------------------------------------------------------
// Journey 8 (LANE B) — Rotación ABC (Pareto) sobre la valorización vendida.
// --------------------------------------------------------------------------
describe("journey · ABC rotación — Pareto sobre la rotación", () => {
  it("clasifica A/B/C por participación acumulada", () => {
    // Total = 100. P1=80 (80% → A), P2=15 (95% acum → B), P3=4 + P4=1 (cola → C).
    const ranked = abcClassify([
      { id: "p3", value: 4 },
      { id: "p1", value: 80 },
      { id: "p4", value: 1 },
      { id: "p2", value: 15 },
    ]);
    // Ordenado por valor desc.
    expect(ranked.map((r) => r.id)).toEqual(["p1", "p2", "p3", "p4"]);
    expect(ranked.map((r) => r.class)).toEqual(["A", "B", "C", "C"]);
    // Participación del líder.
    expect(ranked[0].share).toBeCloseTo(0.8, 5);
  });

  it("total cero → todo C (sin señal de rotación todavía)", () => {
    const ranked = abcClassify([
      { id: "a", value: 0 },
      { id: "b", value: 0 },
    ]);
    expect(ranked.every((r) => r.class === "C")).toBe(true);
  });
});
