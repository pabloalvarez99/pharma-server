// LANE B — user-journey tests for the INVENTORY VIEW presentation logic
// (inventory.ts delegates its near-expiry ordering/highlight, the min-stock
// reorder signal, and the Rotación-ABC ranking to these pure helpers — see the
// wiring in inventory.ts renderVencimientos / productRow / renderRotacion). A
// green run means what the químico/dueño SEES on screen — the order of the
// urgent lotes, the "Reponer N" hint, the A/B/C rotation badges — is correct,
// not just the underlying math. Both verticals (pharmacy + minimarket) are
// exercised where the rubro changes the data, never the logic.
import { describe, it, expect } from "vitest";
import {
  nearExpiryView,
  reorderSuggestion,
  reorderList,
  rotacionRows,
  REORDER_TARGET,
} from "./stock-helpers";

// --------------------------------------------------------------------------
// Journey 9 (LANE B) — La vista de "Próximos a vencer" ORDENA por urgencia y
//   resalta: el operador ve primero lo caducado, luego lo que vence antes.
//   El feed del server no garantiza orden → la vista lo impone (FEFO).
// --------------------------------------------------------------------------
describe("journey · vista vencimientos — orden FEFO + resaltado por urgencia", () => {
  it("caducados primero (más vencido arriba), luego el que vence antes", () => {
    // Llega desordenado, como puede venir del endpoint.
    const feed = [
      { product_id: "p1", product_name: "Amoxicilina", batch_code: "B-90", days_to_expiry: 90, expired: false, stock: 10 },
      { product_id: "p2", product_name: "Ibuprofeno", batch_code: "B-VENC", days_to_expiry: -5, expired: true, stock: 4 },
      { product_id: "p3", product_name: "Paracetamol", batch_code: "B-12", days_to_expiry: 12, expired: false, stock: 7 },
      { product_id: "p4", product_name: "Loratadina", batch_code: "B-OLD", days_to_expiry: -30, expired: true, stock: 2 },
    ];
    const view = nearExpiryView(feed);
    // El más vencido (−30) arriba, luego −5, luego 12, luego 90.
    expect(view.map((r) => r.batch_code)).toEqual(["B-OLD", "B-VENC", "B-12", "B-90"]);
    // Resaltado: caducados en rojo, por-vencer ≤30d en ámbar, lejano en ok.
    expect(view.map((r) => r.tone)).toEqual(["danger", "danger", "warn", "ok"]);
    expect(view.map((r) => r.label)).toEqual(["Caducado", "Caducado", "Por vencer", "Por vencer"]);
  });

  it("trata days_to_expiry < 0 como caducado aunque expired venga en false", () => {
    // Defensa: si el server marca expired=false pero los días son negativos, la
    // vista igual lo resalta como caducado (no lo deja pasar como 'por vencer').
    const view = nearExpiryView([
      { product_id: "p", product_name: "X", batch_code: "B", days_to_expiry: -1, expired: false, stock: 1 },
    ]);
    expect(view[0].tone).toBe("danger");
    expect(view[0].label).toBe("Caducado");
  });

  it("no muta el arreglo del server", () => {
    const feed = [
      { product_id: "a", product_name: "A", batch_code: "A", days_to_expiry: 50, expired: false, stock: 1 },
      { product_id: "b", product_name: "B", batch_code: "B", days_to_expiry: 5, expired: false, stock: 1 },
    ];
    const before = feed.map((r) => r.batch_code);
    nearExpiryView(feed);
    expect(feed.map((r) => r.batch_code)).toEqual(before);
  });
});

// --------------------------------------------------------------------------
// Journey 10 (LANE B) — Multi-rubro: el resaltado de vencimiento sirve a
//   perecibles de minimarket igual que a fármacos. Un producto sin lote
//   simplemente no aparece en el feed (no rompe la vista).
// --------------------------------------------------------------------------
describe("journey · vista vencimientos — multi-rubro (minimarket perecibles)", () => {
  it("un pan a 2 días y una leche caducada se ordenan/resaltan como un fármaco", () => {
    const feed = [
      { product_id: "pan", product_name: "Pan amasado", batch_code: "PAN-1", days_to_expiry: 2, expired: false, stock: 20 },
      { product_id: "leche", product_name: "Leche 1L", batch_code: "LEC-1", days_to_expiry: -1, expired: true, stock: 6 },
      { product_id: "fideo", product_name: "Fideos", batch_code: "FID-1", days_to_expiry: 60, expired: false, stock: 30 },
    ];
    const view = nearExpiryView(feed);
    expect(view.map((r) => r.product_name)).toEqual(["Leche 1L", "Pan amasado", "Fideos"]);
    expect(view.map((r) => r.tone)).toEqual(["danger", "warn", "ok"]);
  });

  it("feed vacío (rubro sin perecibles con lote) → vista sin filas, sin crash", () => {
    expect(nearExpiryView([])).toEqual([]);
  });
});

