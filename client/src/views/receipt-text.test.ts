// The shareable boleta text the cashier "Copiar" button puts on the clipboard.
// Asserts a real operator gets a legible ticket with the vuelto ALWAYS explicit
// for cash and mixed tenders (the F-paul-pay-001 split-tender change). Money is
// compared through `clp` so the test never hard-codes a locale glyph.
import { describe, it, expect } from "vitest";
import { receiptText } from "./receipt-text";
import { clp, num } from "../format";
import type { Receipt } from "../api";

function base(): Receipt {
  return {
    order_id: "order:abc",
    folio_or_number: "B-1001",
    datetime: "2026-06-16T15:30:00",
    tenant_name: "Farmacia Demo",
    items: [
      { name: "Paracetamol 500mg", qty: 2, unit_price: "990", line_total: "1980" },
      { name: "Alcohol gel 250ml", qty: 1, unit_price: "2500", line_total: "2500" },
    ],
    subtotal: "4480",
    discount: "0",
    total: "4480",
    payment_method: "pos_cash",
    cash_amount: "5000",
    card_amount: null,
    change: "520",
    loyalty_points_awarded: 0,
    cashier: "Ana",
    footer_note: "Gracias por su compra",
  };
}

describe("receiptText — shareable boleta", () => {
  it("cash sale: header, lines, total and an explicit vuelto", () => {
    const t = receiptText(base());
    expect(t).toContain("Farmacia Demo");
    expect(t).toContain("Boleta B-1001");
    expect(t).toContain("Cajero: Ana");
    expect(t).toContain(`2x Paracetamol 500mg  ${clp("1980")}`);
    expect(t).toContain(`TOTAL: ${clp("4480")}`);
    expect(t).toContain(`Recibido: ${clp("5000")}`);
    expect(t).toContain(`Vuelto: ${clp("520")}`);
    expect(t).toContain("Gracias por su compra");
  });

  it("mixed tender: both legs AND the vuelto are shown", () => {
    const r: Receipt = {
      ...base(),
      payment_method: "pos_mixed",
      cash_amount: "3000",
      card_amount: "2000",
      change: "520",
    };
    const t = receiptText(r);
    expect(t).toContain(`Efectivo: ${clp("3000")}`);
    expect(t).toContain(`Tarjeta: ${clp("2000")}`);
    expect(t).toContain(`Vuelto: ${clp("520")}`);
  });

  it("card sale: no cash/vuelto lines, no NaN", () => {
    const r: Receipt = {
      ...base(),
      payment_method: "pos_debit",
      cash_amount: null,
      change: null,
    };
    const t = receiptText(r);
    expect(t).not.toContain("Recibido:");
    expect(t).not.toContain("Vuelto:");
    expect(t).not.toContain("NaN");
  });

  it("shows discount line and loyalty points when present", () => {
    const r: Receipt = {
      ...base(),
      discount: "480",
      total: "4000",
      loyalty_points_awarded: 12,
    };
    const t = receiptText(r);
    expect(t).toContain(`Descuento: -${clp("480")}`);
    expect(t).toContain(`Puntos ganados: +${num(12)}`);
  });

  it("minimarket (no cashier/footer): still renders without empty labels", () => {
    const r: Receipt = { ...base(), cashier: null, footer_note: "" };
    const t = receiptText(r);
    expect(t).not.toContain("Cajero:");
    expect(t.endsWith("\n")).toBe(false);
  });
});
