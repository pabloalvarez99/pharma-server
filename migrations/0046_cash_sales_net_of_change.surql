-- pharma-server 0046: cash_sales_running suma el efectivo NETO DE VUELTO
-- (BUG de plata en el arqueo)
-- ---------------------------------------------------------------------------
-- La migración 0030 sumaba `order.cash_amount` CRUDO al agregado
-- `cash_sales_running`, que es lo que `compute_summary()` usa como `cash_sales`
-- para el efectivo esperado del cajón.
--
-- Pero `cash_amount` es lo que el cliente **ENTREGÓ**, no lo que se cobró. Lo
-- dice el propio dominio (`sales/model.rs` → "Vuelto: `cash_amount - total`") y
-- lo confirma el cliente Android, que manda ahí el campo "Pagó con".
--
-- Consecuencia, todos los días: una venta de $5.000 pagada con un billete de
-- $10.000 metía $10.000 al esperado. El cajón tiene $5.000 — el vuelto salió de
-- ahí — así que el arqueo declaraba un faltante de $5.000 que nunca existió.
-- El error se acumula por venta: es el número que decide si el feriante confía
-- en la app o vuelve al cuaderno.
--
-- Regla nueva (`domain::invariants::cash_into_drawer`, misma fórmula que ya
-- usaba `expenses::service::sales_by_method` para las mixtas):
--
--   falta   = max(total − card, 0)      -- el vuelto SIEMPRE sale del efectivo
--   entrega = cash_amount ?? falta      -- NONE = "no se registró", no "0"
--   neto    = max(min(entrega, falta), 0)
--
-- Los tres casos de borde, que son decisiones de negocio:
--   * `cash_amount` NONE ⇒ `falta` completo. Antes daba 0 y el arqueo quedaba
--     por DEBAJO. Pasa de verdad: en Android "Pagó con" es opcional
--     (`takeIf { isNotBlank() }`) y el agente de voz fía sin monto.
--   * `cash_amount` menor que `falta` (venta mal registrada) ⇒ entró sólo eso.
--   * tarjeta que cubre de más ⇒ 0, nunca negativo.
--
-- pos_mixed: `cash + card >= total` está validado en `post_sale`, así que el
-- neto de una mixta es exactamente `total − card`; el excedente del efectivo es
-- el vuelto.
--
-- Por qué una migración nueva y no editar 0030: 0030 YA CORRIÓ en
-- instalaciones vivas. `migrations/` es append-only (`_migrations` trackea por
-- nombre de archivo: reeditar un archivo aplicado no vuelve a correr nunca).
-- Se redefine el evento con `DEFINE EVENT OVERWRITE` — mismo patrón con el que
-- 0031 redefinió `product_stats_maint` — y se recalculan las sesiones que ya
-- existen, porque el agregado guardado hoy está inflado y ninguna corrección
-- del evento lo toca hacia atrás.
--
-- Se recalculan TODAS las sesiones, incluidas las cerradas: el arqueo de una
-- sesión cerrada es el papel con el que alguien buscó plata que no faltaba, y
-- `closing_cash_expected`/`discrepancia` quedaron congelados con el número
-- malo. Ver el bloque de reparación al final.
-- Append-only: nunca editar una vez aplicada.
-- ---------------------------------------------------------------------------

-- Mismo evento que 0030 (mismas ramas CREATE/UPDATE/DELETE, mismo guard de
-- ventana y mismo delta exacto `after − before`); lo único que cambia es que
-- cada lado aporta su efectivo NETO en vez de `cash_amount` crudo.
DEFINE EVENT OVERWRITE cash_sales_running_maint ON TABLE order
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $tenant  = IF $event = "DELETE" THEN $before.tenant ELSE $after.tenant END;
    LET $created = IF $event = "DELETE" THEN $before.created_at ELSE $after.created_at END;

    LET $bq = IF $event = "CREATE" THEN false
              ELSE ($before.payment_method IN ['pos_cash','pos_mixed']
                    AND $before.status NOT IN ['refunded','cancelled']) END;
    LET $b_falta_raw = ($before.total ?? 0dec) - ($before.card_amount ?? 0dec);
    LET $b_falta = IF $b_falta_raw > 0dec THEN $b_falta_raw ELSE 0dec END;
    LET $b_entrega = $before.cash_amount ?? $b_falta;
    LET $b_neto = IF $b_entrega < $b_falta THEN $b_entrega ELSE $b_falta END;
    LET $bc = IF $bq AND $b_neto > 0dec THEN $b_neto ELSE 0dec END;

    LET $aq = IF $event = "DELETE" THEN false
              ELSE ($after.payment_method IN ['pos_cash','pos_mixed']
                    AND $after.status NOT IN ['refunded','cancelled']) END;
    LET $a_falta_raw = ($after.total ?? 0dec) - ($after.card_amount ?? 0dec);
    LET $a_falta = IF $a_falta_raw > 0dec THEN $a_falta_raw ELSE 0dec END;
    LET $a_entrega = $after.cash_amount ?? $a_falta;
    LET $a_neto = IF $a_entrega < $a_falta THEN $a_entrega ELSE $a_falta END;
    LET $ac = IF $aq AND $a_neto > 0dec THEN $a_neto ELSE 0dec END;

    LET $delta = $ac - $bc;

    IF $delta != 0dec THEN
      UPDATE cash_register_session SET cash_sales_running += $delta
        WHERE tenant = $tenant AND status = 'open' AND opened_at <= $created
    END;
  };

