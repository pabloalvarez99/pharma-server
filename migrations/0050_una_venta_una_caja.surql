-- pharma-server 0050: una venta le entra a UNA caja, no a todas
-- ---------------------------------------------------------------------------
-- `cash_sales_running_maint` (0030 → 0046 → 0049) aplica su delta con
--
--     WHERE tenant = $tenant AND status = 'open' AND opened_at <= $created
--
-- sin filtro por cajero ni por sucursal: le pega a TODAS las cajas abiertas del
-- tenant. Y el chequeo de caja única de `open_session` es POR USUARIO
-- (`WHERE tenant=$t AND user=$u AND status='open'`), así que dos cajeros con
-- caja abierta es un estado alcanzable desde la API pública, no un accidente.
--
-- Resultado: una venta de $5.000 le entra a las dos cajas. El negocio recibió
-- $5.000 y el sistema espera $10.000 repartidos en dos arqueos que nadie puede
-- cuadrar, porque cada cajón se cuenta por separado y a mano.
--
-- ## Dos cajas abiertas NO es el bug
--
-- Es una farmacia con dos mostradores, y es correcto que existan. El bug es que
-- la venta no sabe de cuál es. Por eso el arreglo scopea la venta a su caja, y
-- NO prohíbe la segunda: prohibirla arreglaría el síntoma rompiendo un caso
-- legítimo, y el feriante —que tiene una sola— no notaría la diferencia
-- mientras el negocio de dos mostradores pierde una función.
--
-- ## El dato faltaba, y ya se estaba calculando
--
-- El evento no puede scopear si la fila de `order` no dice qué caja la tomó, y
-- no lo decía: `order` era la ÚNICA tabla que toca el cajón sin nombrar su
-- sesión (`expense` tiene `cash_session` desde 0012, `purchase_payment` desde
-- 0016, `customer_ledger` desde 0039).
--
-- Pero el dato no hubo que inventarlo. `sales::service::post_sale` ya resolvía
-- la sesión de caja abierta del cajero para deducir la sucursal de la venta
-- (`cash_register::service::open_session_branch`, migración 0041) y se quedaba
-- sólo con `branch`, tirando el id de la sesión. Esta migración persiste lo que
-- ya se calculaba. **El contrato público no cambia:** `PosSaleRequest` no
-- manda la caja y sigue sin mandarla.
--
-- ## Precedencia al resolver (`cash_register::service::sale_cash_session`)
--
--   1. la caja abierta DEL VENDEDOR — exacta, y sin ambigüedad posible porque
--      `open_session` rechaza la segunda del mismo usuario;
--   2. si el vendedor no tiene y hay EXACTAMENTE UNA abierta en el tenant, esa;
--   3. ninguna.
--
-- La rama 2 no es una concesión: es la dueña que abre la caja a la mañana y el
-- empleado que vende. Hoy esa venta le entra a la caja de la dueña por accidente
-- —el evento se las aplica a todas— y está bien que le entre, porque la plata
-- terminó en ese cajón. Un filtro estricto por vendedor la dejaría sin caja y le
-- inventaría un faltante del tamaño del día.
--
-- Neto: con 0 o 1 caja abierta el comportamiento es IDÉNTICO al anterior. Sólo
-- cambia con 2 o más, que es exactamente donde estaba mal.
--
-- ## Qué NO hace esta migración
--
-- No reparte hacia atrás lo ya acumulado, y en particular **no toca ninguna
-- sesión CERRADA**. `closing_cash_expected` y `discrepancia` de una sesión
-- cerrada son el número contra el que alguien contó billetes ese día; el arqueo
-- se cuenta a mano y no hay forma de volver a contar el cajón de ayer.
-- Reescribirlo cambia lo que significa un cierre ya firmado.
--
-- Las órdenes anteriores quedan con `cash_session = NONE`. Para el evento eso
-- significa "no le entra a ningún arqueo", que para una orden histórica es la
-- verdad. La ventana afectada es angosta —sólo las órdenes de las sesiones que
-- estén abiertas en el momento de migrar— y se mide antes de decidir nada:
--
--     SELECT count() AS n FROM order
--       WHERE cash_session = NONE
--         AND payment_method IN ['pos_cash','pos_mixed']
--         AND status != 'cancelled'
--         AND created_at >= (SELECT VALUE math::min(opened_at)
--                            FROM cash_register_session WHERE status='open')[0]
--       GROUP ALL;
--
-- Si da > 0, el backfill va en migración APARTE para poder decidirlo sin
-- bloquear esto.
--
-- Salvedad conocida, angosta y dicha en voz alta: una orden ANTERIOR a esta
-- migración tiene `cash_session = NONE`, así que si después de migrar recibe un
-- cambio que la descalifica, el evento ya no tiene a quién restarle y el
-- acumulado de esa caja queda inflado por su monto. En la práctica no se llega:
-- el único camino a `cancelled` sale de `reserved`/`preparing`/
-- `ready_for_pickup` (`transition_web_order`), que son pedidos web sin
-- `pos_cash`/`pos_mixed`, y una devolución ya no descalifica desde la 0049
-- (`$bc == $ac`, delta 0, idéntico antes y después de este cambio).
--
-- Append-only: nunca editar una vez aplicada.
-- ---------------------------------------------------------------------------

-- La caja que se llevó el efectivo de esta venta. Lo escribe
-- `sales::repo::apply_sale` con lo que resolvió `sale_cash_session`, dentro del
-- mismo `BEGIN;…COMMIT;` que la orden: no hay instante en que la orden exista
-- sin saber a qué cajón fue.
--
-- `option<>` y no un default: NONE es un estado real y frecuente —vender sin
-- caja abierta es legal, y es lo que hace el feriante que nunca usa el arqueo—.
-- Un default apuntaría a una caja inventada.
DEFINE FIELD cash_session ON order TYPE option<record<cash_register_session>>;

-- El agregado del arqueo, scopeado a la caja de la venta.
--
-- El neto de vuelto es idéntico al de 0046 y la calificación idéntica a la de
-- 0049 (`status != 'cancelled'`); lo único que cambia es DÓNDE aterriza el
-- delta.
--
-- Dos UPDATE en vez de uno con el delta, a propósito: `$before` y `$after`
-- pueden apuntar a cajas distintas, y con un solo `+= $delta` sobre una de las
-- dos el agregado de la otra queda torcido para siempre. Cuando la caja no
-- cambia —el caso normal— las dos sentencias se degradan a `-= $bc` y `+= $ac`
-- sobre la misma fila, que es el mismo neto. En un CREATE `$bc` es 0 y se emite
-- una sola escritura, así que el camino caliente del POS no paga nada.
--
-- Se mantiene `status = 'open'`: si la caja ya cerró, el delta se descarta en
-- vez de reescribir un arqueo firmado. Es la misma protección que tenía la
-- ventana `opened_at <= $created` de 0030, dicha directo.
--
-- Ya no hace falta comparar fechas: la orden nombra su caja, así que la ventana
-- temporal —que era una forma indirecta y ambigua de adivinarla— se va.
DEFINE EVENT OVERWRITE cash_sales_running_maint ON TABLE order
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $bsess = IF $event = "CREATE" THEN NONE ELSE $before.cash_session END;
    LET $asess = IF $event = "DELETE" THEN NONE ELSE $after.cash_session END;

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

    IF $bc != 0dec AND $bsess != NONE THEN
      UPDATE $bsess SET cash_sales_running -= $bc WHERE status = 'open'
    END;

    IF $ac != 0dec AND $asess != NONE THEN
      UPDATE $asess SET cash_sales_running += $ac WHERE status = 'open'
    END;
  };
