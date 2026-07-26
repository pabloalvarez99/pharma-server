-- pharma-server 0040: datos de la FACTURA del proveedor sobre la orden de compra
-- (V3 — libro de compras + resumen IVA/F29).
--
-- El ERP ya tenía libro de VENTAS (DTE) pero no de COMPRAS: el dueño no podía
-- sacar su IVA crédito. Al recepcionar mercadería el operador tiene la factura
-- del proveedor en la mano; acá se capturan sus campos para que el libro de
-- compras sea real (folio + fecha + neto + IVA) en vez de estimado.
--
-- Todos opcionales y append-only: las OC existentes siguen válidas sin factura,
-- y el libro deriva neto/IVA del total (19% CL) cuando no está declarada.

DEFINE FIELD invoice_folio ON purchase_order TYPE option<string>;
-- Fecha del documento (puede diferir de la fecha de la OC/recepción; el libro
-- de compras se arma por fecha de FACTURA, que es lo que exige el SII).
DEFINE FIELD invoice_date  ON purchase_order TYPE option<datetime>;
-- Tipo DTE del documento del proveedor: 33 factura afecta, 34 exenta,
-- 61 nota de crédito, 56 nota de débito.
DEFINE FIELD invoice_tipo  ON purchase_order TYPE option<int>;
-- Montos declarados en la factura. Si vienen, mandan sobre el cálculo derivado.
DEFINE FIELD invoice_neto  ON purchase_order TYPE option<decimal>;
DEFINE FIELD invoice_iva   ON purchase_order TYPE option<decimal>;
DEFINE FIELD invoice_total ON purchase_order TYPE option<decimal>;

-- Libro de compras del período: barrido por fecha de factura dentro del tenant.
DEFINE INDEX po_tenant_invoice_date ON purchase_order FIELDS tenant, invoice_date;
