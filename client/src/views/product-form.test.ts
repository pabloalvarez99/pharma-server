// Pure product-form model tests — pack attrs, money-as-string, clinical/service
// gates. No DOM (same pattern as stock-helpers / cashier-loop).
import { describe, it, expect } from "vitest";
import {
  productFormLabels,
  visibleAttrFields,
  parseMoneyString,
  buildProductInput,
  promoteTopLevel,
  stripTopLevel,
  isClinicalAttrKey,
  productAttrsWireBody,
  type ProductFormOptions,
} from "./product-form";
import type { PackAttrField } from "../api/rubro";
import { localAttrsForRubro } from "../vertical";

const TIENDA_ATTRS: PackAttrField[] = [
  { key: "talla", label: "Talla", kind: "text" },
  { key: "color", label: "Color", kind: "text" },
  { key: "sku", label: "SKU", kind: "text" },
];

const FARMA_ATTRS: PackAttrField[] = [
  { key: "active_ingredient", label: "Principio activo", kind: "text" },
  { key: "laboratory", label: "Laboratorio", kind: "text" },
];

function opts(partial: Partial<ProductFormOptions> = {}): ProductFormOptions {
  return {
    vocab: { item: "Producto", catalog: "Inventario" },
    physicalStock: true,
    clinical: false,
    attrFields: TIENDA_ATTRS,
    ...partial,
  };
}

describe("productFormLabels", () => {
  it("uses pack item word for physical rubros", () => {
    const l = productFormLabels({
      vocab: { item: "Producto", catalog: "Inventario" },
      physicalStock: true,
    });
    expect(l.title).toBe("Nuevo producto");
    expect(l.submitLabel).toBe("Crear producto");
    expect(l.stockHint).toBeNull();
  });

  it("service rubro: no stock field, servicio copy", () => {
    const l = productFormLabels({
      vocab: { item: "Servicio", catalog: "Inventario" },
      physicalStock: false,
    });
    expect(l.title).toMatch(/servicio/i);
    expect(l.stockHint).toMatch(/servicios/i);
    expect(l.stockLabel).toMatch(/no aplica/i);
  });
});

describe("visibleAttrFields", () => {
  it("keeps retail attrs always", () => {
    expect(visibleAttrFields(TIENDA_ATTRS, false).map((a) => a.key)).toEqual([
      "talla",
      "color",
      "sku",
    ]);
  });

  it("drops clinical keys when !clinical", () => {
    const mixed = [...TIENDA_ATTRS, ...FARMA_ATTRS];
    expect(visibleAttrFields(mixed, false).every((a) => !isClinicalAttrKey(a.key))).toBe(true);
  });

  it("keeps clinical keys when clinical", () => {
    expect(visibleAttrFields(FARMA_ATTRS, true)).toHaveLength(2);
  });
});

describe("parseMoneyString", () => {
  it("accepts plain pesos and CL grouping as STRING wire values", () => {
    expect(parseMoneyString("1990", { required: true, fieldLabel: "Precio" })).toEqual({
      ok: true,
      value: "1990",
    });
    expect(parseMoneyString("1.990", { required: true, fieldLabel: "Precio" })).toEqual({
      ok: true,
      value: "1990",
    });
    expect(parseMoneyString("$10.000", { required: true, fieldLabel: "Precio" })).toEqual({
      ok: true,
      value: "10000",
    });
  });

  it("Spanish errors for blank required / garbage", () => {
    const blank = parseMoneyString("", { required: true, fieldLabel: "Precio de venta" });
    expect(blank.ok).toBe(false);
    if (!blank.ok) expect(blank.error).toMatch(/precio/i);

    const bad = parseMoneyString("abc", { required: true, fieldLabel: "Precio" });
    expect(bad.ok).toBe(false);
  });

  it("optional blank → undefined", () => {
    expect(parseMoneyString("", { required: false, fieldLabel: "Costo" })).toEqual({
      ok: true,
      value: undefined,
    });
  });
});

