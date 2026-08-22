import { describe, expect, it } from "vitest";
import type { Dte } from "../api";
import { dteRowHtml } from "./dte-row";

const hostile = {
  id: 'dte:<img src=x onerror="boom">',
  tipo: 33,
  folio: 7,
  fecha_emision: 'bad"><script>alert(1)</script>',
  rut_emisor: "76123456-0",
  rut_receptor: '66666666-6',
  razon_social_receptor: "Cliente <script>alert('x')</script>",
  monto_total: "1190",
  estado: '<img src=x onerror=boom>',
  track_id: '<svg onload=boom>',
  sii_glosa: 'rechazo <b>no confiable</b>',
  order_id: null,
  has_xml: true,
} as unknown as Dte;

describe("dteRowHtml · cumplimiento", () => {
  it("escapa campos del servidor, incluidos fecha, estado desconocido y track SII", () => {
    const html = dteRowHtml(hostile, { prefix: "bol", sendPlan: "Pro" });
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("&lt;svg onload=boom&gt;");
    expect(html).toContain("&lt;img src=x onerror=boom&gt;");
  });

  it("mantiene IDs de acciones y el copy de plan por vista", () => {
    const html = dteRowHtml({ ...hostile, estado: "signed" }, { prefix: "fac", sendPlan: "Business" });
    expect(html).toMatch(/id="fac-xml-dte-/);
    expect(html).toContain("requiere plan Business");
  });
});