// --------------------------------------------------------------------------
// Journey 11 (LANE B) — Alerta de stock mínimo: el SKU bajo el umbral muestra
//   una recompra sugerida (cuánto comprar para volver al objetivo). El SKU sano
//   no genera ruido.
// --------------------------------------------------------------------------
describe("journey · alerta mínimo → recompra sugerida", () => {
  it("sugiere reponer hasta el objetivo; stock sano sugiere 0", () => {
    // Agotado → reponer el objetivo completo.
    expect(reorderSuggestion(0)).toBe(REORDER_TARGET);
    // Bajo (3 u, umbral 5) → reponer hasta el objetivo.
    expect(reorderSuggestion(3)).toBe(REORDER_TARGET - 3);
    // En o sobre el objetivo → nada que reponer (sin ruido).
    expect(reorderSuggestion(REORDER_TARGET)).toBe(0);
    expect(reorderSuggestion(50)).toBe(0);
    // Stock fraccionario/raro → techo, nunca negativo.
    expect(reorderSuggestion(2.4)).toBe(Math.ceil(REORDER_TARGET - 2.4));
    expect(reorderSuggestion(Number.NaN)).toBe(REORDER_TARGET);
  });
});

// --------------------------------------------------------------------------
// Journey 12 (LANE B) — La worklist de recompra prioriza por urgencia y NO
//   ensucia con productos sanos. Multi-rubro: mezcla fármacos y abarrotes.
// --------------------------------------------------------------------------
describe("journey · worklist de recompra — agotados primero, sanos fuera", () => {
  it("ordena agotado→más bajo y excluye los OK (ambos verticales)", () => {
    const catalogo = [
      { id: "para", name: "Paracetamol", stock: 40 }, // ok → fuera
      { id: "ibu", name: "Ibuprofeno", stock: 3 }, // bajo
      { id: "coca", name: "Coca-Cola 1.5L", stock: 0 }, // agotado (minimarket)
      { id: "pan", name: "Pan", stock: 5 }, // bajo (umbral)
      { id: "agua", name: "Agua 500ml", stock: 80 }, // ok → fuera
    ];
    const work = reorderList(catalogo);
    // Sólo los 3 que necesitan reposición, agotado arriba, luego menor stock.
    expect(work.map((w) => w.item.id)).toEqual(["coca", "ibu", "pan"]);
    expect(work.map((w) => w.level)).toEqual(["out", "low", "low"]);
    // Cada uno trae su cantidad sugerida hasta el objetivo.
    expect(work.map((w) => w.suggest)).toEqual([
      REORDER_TARGET,
      REORDER_TARGET - 3,
      REORDER_TARGET - 5,
    ]);
  });

  it("catálogo todo sano → worklist vacía (cero alertas falsas)", () => {
    expect(reorderList([{ id: "x", name: "X", stock: 99 }])).toEqual([]);
  });
});

// --------------------------------------------------------------------------
// Journey 13 (LANE B) — Vista "Rotación": ranking ABC sobre unidades vendidas.
//   Lo que más se mueve (A) arriba, con su participación en %; cola = C.
// --------------------------------------------------------------------------
describe("journey · vista rotación — ranking ABC sobre unidades vendidas", () => {
  it("ordena por vendidas desde el feed del server y arma badges A/B/C + %", () => {
    // El server entrega filas de rotación; la vista las rankea (offline-friendly).
    const feed = [
      { product_id: "p3", product_name: "Vitamina C", qty_sold: 4, current_stock: 10 },
      { product_id: "p1", product_name: "Paracetamol", qty_sold: 80, current_stock: 120 },
      { product_id: "p4", product_name: "Gasa", qty_sold: 1, current_stock: 50 },
      { product_id: "p2", product_name: "Ibuprofeno", qty_sold: 15, current_stock: 60 },
    ];
    const rows = rotacionRows(feed);
    // Ordenado por vendidas desc.
    expect(rows.map((r) => r.id)).toEqual(["p1", "p2", "p3", "p4"]);
    // Pareto: 80% A, 95% acum B, cola C.
    expect(rows.map((r) => r.class)).toEqual(["A", "B", "C", "C"]);
    // Participación del líder como % entero string para la UI.
    expect(rows[0].sharePct).toBe("80%");
    // Arrastra el stock actual y el nombre para la fila clickeable.
    expect(rows[0].current_stock).toBe(120);
    expect(rows[0].name).toBe("Paracetamol");
  });
});

// --------------------------------------------------------------------------
// Journey 14 (LANE B) — Rotación multi-rubro / sin ventas: abarrotes rankean
//   igual; un producto nunca vendido cae a C con 0% (no rompe la vista).
// --------------------------------------------------------------------------
describe("journey · vista rotación — multi-rubro y stock muerto", () => {
  it("minimarket: abarrotes rankean; el nunca-vendido es C con 0%", () => {
    const feed = [
      { product_id: "coca", product_name: "Coca-Cola 1.5L", qty_sold: 75, current_stock: 24 },
      { product_id: "fid", product_name: "Fideos", qty_sold: 25, current_stock: 40 },
      { product_id: "esp", product_name: "Espárragos lata", qty_sold: 0, current_stock: 12 },
    ];
    const rows = rotacionRows(feed);
    const muerto = rows.find((r) => r.id === "esp")!;
    expect(muerto.class).toBe("C");
    expect(muerto.sharePct).toBe("0%");
    // El best-seller del minimarket es A.
    expect(rows.find((r) => r.id === "coca")!.class).toBe("A");
  });

  it("sin ventas en el período → todo C, sin crash", () => {
    const rows = rotacionRows([
      { product_id: "a", product_name: "A", qty_sold: 0, current_stock: 5 },
      { product_id: "b", product_name: "B", qty_sold: 0, current_stock: 8 },
    ]);
    expect(rows.every((r) => r.class === "C")).toBe(true);
    expect(rows.every((r) => r.sharePct === "0%")).toBe(true);
  });
});
