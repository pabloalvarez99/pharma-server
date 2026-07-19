-- pharma-server 0034: product.parent_id — variantes multi-SKU (Opción A).
--
-- Retail (rubro tienda): un producto padre (ej. "Polera básica") tiene N
-- variantes vendibles (talla M/L, color, barcode y stock propios). Cada
-- variante ES un `product` hijo; no hay tabla product_variant.
--
-- Por qué no tabla aparte (ver docs/product/variants-design.md):
--   * POS ya resuelve product_barcode → product (scan = id de variante).
--   * stock / FEFO / stock_movement / compras operan sobre product.
--   * un solo DTO en el wire; listados excluyen hijos por defecto.
--
-- Append-only: no se tocan filas existentes (parent_id = NONE implícito).

DEFINE FIELD parent_id ON product TYPE option<record<product>>;
DEFINE INDEX product_tenant_parent ON product FIELDS tenant, parent_id;
