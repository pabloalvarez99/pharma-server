-- pharma-server 0041: stock OPERATIVO por sucursal — business-depth V2.
-- ---------------------------------------------------------------------------
-- La migración 0032 dejó `branch`/`register` como records de CONFIGURACIÓN: el
-- stock seguía siendo global (`product.stock`, un único contador por producto),
-- así que "tengo 2 locales" no cambiaba nada operativo. Esta migración hace el
-- stock por sucursal REAL:
--
--   1. `stock_movement.branch`  — cada movimiento sabe a qué local pega.
--   2. `product_branch_stock`   — on-hand por (tenant, producto, sucursal),
--                                 mantenido incrementalmente desde el ledger.
--   3. `order.branch`           — la venta queda atribuida a su sucursal
--                                 (reportes de venta por local).
--   4. `cash_register_session.register` / `.branch` — la caja se abre EN una
--                                 sucursal, y la venta hereda esa sucursal
--                                 cuando el POS no la manda explícita.
--
-- Por qué es exacto (y por qué no hace falta un contador nuevo que mantener a
-- mano): el invariante raíz del inventario (migración 0004) es
--
--     product.stock = SUM(stock_movement.delta)   per (tenant, product)
--
-- y el código NUNCA escribe `product.stock` fuera de un `stock_movement` en la
-- misma transacción (venta, devolución, lotes, compras, ajustes y seed pasan
-- todos por ahí). Si cada movimiento lleva su `branch` (NONE = casa matriz /
-- sitio único), entonces SUM(delta) agrupado por sucursal es exactamente el
-- on-hand de esa sucursal, y por construcción:
--
--     Σ_sucursal product_branch_stock.stock == product.stock      (invariante V2)
--
-- La transferencia entre locales emite DOS movimientos de suma cero
-- (`transfer_out` -qty en el origen, `transfer_in` +qty en el destino) dentro de
-- una sola transacción: reparte entre buckets sin tocar el total del tenant.
--
-- Mantenimiento vía `DEFINE EVENT` sobre `stock_movement` CREATE — las filas del
-- ledger son inmutables (append-only), así que el evento es CREATE puro y no
-- necesita las ramas $before/$after de `product_stats_maint` (0029). Por qué
-- EVENT y no un view `DEFINE TABLE … AS SELECT`: el view mis-mantiene UPDATE
-- (misma razón documentada en 0029).
--
-- Servicios (`physical_stock = false`) no emiten movimiento → nunca entran a
-- `product_branch_stock`. Un corte de pelo no tiene inventario que repartir.
--
-- FUERA DE ALCANCE (lane siguiente, explícita — ver bitácora): los LOTES
-- FÍSICOS por sucursal (`product_batch.branch`) y por lo tanto el FEFO acotado
-- al local. Hoy el stock por sucursal es un ledger de CANTIDADES: la sucursal no
-- puede vender más de lo que tiene, pero la elección de lote (vencimiento) sigue
-- siendo global. Append-only — nunca editar una migración ya aplicada.

-- ---------------------------------------------------------------------------
-- 1. stock_movement.branch — sucursal a la que pega el movimiento.
--
-- `option` + ausente ⇒ todo movimiento legacy y todo flujo que aún no es
-- branch-aware (recepción de compras, ajustes de lote) cae en el bucket NONE
-- = casa matriz. Venta, devolución y transferencia sí la setean.
-- ---------------------------------------------------------------------------
DEFINE FIELD branch ON stock_movement TYPE option<record<branch>>;
DEFINE INDEX sm_tenant_branch_created ON stock_movement FIELDS tenant, branch, created_at;

