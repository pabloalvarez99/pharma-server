-- pharma-server 0042: LOTES por sucursal — cierra el FEFO multi-local.
-- ---------------------------------------------------------------------------
-- La migración 0041 hizo el stock por sucursal un ledger de CANTIDADES: una
-- sucursal no puede vender más de lo que tiene, pero la elección de LOTE seguía
-- siendo global. Consecuencia concreta: vender en el local A podía consumir el
-- lote que está físicamente en el local B (el que vence antes), y el vencimiento
-- que el cajero de A ve en pantalla no era el frasco que tiene en la mano.
--
-- Esta migración le da domicilio al lote:
--
--   1. `product_batch.branch` — el lote FÍSICO vive en UNA sucursal.
--   2. `purchase_order.branch` — la recepción de mercadería entra a UNA
--      sucursal: los lotes creados al recibir nacen ahí y el `stock_movement`
--      de la recepción pega en ese bucket (antes caía siempre en casa matriz).
--
-- Invariante que se agrega al de 0041 (`Σ_sucursal product_branch_stock.stock
-- == product.stock`), para todo producto con lotes activos:
--
--     Σ product_batch.stock WHERE branch = X  ==  product_branch_stock[X].stock
--
-- Se sostiene porque los tres caminos que mueven lotes ahora estampan la MISMA
-- sucursal en el lote y en el `stock_movement` de la misma transacción: venta
-- (FEFO acotado a la sucursal), recepción (lote nace en la sucursal de la OC) y
-- transferencia (mueve el lote, no sólo la cantidad — ver `stock::repo::
-- apply_transfer`). La devolución repone en los lotes que consumió la venta, que
-- por construcción son de la sucursal de esa venta.
--
-- `option` + ausente ⇒ casa matriz, igual que 0041. NO hace falta backfill con
-- FOR: sobre una tabla SCHEMAFULL un campo `option` nuevo lee NONE en las filas
-- viejas, y NONE es exactamente el bucket al que 0041 asignó todo el stock
-- histórico. Un instalado hoy queda coherente sin tocar una sola fila: sus lotes
-- y su stock apuntan los dos a casa matriz.
--
-- Append-only — nunca editar una migración ya aplicada.

-- ---------------------------------------------------------------------------
-- 1. product_batch.branch — el lote pertenece a una sucursal.
--
-- El índice lidera con `tenant` (regla #4) y ordena por `expiry_date` para que
-- el plan FEFO acotado al local (`WHERE tenant AND product AND branch AND active
-- AND stock > 0 AND expiry_date >= now ORDER BY expiry_date ASC`) se resuelva
-- por índice y no por scan: es hot path de venta (<50ms p99).
-- ---------------------------------------------------------------------------
DEFINE FIELD branch ON product_batch TYPE option<record<branch>>;
DEFINE INDEX pb_tenant_product_branch_expiry
  ON product_batch FIELDS tenant, product, branch, expiry_date;

-- ---------------------------------------------------------------------------
-- 2. purchase_order.branch — a qué local entra la mercadería.
--
-- Se fija al crear la OC (el comprador ya sabe para qué local compra) y la
-- recepción la lee de ahí: no hace falta que el que recibe la re-elija, y dos
-- recepciones parciales de la misma OC no pueden contradecirse de local.
-- ---------------------------------------------------------------------------
DEFINE FIELD branch ON purchase_order TYPE option<record<branch>>;
DEFINE INDEX po_tenant_branch_created ON purchase_order FIELDS tenant, branch, created_at;
