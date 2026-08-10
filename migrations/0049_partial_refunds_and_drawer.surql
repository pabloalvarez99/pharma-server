-- pharma-server 0049: la devolución deja de mentirle al cajón
-- ---------------------------------------------------------------------------
-- Tres defectos con el mismo origen: `order.status = 'refunded'` se usa como si
-- fuera un booleano "esta venta ya no vale", y el arqueo cuelga de él.
--
-- 1. `apply_refund` (sales/repo.rs) marca `status='refunded'` ante CUALQUIER
--    devolución, parcial incluida. Devolver $5.000 de una venta de $15.000
--    marca la venta entera como devuelta.
-- 2. El evento `cash_sales_running_maint` (0030, redefinido en 0046) excluye
--    `status IN ['refunded','cancelled']`, así que ese mismo instante RESTA del
--    esperado del cajón los $15.000 completos. El cajón tiene $10.000. El
--    arqueo declara un sobrante de $10.000 que no existe.
-- 3. Los reportes que filtran `status NOT IN ['refunded','cancelled']`
--    (`sales_daily`, `sales_by_method`, `margins_daily`, `top_products`,
--    `stock_rotation`) hacen desaparecer la venta ENTERA del día. Se vendió
--    $15.000 y se devolvió $5.000: el día facturó $10.000, no $0.
--
-- La corrección de fondo NO es tocar el cajón: es que `refunded` deje de ser un
-- booleano disfrazado. Se agrega `order.refunded_total` — cuánta plata de esta
-- venta se devolvió — y `status='refunded'` pasa a significar lo que dice:
-- devuelta ENTERA. Con eso, los cinco filtros de arriba siguen leyéndose bien
-- (una venta totalmente devuelta sí desaparece del día) y la parcial deja de
-- evaporarse.
--
-- ## El cajón pasa a ser libro de caja puro
--
-- Regla nueva: **una venta aporta al esperado del cajón el efectivo que entró
-- cuando se cobró, y nada más.** La plata que se devuelve sale por un
-- `cash_movement(tipo='retiro')` con fecha propia, igual que un gasto.
--
-- Por qué, y es el argumento que hay que poder defender: el arqueo se cuenta a
-- mano. `closing_cash_expected` existe únicamente para compararse contra
-- billetes contados en un momento dado. Por lo tanto el esperado de un día sólo
-- puede ser función de movimientos que YA ocurrieron. Si un evento de mañana
-- puede cambiar el esperado de ayer, el número deja de ser verificable: no hay
-- forma de volver a contar el cajón de ayer. Ya tenemos la cicatriz
-- (`closing_cash_expected` de sesiones cerradas, reparado en 0046).
--
-- Sale gratis, además, la devolución con tarjeta: no toca el cajón porque la
-- plata vuelve por el procesador. El movimiento lleva su propio método; no hay
-- caso especial.
--
-- Y cierra la salvedad que 0030 heredó a 0046 ("una orden devuelta DESPUÉS del
-- cierre queda fuera de la ventana recalculada"): ya no hay tal cosa. El retiro
-- cae en la caja abierta el día que la plata sale, que es donde sale de verdad.
--
-- ## Qué NO hace esta migración
--
-- No corrige hacia atrás. Las ventas ya marcadas `refunded` por una devolución
-- parcial quedan como están: `refunded_total` arranca en 0 y el esperado de las
-- sesiones viejas no se recalcula. Es deliberado — reescribir un arqueo cerrado
-- es peor que un número malo que nadie va a volver a contar. Si la medición en
-- producción encuentra filas afectadas, el backfill va en migración APARTE para
-- que se pueda decidir sin bloquear esto.
--
-- Append-only: nunca editar una vez aplicada.
-- ---------------------------------------------------------------------------

-- Cuánta plata de esta venta ya se devolvió, acumulado sobre N devoluciones
-- parciales. `status='refunded'` ⇔ `refunded_total >= total` (lo mantiene
-- `sales::repo::apply_refund`, que es el único escritor).
--
-- OJO con el nombre: la 0046 es `cash_sales_net_of_change`, neto de VUELTO.
-- Esto es otra cosa — devoluciones. Son dos "netos" distintos y meterlos en el
-- mismo campo es el próximo bug. No se tocan.
--
-- Filas existentes: bajo SCHEMAFULL leen el DEFAULT, igual que el `status` de
-- 0038. Sin backfill.
DEFINE FIELD refunded_total ON order TYPE decimal DEFAULT 0dec;

-- El evento del arqueo deja de mirar `refunded`.
--
-- Idéntico al de 0046 (mismas ramas CREATE/UPDATE/DELETE, mismo neto de vuelto
-- vía la fórmula de `domain::invariants::cash_into_drawer`, mismo delta exacto
-- `after − before`, misma ventana). Lo único que cambia es la calificación:
--
--   antes:  status NOT IN ['refunded','cancelled']
--   ahora:  status != 'cancelled'
--
-- `cancelled` sigue afuera y no es una omisión: al `cancelled` sólo se llega
-- desde `reserved`/`preparing`/`ready_for_pickup` (`transition_web_order`), que
-- son estados de pedido web ANTES de pagar. Una orden cancelada nunca movió
-- efectivo, así que no hay retiro que emitir ni aporte que sacar.
--
-- El evento queda simétrico: si una orden vieja marcada `refunded` recibe
-- cualquier UPDATE después de esta migración, `$before` y `$after` califican
-- los dos bajo la regla nueva y el delta da 0. Re-disparar es inofensivo.
DEFINE EVENT OVERWRITE cash_sales_running_maint ON TABLE order
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $tenant  = IF $event = "DELETE" THEN $before.tenant ELSE $after.tenant END;
    LET $created = IF $event = "DELETE" THEN $before.created_at ELSE $after.created_at END;

    LET $bq = IF $event = "CREATE" THEN false
              ELSE ($before.payment_method IN ['pos_cash','pos_mixed']
                    AND $before.status != 'cancelled') END;
    LET $b_falta_raw = ($before.total ?? 0dec) - ($before.card_amount ?? 0dec);
    LET $b_falta = IF $b_falta_raw > 0dec THEN $b_falta_raw ELSE 0dec END;
    LET $b_entrega = $before.cash_amount ?? $b_falta;
    LET $b_neto = IF $b_entrega < $b_falta THEN $b_entrega ELSE $b_falta END;
    LET $bc = IF $bq AND $b_neto > 0dec THEN $b_neto ELSE 0dec END;

    LET $aq = IF $event = "DELETE" THEN false
              ELSE ($after.payment_method IN ['pos_cash','pos_mixed']
                    AND $after.status != 'cancelled') END;
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

-- Devolver mercadería de una venta FIADA tiene que bajar la deuda: hoy no hay
-- ningún camino que escriba en `customer_ledger` al devolver, así que el cliente
-- sigue debiendo lo que ya entregó de vuelta.
--
-- Tercer `kind`, no un `abono`: el cliente NO pagó — devolvió. Mezclarlos
-- rompería cualquier lectura de "cuánto cobré" sobre el ledger.
--
-- `credit::repo::account()` no necesita cambio: computa
-- `balance = Σ cargo − Σ (todo lo demás)`, así que 'devolucion' ya baja el
-- saldo con la misma aritmética. `amount > 0` se mantiene — el signo lo da
-- `kind`, invariante de 0039.
DEFINE FIELD OVERWRITE kind ON customer_ledger TYPE string
    ASSERT $value IN ["cargo", "abono", "devolucion"];
