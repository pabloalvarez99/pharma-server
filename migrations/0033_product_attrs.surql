-- pharma-server 0033: product.attrs — per-rubro flexible attributes.
--
-- The multi-rubro pivot ("RutBusiness = ERP global"): the fixed product schema
-- is pharmacy-shaped (laboratory / therapeutic_action / active_ingredient /
-- presentation). New rubros must NOT accrete columns (talla, SKU variante,
-- duración del servicio, ingredientes de cocina…). `attrs` is the escape hatch:
-- a free-form object whose keys the rubro pack (GET /api/v1/rubro-pack)
-- declares; the core stays agnóstico.
--
-- Back-fill: the existing clinical fields are COPIED into `attrs` so new code
-- can read one place. The original columns stay (append-only rule + zero blast
-- radius on the POS hot path); dropping them is a later, deliberate migration.
-- NONE-valued fields are omitted from the object by SurrealQL.

DEFINE FIELD attrs ON product TYPE option<object>;

UPDATE product SET attrs = {
    laboratory: laboratory,
    therapeutic_action: therapeutic_action,
    active_ingredient: active_ingredient,
    presentation: presentation,
    prescription_type: prescription_type
};