-- Recálculo de lo ya acumulado. El backfill de 0030 dejó el número inflado en
-- cada fila; el evento nuevo sólo corrige lo que pase de ahora en adelante.
-- Misma ventana que 0030: abiertas hasta ahora, cerradas hasta su cierre.
--
-- `SELECT VALUE <expr>` + `math::sum()` sobre el array, en vez de un
-- `math::sum(<expr>)` agregado en la proyección: la suma de una expresión
-- derivada dentro de un GROUP ALL no es un camino que este esquema ya ejerza,
-- y acá corre una vez por instalación sobre las órdenes de la ventana.
FOR $s IN (SELECT id, tenant, opened_at, closed_at, status,
                  opening_cash, closing_cash_counted
           FROM cash_register_session) {
  LET $upper = $s.closed_at ?? time::now();
  LET $netos = (SELECT VALUE
      (IF ((total ?? 0dec) - (card_amount ?? 0dec)) <= 0dec THEN 0dec
       ELSE (IF (cash_amount ?? ((total ?? 0dec) - (card_amount ?? 0dec)))
                < ((total ?? 0dec) - (card_amount ?? 0dec))
             THEN (cash_amount ?? 0dec)
             ELSE ((total ?? 0dec) - (card_amount ?? 0dec)) END) END)
    FROM order
    WHERE tenant = $s.tenant
      AND payment_method IN ['pos_cash','pos_mixed']
      AND status NOT IN ['refunded','cancelled']
      AND created_at >= $s.opened_at
      AND created_at <= $upper);
  LET $sum = math::sum($netos);
  UPDATE $s.id SET cash_sales_running = <decimal> ($sum ?? 0dec);
};

-- Reparación de las sesiones YA CERRADAS: `closing_cash_expected` y
-- `discrepancia` se congelaron con el agregado inflado, así que el historial
-- muestra faltantes inventados. Se recalculan con el mismo movimiento de caja
-- que usa `compute_summary()` (apertura + ventas efectivo + ingresos − retiros)
-- y el efectivo contado, que es un hecho medido y NO se toca.
--
-- No se reescribe `closing_notes`: es lo que la persona escribió.
--
-- Salvedad conocida, heredada del backfill de 0030 y con el mismo alcance: una
-- orden devuelta DESPUÉS del cierre queda fuera de la ventana recalculada
-- aunque al cerrar sí contara, así que en ese caso el número reparado puede
-- correrse por el monto de esa devolución. Es raro (la devolución posterior al
-- cierre lo es) y es preferible a dejar el faltante inventado de TODAS las
-- ventas con vuelto, que es sistemático y diario.
FOR $s IN (SELECT id, tenant, opening_cash, closing_cash_counted,
                  cash_sales_running
           FROM cash_register_session WHERE status = 'closed') {
  LET $rin  = (SELECT VALUE math::sum(amount) FROM cash_movement
               WHERE tenant = $s.tenant AND session = $s.id
                 AND tipo = 'ingreso' GROUP ALL)[0];
  LET $rout = (SELECT VALUE math::sum(amount) FROM cash_movement
               WHERE tenant = $s.tenant AND session = $s.id
                 AND tipo = 'retiro' GROUP ALL)[0];
  LET $exp = ($s.opening_cash ?? 0dec) + ($s.cash_sales_running ?? 0dec)
             + ($rin ?? 0dec) - ($rout ?? 0dec);
  UPDATE $s.id SET
    closing_cash_expected = <decimal> $exp,
    discrepancia = <decimal> (($s.closing_cash_counted ?? 0dec) - $exp);
};
