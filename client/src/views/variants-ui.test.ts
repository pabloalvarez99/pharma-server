import { describe, it, expect } from "vitest";
import {
  variantsParentBanner,
  parentWithVariantsError,
  isParentHasVariantsMessage,
  variantAttrsLabel,
  preferBarcodeLookup,
} from "./variants-ui";

describe("variantsParentBanner", () => {
  it("Spanish copy with count for multi-SKU parent", () => {
    expect(variantsParentBanner(2)).toMatch(/2 variantes/i);
    expect(variantsParentBanner(2)).toMatch(/código de barras/i);
    expect(variantsParentBanner(2)).toMatch(/padre/i);
    expect(variantsParentBanner(1)).toMatch(/1 variante/i);
  });
  it("empty when no children", () => {
    expect(variantsParentBanner(0)).toBe("");
  });
});

describe("parentWithVariantsError", () => {
  it("names the product and asks for barcode scan", () => {
    const m = parentWithVariantsError("Polera básica");
    expect(m).toContain("Polera básica");
    expect(m.toLowerCase()).toMatch(/variantes/);
    expect(m.toLowerCase()).toMatch(/escanea|código/);
  });
});

describe("isParentHasVariantsMessage", () => {
  it("matches domain sales Spanish reject", () => {
    expect(
      isParentHasVariantsMessage(
        "el producto 'Polera' tiene variantes; venda por talla/SKU o escanee el código de barras de la variante",
      ),
    ).toBe(true);
    expect(isParentHasVariantsMessage("stock insuficiente")).toBe(false);
  });
});

describe("variantAttrsLabel", () => {
  it("prefers talla/color/sku order", () => {
    expect(variantAttrsLabel({ talla: "M", color: "Negro", sku: "X1" })).toBe("M · Negro · X1");
  });
  it("empty bag → empty string", () => {
    expect(variantAttrsLabel(null)).toBe("");
    expect(variantAttrsLabel({})).toBe("");
  });
});

describe("preferBarcodeLookup", () => {
  it("EAN-13 style codes prefer barcode path", () => {
    expect(preferBarcodeLookup("7804999700013")).toBe(true);
    expect(preferBarcodeLookup("7804999700020")).toBe(true);
  });
  it("short names do not", () => {
    expect(preferBarcodeLookup("para")).toBe(false);
    expect(preferBarcodeLookup("ab")).toBe(false);
  });
  it("internal SKU without spaces can prefer barcode", () => {
    expect(preferBarcodeLookup("POL-M-001")).toBe(true);
  });
});