-- ---------------------------------------------------------------------------
-- 2. product_branch_stock — on-hand por (tenant, producto, sucursal).
--
-- Multi-tenant (regla #4): `tenant` + índices compuestos que lideran con tenant.
-- La identidad de fila es un record-id determinístico
--   product_branch_stock:[tenant_key, product_key, branch_key]
-- (branch_key = 'none' para casa matriz). Así el UPSERT del evento es
-- idempotente y la unicidad por (tenant, producto, sucursal) es implícita en el
-- id — sin índice UNIQUE, que dejaría ambiguo el caso NONE.
-- ---------------------------------------------------------------------------
DEFINE TABLE product_branch_stock SCHEMAFULL;
DEFINE FIELD tenant     ON product_branch_stock TYPE record<tenant>;
DEFINE FIELD product    ON product_branch_stock TYPE record<product>;
DEFINE FIELD branch     ON product_branch_stock TYPE option<record<branch>>;
DEFINE FIELD stock      ON product_branch_stock TYPE int DEFAULT 0;
DEFINE FIELD updated_at ON product_branch_stock TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX pbs_tenant_product ON product_branch_stock FIELDS tenant, product;
DEFINE INDEX pbs_tenant_branch  ON product_branch_stock FIELDS tenant, branch;

-- Mantenimiento incremental: el movimiento aplica su `delta` a la fila de su
-- (tenant, producto, sucursal). `stock` arranca en DEFAULT 0 cuando el UPSERT
-- crea la fila, así que `+= delta` materializa el saldo correcto desde la nada
-- (mismo mecanismo que product_stats_maint para una clave nueva).
DEFINE EVENT product_branch_stock_maint ON TABLE stock_movement
  WHEN $event = "CREATE" THEN {
    LET $bkey = IF $after.branch = NONE THEN "none" ELSE meta::id($after.branch) END;
    LET $row  = type::thing("product_branch_stock",
                  [meta::id($after.tenant), meta::id($after.product), $bkey]);
    UPSERT $row SET
      tenant   = $after.tenant,
      product  = $after.product,
      branch   = $after.branch,
      stock   += $after.delta;
  };

-- Stock INICIAL de un producto nuevo. `catalog::create_product` escribe
-- `product.stock` directo en el CREATE (es el único punto donde stock nace sin
-- pasar por `stock_movement`: no hay nada que auditar todavía). Sin este evento,
-- un producto creado con stock > 0 después de esta migración tendría
-- `product.stock = N` y `Σ product_branch_stock = 0`, y la casa matriz no podría
-- venderlo ni transferirlo. El evento lo siembra en el bucket casa matriz, que es
-- la misma atribución que usa el backfill de más abajo.
--
-- Sólo CREATE: un evento sobre UPDATE contaría DOBLE, porque venta / devolución /
-- recepción ya actualizan `product.stock` **y** emiten su `stock_movement`.
DEFINE EVENT product_branch_stock_seed ON TABLE product
  WHEN $event = "CREATE" AND $after.physical_stock = true AND $after.stock != 0 THEN {
    LET $row = type::thing("product_branch_stock",
                 [meta::id($after.tenant), meta::id($after.id), "none"]);
    UPSERT $row SET
      tenant   = $after.tenant,
      product  = $after.id,
      branch   = NONE,
      stock   += $after.stock;
  };

-- ---------------------------------------------------------------------------
-- 3. order.branch — sucursal donde se hizo la venta (reportes por local).
-- ---------------------------------------------------------------------------
DEFINE FIELD branch ON order TYPE option<record<branch>>;
DEFINE INDEX order_tenant_branch_created ON order FIELDS tenant, branch, created_at;

-- ---------------------------------------------------------------------------
-- 4. cash_register_session.register / .branch — la caja se abre EN un local.
--
-- `register_name` (0011) sigue siendo el texto libre de siempre para no romper
-- sesiones existentes; `register` es el link opcional al record de caja (0032) y
-- `branch` la sucursal resuelta al abrir. La venta que no manda `branch` toma
-- ESTA — de ahí sale "la venta descuenta de la sucursal de la caja".
-- ---------------------------------------------------------------------------
DEFINE FIELD register ON cash_register_session TYPE option<record<register>>;
DEFINE FIELD branch   ON cash_register_session TYPE option<record<branch>>;
DEFINE INDEX crs_tenant_branch_status ON cash_register_session FIELDS tenant, branch, status;

-- ---------------------------------------------------------------------------
-- Backfill del upgrade path: una instalación con stock previo queda correcta de
-- inmediato (el evento sólo dispara en movimientos futuros). Todo el stock
-- histórico se asigna al bucket NONE (casa matriz) — la única atribución honesta,
-- porque el stock pre-V2 no sabe de sucursales; el operador lo reparte a sus
-- locales con transferencias. Sólo bienes físicos; los servicios quedan fuera.
-- No-op en install fresco (`product` vacío).
-- ---------------------------------------------------------------------------
FOR $p IN (SELECT id, tenant, stock FROM product WHERE physical_stock = true) {
  LET $row = type::thing("product_branch_stock",
               [meta::id($p.tenant), meta::id($p.id), "none"]);
  UPSERT $row SET
    tenant  = $p.tenant,
    product = $p.id,
    branch  = NONE,
    stock   = $p.stock;
};
