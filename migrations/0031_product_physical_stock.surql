-- pharma-server 0031: product.physical_stock flag — modelar servicios sin inventario.
-- ---------------------------------------------------------------------------
-- Un negocio de servicios (belleza, oficios) vende ítems que NO son bienes
-- físicos: un corte de pelo no descuenta inventario ni tiene lote/vencimiento.
-- Hasta ahora el seed de servicios usaba un stock-proxy alto (9999) porque el
-- path de venta chequea `stock < qty` de forma incondicional. Esta migración
-- agrega el flag honesto: `physical_stock = false` ⇒ el ítem es un servicio →
-- la venta SALTA el chequeo de stock y el plan FEFO/decremento de lotes.
--
-- `product` ya es tenant-scoped (migración 0003) → esto es un column add, no
-- una tabla nueva (sin índice nuevo).
--
-- DEFAULT true ⇒ todo producto existente sigue siendo físico: el chequeo
-- `stock < qty`, los lotes y el invariante `product.stock == Σ batch == Σ
-- movement` quedan INTACTOS. Sólo los ítems sembrados/creados explícitamente
-- como servicio (`physical_stock = false`) cambian de comportamiento.
-- Append-only: nunca editar una vez aplicada.
-- ---------------------------------------------------------------------------

DEFINE FIELD physical_stock ON product TYPE bool DEFAULT true;

-- ---------------------------------------------------------------------------
-- Métricas de stock excluyen servicios. Un servicio se siembra con `stock = 0`
-- (no entra por lote), así que sin este ajuste contaría como `out_of_stock` y
-- `low_stock`, ensuciando las alertas del dashboard con falsos "agotados". Los
-- servicios siguen contando en `total`/`active` (son productos vendibles), pero
-- NO en las métricas de inventario. Redefinimos el EVENT de mantenimiento del
-- agregado `product_stats` (migración 0029) con el guard `physical_stock = true`
-- en `low_stock`/`out_of_stock`. `inventory_value` ya es 0 para un servicio
-- (stock 0 × costo), así que no cambia.
-- ---------------------------------------------------------------------------

DEFINE EVENT OVERWRITE product_stats_maint ON TABLE product
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $tenant = IF $event = "DELETE" THEN $before.tenant ELSE $after.tenant END;

    LET $bt = IF $event = "CREATE" THEN 0 ELSE 1 END;
    LET $ba = IF $event = "CREATE" THEN 0 ELSE (IF $before.active THEN 1 ELSE 0 END) END;
    LET $bl = IF $event = "CREATE" THEN 0
              ELSE (IF $before.active AND $before.physical_stock AND $before.stock <= 5 THEN 1 ELSE 0 END) END;
    LET $bo = IF $event = "CREATE" THEN 0
              ELSE (IF $before.active AND $before.physical_stock AND $before.stock <= 0 THEN 1 ELSE 0 END) END;
    LET $bi = IF $event = "CREATE" THEN 0dec
              ELSE ($before.stock * ($before.cost_price ?? 0dec)) END;

    LET $at = IF $event = "DELETE" THEN 0 ELSE 1 END;
    LET $aa = IF $event = "DELETE" THEN 0 ELSE (IF $after.active THEN 1 ELSE 0 END) END;
    LET $al = IF $event = "DELETE" THEN 0
              ELSE (IF $after.active AND $after.physical_stock AND $after.stock <= 5 THEN 1 ELSE 0 END) END;
    LET $ao = IF $event = "DELETE" THEN 0
              ELSE (IF $after.active AND $after.physical_stock AND $after.stock <= 0 THEN 1 ELSE 0 END) END;
    LET $ai = IF $event = "DELETE" THEN 0dec
              ELSE ($after.stock * ($after.cost_price ?? 0dec)) END;

    UPSERT type::thing('product_stats', meta::id($tenant)) SET
      tenant           = $tenant,
      total           += ($at - $bt),
      active          += ($aa - $ba),
      low_stock       += ($al - $bl),
      out_of_stock    += ($ao - $bo),
      inventory_value += ($ai - $bi);
  };

-- Re-backfill: recomputa el agregado con el guard de servicios para que una
-- instalación con catálogo previo quede correcta de inmediato (los eventos sólo
-- disparan en escrituras futuras). UPSERT pisa la fila existente. No-op en
-- install fresco (`product` vacío).
FOR $r IN (
  SELECT
    tenant,
    count() AS total,
    count(active = true) AS active,
    count(active = true AND physical_stock = true AND stock <= 5) AS low_stock,
    count(active = true AND physical_stock = true AND stock <= 0) AS out_of_stock,
    math::sum(stock * (cost_price ?? 0dec)) AS inventory_value
  FROM product GROUP BY tenant
) {
  UPSERT type::thing('product_stats', meta::id($r.tenant)) SET
    tenant          = $r.tenant,
    total           = $r.total,
    active          = $r.active,
    low_stock       = $r.low_stock,
    out_of_stock    = $r.out_of_stock,
    inventory_value = ($r.inventory_value ?? 0dec);
};