describe("buildProductInput", () => {
  it("maps tienda attrs into the flexible bag (not top-level)", () => {
    const r = buildProductInput(
      {
        name: "Polera básica",
        price: "12.990",
        stock: "5",
        attrs: { talla: "M", color: "Negro", sku: "POL-001" },
      },
      opts(),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.value.price).toBe("12990"); // STRING
    expect(r.value.stock).toBe(5);
    expect(r.value.attrs).toEqual({ talla: "M", color: "Negro", sku: "POL-001" });
    expect(r.value.laboratory).toBeUndefined();
  });

  it("promotes laboratory / active_ingredient to top-level for farmacia", () => {
    const r = buildProductInput(
      {
        name: "Paracetamol 500mg",
        price: "1990",
        attrs: { laboratory: "Lab Chile", active_ingredient: "Paracetamol" },
      },
      opts({ clinical: true, attrFields: FARMA_ATTRS }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.value.laboratory).toBe("Lab Chile");
    expect(r.value.activeIngredient).toBe("Paracetamol");
    // Promoted keys leave the flexible bag empty.
    expect(r.value.attrs).toBeUndefined();
  });

  it("service rubro omits stock and clinical fields", () => {
    const r = buildProductInput(
      {
        name: "Corte de cabello",
        price: "12000",
        stock: "99", // ignored
        laboratory: "should-drop",
        attrs: { duracion_min: "45" },
      },
      opts({
        physicalStock: false,
        clinical: false,
        vocab: { item: "Servicio", catalog: "Inventario" },
        attrFields: [{ key: "duracion_min", label: "Duración (min)", kind: "number" }],
      }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.value.stock).toBeUndefined();
    expect(r.value.laboratory).toBeUndefined();
    expect(r.value.attrs).toEqual({ duracion_min: "45" });
  });

  it("rejects empty name with Spanish copy using pack vocab", () => {
    const r = buildProductInput(
      { name: "  ", price: "1000" },
      opts({ vocab: { item: "Servicio", catalog: "Inventario" } }),
    );
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.error).toMatch(/servicio/i);
  });

  it("money stays string — never a float on the wire shape", () => {
    const r = buildProductInput({ name: "X", price: "1500", costPrice: "900" }, opts());
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(typeof r.value.price).toBe("string");
    expect(typeof r.value.costPrice).toBe("string");
  });
});

describe("promoteTopLevel / stripTopLevel", () => {
  it("splits bag correctly", () => {
    const bag = { laboratory: "L", talla: "M", color: "Rojo" };
    expect(promoteTopLevel(bag).laboratory).toBe("L");
    expect(stripTopLevel(bag)).toEqual({ talla: "M", color: "Rojo" });
  });
});

describe("localAttrsForRubro (offline pack mirror)", () => {
  it("tienda has talla/color/sku", () => {
    expect(localAttrsForRubro("tienda").map((a) => a.key)).toEqual(["talla", "color", "sku"]);
  });
  it("belleza has duration", () => {
    expect(localAttrsForRubro("belleza").some((a) => a.key === "duracion_min")).toBe(true);
  });
  it("unknown → empty", () => {
    expect(localAttrsForRubro("otro")).toEqual([]);
  });
});

describe("productAttrsWireBody — serde contract for B NewProduct.attrs", () => {
  it("locks the HTTP key to `attrs` (not attributes / product_attrs)", () => {
    const body = productAttrsWireBody({ talla: "M", color: "Negro", sku: "POL-001" });
    expect(body).toEqual({
      attrs: { talla: "M", color: "Negro", sku: "POL-001" },
    });
    // JSON round-trip shape domain will deserialize as Option<serde_json::Value>
    expect(JSON.parse(JSON.stringify(body))).toEqual({
      attrs: { talla: "M", color: "Negro", sku: "POL-001" },
    });
  });

  it("omits empty bag (never null / {})", () => {
    expect(productAttrsWireBody({})).toBeUndefined();
    expect(productAttrsWireBody({ talla: "  " })).toBeUndefined();
    expect(productAttrsWireBody(null)).toBeUndefined();
  });

  it("create payload for tienda SKU matches buildProductInput → wire fragment", () => {
    const r = buildProductInput(
      {
        name: "Polera",
        price: "9990",
        attrs: { talla: "L", color: "Azul", sku: "P-L-AZ" },
      },
      opts(),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(productAttrsWireBody(r.value.attrs)).toEqual({
      attrs: { talla: "L", color: "Azul", sku: "P-L-AZ" },
    });
  });
});
