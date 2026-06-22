// V1 business-depth — fiado / cuenta corriente (cliente side). Pure-helper
// contracts for the customer store-credit surface: the saldo + movimientos
// render, the printable estado de cuenta, the abono validation, the POS fiar
// gate, and the graceful "backend not ready yet" detector. No DOM here — every
// helper is a pure string/value producer (the view wiring is exercised in
// cuenta-fiado.dom.test.ts). Money is integer CLP throughout.
import { describe, it, expect } from "vitest";
import { cuentaUnavailable, CUENTA_MODULE_MISSING, type CuentaCorriente } from "../api";
import { fiarEnabled } from "./pos";
import { renderCuentaCard, estadoCuentaPrintHtml, validateAbono } from "./clientes";
import { clp } from "../format";

const cuenta = (over: Partial<CuentaCorriente> = {}): CuentaCorriente => ({
  saldo: 12345,
  limite: 50000,
  movimientos: [
    { id: "mov:1", fecha: "2026-06-20T14:30:00Z", tipo: "cargo", monto: 18000, glosa: "Venta boleta 41", ref: "order:41" },
    { id: "mov:2", fecha: "2026-06-21T10:00:00Z", tipo: "abono", monto: 5655, glosa: "Abono efectivo", ref: null },
  ],
  ...over,
});

describe("cuentaUnavailable — graceful 'backend not ready' detector", () => {
  it("is true for the module-missing sentinel", () => {
    expect(cuentaUnavailable(CUENTA_MODULE_MISSING)).toBe(true);
  });
  it("is true for a Tauri 'command not found' (client shipped ahead of server)", () => {
    expect(cuentaUnavailable("Command cuenta_corriente not found")).toBe(true);
  });
  it("is true for a 404 / not-implemented shape", () => {
    expect(cuentaUnavailable("404 Not Found")).toBe(true);
    expect(cuentaUnavailable("feature no implementada")).toBe(true);
  });
  it("is FALSE for a real Spanish error — those must surface, never hide", () => {
    expect(cuentaUnavailable("No tienes permiso para registrar abonos.")).toBe(false);
    expect(cuentaUnavailable(new Error("Saldo insuficiente"))).toBe(false);
    expect(cuentaUnavailable("boom")).toBe(false);
  });
});

describe("fiarEnabled — POS fiar gate", () => {
  it("requires a selected customer", () => {
    expect(fiarEnabled(false)).toBe(false);
    expect(fiarEnabled(true)).toBe(true);
  });
});

describe("validateAbono — abono amount parsing", () => {
  it("accepts a positive amount, plain or formatted", () => {
    expect(validateAbono("10000")).toEqual({ ok: true, monto: 10000 });
    expect(validateAbono("$10.000")).toEqual({ ok: true, monto: 10000 });
  });
  it("rejects empty / zero / non-numeric with an error message", () => {
    expect(validateAbono("").ok).toBe(false);
    expect(validateAbono("0").ok).toBe(false);
    expect(validateAbono("abc").ok).toBe(false);
    expect(validateAbono("").error).toMatch(/monto/i);
  });
});

describe("renderCuentaCard — saldo + movimientos render", () => {
  it("shows the saldo as CLP and flags a debt (saldo > 0)", () => {
    const html = renderCuentaCard(cuenta(), "Juan Pérez");
    expect(html).toContain(clp(12345));
    expect(html).toContain("pill-danger"); // debe
    expect(html.toLowerCase()).toContain("debe");
  });
  it("flags 'al día' when the saldo is zero or negative", () => {
    const html = renderCuentaCard(cuenta({ saldo: 0, movimientos: [] }), "Ana");
    expect(html).toContain("pill-ok");
    expect(html.toLowerCase()).toContain("al día");
  });
  it("shows the cupo and the available credit when a limite is set", () => {
    const html = renderCuentaCard(cuenta(), "Juan");
    expect(html).toContain(clp(50000)); // cupo
    expect(html).toContain(clp(50000 - 12345)); // disponible
  });
  it("renders a row per movimiento, signed by tipo (cargo +, abono −)", () => {
    const html = renderCuentaCard(cuenta(), "Juan");
    expect(html).toContain("Venta boleta 41");
    expect(html).toContain("Abono efectivo");
    expect(html).toContain("cuenta-mov-cargo");
    expect(html).toContain("cuenta-mov-abono");
    expect(html).toContain(`+ ${clp(18000)}`);
    expect(html).toContain(`− ${clp(5655)}`);
  });
  it("shows an empty state when there are no movimientos", () => {
    const html = renderCuentaCard(cuenta({ movimientos: [] }), "Juan");
    expect(html.toLowerCase()).toContain("sin movimientos");
  });
  it("escapes a hostile glosa (no raw markup injection)", () => {
    const html = renderCuentaCard(
      cuenta({ movimientos: [{ id: "m", fecha: "2026-06-20T00:00:00Z", tipo: "cargo", monto: 1000, glosa: "<script>x</script>", ref: null }] }),
      "Juan",
    );
    expect(html).not.toContain("<script>x</script>");
    expect(html).toContain("&lt;script&gt;");
  });
  it("exposes the abono + print action buttons by id", () => {
    const html = renderCuentaCard(cuenta(), "Juan");
    expect(html).toContain("cli-abono-btn");
    expect(html).toContain("cli-cuenta-print");
  });
});

describe("estadoCuentaPrintHtml — printable statement", () => {
  it("is a self-contained #cuenta-print block naming the customer and saldo", () => {
    const html = estadoCuentaPrintHtml("Juan Pérez", cuenta());
    expect(html).toContain('id="cuenta-print"');
    expect(html.toLowerCase()).toContain("estado de cuenta");
    expect(html).toContain("Juan Pérez");
    expect(html).toContain(clp(12345));
  });
  it("lists every movimiento", () => {
    const html = estadoCuentaPrintHtml("Juan", cuenta());
    expect(html).toContain("Venta boleta 41");
    expect(html).toContain("Abono efectivo");
  });
  it("escapes the customer name", () => {
    const html = estadoCuentaPrintHtml("<b>x</b>", cuenta({ movimientos: [] }));
    expect(html).not.toContain("<b>x</b>");
    expect(html).toContain("&lt;b&gt;x&lt;/b&gt;");
  });
});
