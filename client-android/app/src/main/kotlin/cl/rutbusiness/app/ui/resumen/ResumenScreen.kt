package cl.rutbusiness.app.ui.resumen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.agente.ASI_SE_ANOTA_UNA_VENTA
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA
import cl.rutbusiness.app.ui.agente.irAlAgente
import cl.rutbusiness.app.ui.gente.compartirConGente
import cl.rutbusiness.app.ui.gente.mensajeHoy
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * @param onIrALaCaja abre el ritual de la caja. Sale del bloque "En la caja",
 *   que es donde la dueña ya está mirando cuando se lo pregunta.
 * @param onIrAlFiado abre "quién me debe", desde el bloque que muestra el total.
 * @param recargasPedidas cambia cada vez que se vuelve de una de esas dos
 *   pantallas. El `ViewModel` sobrevive al cambio y su carga inicial ya corrió,
 *   así que sin esto el resumen mostraría la caja y la deuda de hace un rato.
 */
@Composable
fun ResumenRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onIrALaCaja: () -> Unit = {},
    onIrAlFiado: () -> Unit = {},
    recargasPedidas: Int = 0,
) {
    val offline = LocalOffline.current
    val vm: ResumenViewModel = viewModel(
        key = "resumen:${estado.baseUrl}",
        factory = viewModelFactory {
            initializer { ResumenViewModel(sesion = sesion, offline = offline) }
        },
    )

    LaunchedEffect(recargasPedidas) {
        // Cero es el primer montaje, y ahí ya cargó el `init` del ViewModel:
        // volver a pedirlo sería un viaje por datos móviles de más en el
        // arranque, que es justo el momento que el piso de hardware cuida.
        if (recargasPedidas > 0) vm.cargar()
    }

    ResumenScreen(vm = vm, onIrALaCaja = onIrALaCaja, onIrAlFiado = onIrAlFiado)
}

/**
 * "¿Cuánto vendí hoy?".
 *
 * Es la pregunta que la dueña se hace todos los días, y esta pantalla la
 * contesta antes que nada: la cifra del día es lo primero, lo más grande y lo
 * único que se lee de un vistazo desde lejos. Lo demás baja en el orden en que
 * a ella le importa — cómo va contra ayer, cuánta plata hay en el cajón, quién
 * le debe, qué se está por acabar.
 *
 * Lo que **no** es: un tablero. No hay gráficos, ni márgenes, ni porcentajes, ni
 * comparaciones mes contra mes. Cada uno de esos existe en el server y ninguno
 * responde la pregunta del título.
 *
 * Regla de plata: acá no se calcula ni un peso. Cada monto viene del server como
 * texto decimal y se escribe con la [Moneda] del tenant, que también viene del
 * server. Si el negocio factura en soles, esta pantalla muestra soles sin que
 * haya que tocarla.
 */
@Composable
private fun ResumenScreen(
    vm: ResumenViewModel,
    onIrALaCaja: () -> Unit,
    onIrAlFiado: () -> Unit,
) {
    val dimens = RbTheme.dimens

    val feria = esFeria()

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = if (feria) "Hoy" else "Tu día",
            // Cuando lo que se muestra es la copia guardada, el subtítulo lo
            // dice ahí mismo: es la primera línea que se lee y no se puede
            // saltar. "Cómo va el negocio hoy" arriba de cifras de anoche sería
            // la mentira más cara de esta pantalla.
            subtitle = vm.guardadoEn?.let { guardadoEn ->
                "Datos guardados en el teléfono, " +
                    Fechado(Unit, guardadoEn).antiguedad(vm.relojDePared)
            } ?: vm.nombreDelPuesto?.takeIf { feria && it.isNotBlank() }
                ?: "Cómo va el negocio hoy",
            actions = {
                RbButton(
                    label = "Actualizar",
                    onClick = vm::cargar,
                    variant = RbButtonVariant.Secondary,
                    enabled = !vm.cargando,
                )
            },
        )

        val fatal = vm.error
        when {
            fatal != null -> RbErrorState(
                title = fatal.title,
                message = fatal.message,
                modifier = Modifier.padding(dimens.space3),
                retryLabel = fatal.retryLabel,
                onRetry = if (fatal.retryLabel != null) vm::cargar else null,
            )

            vm.cargando && vm.ventasHoy == null -> Column {
                RbLoadingState(label = "Sacando la cuenta del día...")
                RbSkeletonLines(lines = 4, modifier = Modifier.padding(dimens.space3))
            }

            // `weight(1f)` y no `fillMaxSize()`: adentro de la columna que
            // empieza con la barra de arriba, `fillMaxSize` pide el alto entero
            // de la pantalla y la lista termina dibujada encima del título.
            else -> LazyColumn(
                modifier = Modifier.weight(1f),
                contentPadding = PaddingValues(dimens.space3),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                // Feria (ADR-0022): Hoy + deudas primero. Caja/FEFO son
                // retail formal y empujan el fiado fuera del pliegue.
                item("ventas") { VendidoHoy(vm) }
                item("fiado") { TeDeben(vm, onIrAlFiado) }
                if (!feria) {
                    item("caja") { EnCaja(vm, onIrALaCaja) }
                    item("faltantes") { SePorAcabar(vm) }
                    item("vencimientos") { PorVencer(vm) }
                }
            }
        }
    }
}

// --- 1 y 2: cuánto se vendió hoy, y cómo va contra ayer ---------------------

@Composable
private fun VendidoHoy(vm: ResumenViewModel) {
    val ventas = vm.ventasHoy ?: return
    TarjetaDelDia(
        moneda = vm.moneda,
        vendidoHoy = ventas.revenue,
        boletas = ventas.orders,
        comparacion = vm.comparacion,
        vendidoAyer = vm.ventasDeAyer,
    )
}

/**
 * La respuesta a la pregunta del título, sin dependencias.
 *
 * Recibe datos y no el `ViewModel` para que se pueda medir al 200% sin server
 * detrás: es la tarjeta con la cifra grande, o sea el único lugar de la
 * pantalla donde la escala de letra puede romper algo. La prueba
 * `ResumenEscalaTest` la monta con montos de siete dígitos, que es el peor caso
 * real de un negocio chico.
 *
 * En feria y antes de la primera venta del día, la tarjeta se cambia entera por
 * [HoySinVentas]: ver "$0 · 0 boletas · menos que ayer" es enterarse tres veces
 * de lo mismo y no saber qué hacer con eso.
 *
 * @param vendidoHoy monto en el texto decimal exacto que mandó el server.
 * @param vendidoAyer ídem para ayer; `null` si no se pudo traer.
 */
@Composable
internal fun TarjetaDelDia(
    moneda: Moneda,
    vendidoHoy: String,
    boletas: Long,
    comparacion: Comparacion,
    vendidoAyer: String?,
) {
    // `boletas` y no el monto: cuántas ventas hubo lo cuenta el server, y una
    // venta regalada -monto cero, boleta uno- es un día que ya empezó.
    if (esFeria() && boletas == 0L) {
        HoySinVentas(onHablarleAlAgente = irAlAgente())
        return
    }

    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard {
        Text(
            text = "Vendiste hoy",
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )

        RbAmount(
            amount = moneda.formatear(vendidoHoy),
            emphasis = RbAmountEmphasis.Headline,
        )

        Text(
            text = when (boletas) {
                0L -> "Todavía no hay ninguna venta."
                1L -> "1 boleta."
                else -> "$boletas boletas."
            },
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
            RbChip(
                label = when (comparacion) {
                    Comparacion.Mejor -> "Mejor que ayer"
                    Comparacion.Igual -> "Igual que ayer"
                    Comparacion.Peor -> "Menos que ayer"
                    Comparacion.SinDatoDeAyer -> "Sin comparación"
                },
                // El tono acompaña, no informa: la palabra ya lo dice completo.
                // Quien no distingue los colores lee exactamente lo mismo.
                tone = when (comparacion) {
                    Comparacion.Mejor -> RbChipTone.Brand
                    Comparacion.Peor -> RbChipTone.Warn
                    else -> RbChipTone.Neutral
                },
            )

            Text(
                text = if (vendidoAyer == null) {
                    "No pudimos traer lo de ayer para comparar. Toca «Actualizar»."
                } else {
                    // Se dice "día completo" porque es lo que se está
                    // comparando: hoy va a medias y ayer ya terminó. Sin esa
                    // palabra, a las diez de la mañana la pantalla parecería
                    // decir que el negocio se cayó.
                    "Ayer, día completo: ${moneda.formatear(vendidoAyer)}."
                },
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        // Contar cómo va el día — al socio, a la casa, al hijo que atiende el
        // otro puesto. Sólo con al menos una venta: el vacío de "Hoy" enseña a
        // anotar la primera, y ofrecer ahí "compartí que no vendiste nada" es
        // una broma pesada.
        //
        // Va después de la comparación porque es lo último que se hace con esta
        // tarjeta, y como secundario porque la tarjeta se abre para leerla, no
        // para mandarla.
        val compartir = compartirConGente()
        if (compartir != null && boletas > 0L) {
            RbButton(
                label = if (esFeria()) "Contar cómo va el día" else "Compartir el resumen",
                onClick = { compartir(mensajeHoy(resumenDelDia(moneda, vendidoHoy, boletas))) },
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/**
 * Lo mismo que dicen las dos primeras líneas de la tarjeta, en una sola.
 *
 * Se arma con la misma [Moneda] y el mismo `boletas` que ya se dibujaron, y no
 * con una consulta nueva: el mensaje que sale por el chat tiene que decir el
 * número que la dueña estaba mirando cuando tocó el botón.
 */
private fun resumenDelDia(moneda: Moneda, vendidoHoy: String, boletas: Long): String {
    val monto = moneda.formatear(vendidoHoy)
    return if (boletas == 1L) "$monto en 1 boleta" else "$monto en $boletas boletas"
}

/**
 * "Hoy", antes de la primera venta del día.
 *
 * Un cuaderno en blanco tampoco dice nada, y sin embargo la dueña sabe qué hacer
 * con él: anotar. Esto es eso — la frase exacta que hay que decirle al agente, y
 * el botón que lleva a decírsela. Lo que sigue debajo ("acá vas a ver cuánto
 * llevas") es la parte que el cuaderno no puede: la suma sale sola.
 *
 * Sin [onHablarleAlAgente] el botón no se dibuja. La frase sigue enseñando el
 * paso, y en feria el agente está a un toque en la barra de abajo; un botón que
 * no lleva a ninguna parte sería peor que no tenerlo.
 */
@Composable
private fun HoySinVentas(onHablarleAlAgente: (() -> Unit)?) {
    RbEmptyState(
        title = "Todavía no vendiste nada hoy",
        hint = "Cuéntale al agente lo que vayas vendiendo — «$ASI_SE_ANOTA_UNA_VENTA» — y acá " +
            "vas a ver cuánto llevas hecho, sin sumar a mano.",
        actionLabel = onHablarleAlAgente?.let { "Hablarle al agente" },
        onAction = onHablarleAlAgente,
    )
}

// --- 3: cuánta plata hay en caja -------------------------------------------

@Composable
private fun EnCaja(vm: ResumenViewModel, onIrALaCaja: () -> Unit) {
    val colors = RbTheme.colors
    val enCaja = vm.enCaja

    RbCard(title = "En la caja") {
        when {
            enCaja != null -> {
                RbAmount(amount = vm.moneda.formatear(enCaja))
                Text(
                    text = buildString {
                        append("Es lo que debería haber ahora")
                        vm.nombreDeCaja?.let { append(" en «$it»") }
                        append(". Lo calcula el computador del negocio con la apertura, ")
                        append("las ventas en efectivo y los movimientos.")
                    },
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }

            vm.sinCajaAbierta -> Text(
                text = "No hay ninguna caja abierta. Abrir la caja es lo primero del día: " +
                    "desde ahí se empieza a contar la plata que entra.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            // "Arqueo" es la palabra del contador, no la de la dueña. Y el
            // mensaje tenía que decir qué hacer, no sólo qué faltó.
            else -> Text(
                text = "No pudimos traer la cuenta de la caja. El resto sí está al día: toca " +
                    "«Actualizar» arriba para volver a pedirla.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
        }

        // Abrir la caja es lo primero que hace la dueña en el día, así que
        // cuando no hay ninguna abierta el botón es el principal de la tarjeta y
        // no una acción secundaria escondida.
        RbButton(
            label = if (vm.sinCajaAbierta) "Abrir la caja" else "Ver la caja",
            onClick = onIrALaCaja,
            variant = if (vm.sinCajaAbierta) RbButtonVariant.Primary else RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}

// --- 4: cuánto le deben -----------------------------------------------------

@Composable
private fun TeDeben(vm: ResumenViewModel, onIrAlFiado: () -> Unit) {
    val colors = RbTheme.colors
    val deuda = vm.porCobrar
    val hayDeuda = deuda != null && deuda.deudores > 0 && !esCero(deuda.total)

    RbCard(title = "Te deben") {
        when {
            deuda == null -> Text(
                text = "No pudimos traer el fiado. El resto sí está al día: toca " +
                    "«Actualizar» arriba para volver a pedirlo.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            // En feria fiar no es un botón de la pantalla de cobro: es una
            // frase. El vacío la enseña acá, que es donde la dueña se está
            // preguntando por la deuda, y no en una pantalla más adentro.
            !hayDeuda -> Text(
                text = if (esFeria()) {
                    "Nadie te debe. Cuando fíes, díselo al agente: «$ASI_SE_FIA»."
                } else {
                    "Nadie te debe plata. Cuando fíes una venta, la deuda aparece acá hasta " +
                        "que te la paguen."
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            else -> {
                RbAmount(amount = vm.moneda.formatear(deuda.total))
                Text(
                    text = if (deuda.deudores == 1) {
                        "1 cliente con cuenta pendiente."
                    } else {
                        "${deuda.deudores} clientes con cuenta pendiente."
                    },
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        }

        // El botón está incluso cuando nadie debe: es la única puerta al fiado, y
        // esconderla dejaría sin forma de llegar a la cuenta de alguien que ya
        // terminó de pagar.
        RbButton(
            label = if (hayDeuda) "Ver quién me debe" else "Ver el fiado",
            onClick = onIrAlFiado,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}

// --- 5: qué se está por acabar y qué se está por vencer ---------------------

@Composable
private fun SePorAcabar(vm: ResumenViewModel) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard(title = "Se está por acabar") {
        if (vm.cuantosConStockBajo == 0 && vm.stockBajo.isEmpty()) {
            Text(
                text = "No se está acabando nada. Cuando a un producto le queden " +
                    "${ResumenApi.UMBRAL_STOCK_BAJO} o menos, aparece acá para que lo encargues " +
                    "antes de quedarte sin.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        Text(
            text = if (vm.cuantosConStockBajo == 1) {
                "1 producto se está acabando."
            } else {
                "${vm.cuantosConStockBajo} productos se están acabando."
            },
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
        )

        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(dimens.space1),
        ) {
            vm.stockBajo.forEach { producto -> FilaDeProducto(producto) }
        }

        if (vm.cuantosConStockBajo > vm.stockBajo.size) {
            Text(
                text = "Y ${vm.cuantosConStockBajo - vm.stockBajo.size} más. Pídeselos al " +
                    "agente: «¿qué se está por acabar?».",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}

/**
 * Un producto que se está acabando.
 *
 * No usa `RbListRow`: esa fila trae su propio alto táctil de 56dp porque está
 * pensada para tocarse, y acá no hay nada que tocar. Poner tres de esas dentro
 * de una tarjeta de resumen la infla al doble sin agregar nada.
 */
@Composable
private fun FilaDeProducto(producto: ProductDto) {
    Text(
        text = "· ${producto.name} — ${
            if (producto.stock <= 0) "sin stock" else "quedan ${producto.stock}"
        }",
        style = RbTheme.typography.body,
        color = RbTheme.colors.textPrimary,
    )
}

@Composable
private fun PorVencer(vm: ResumenViewModel) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard(title = "Se está por vencer") {
        if (vm.cuantosPorVencer == 0 && vm.porVencer.isEmpty()) {
            Text(
                text = "Nada se vence en los próximos ${ResumenApi.DIAS_DE_VENCIMIENTO} días. " +
                    "Cuando algo se acerque a la fecha, aparece acá con cuántos días le quedan.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        Text(
            text = if (vm.cuantosPorVencer == 1) {
                "1 lote se vence pronto."
            } else {
                "${vm.cuantosPorVencer} lotes se vencen pronto."
            },
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
        )

        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(dimens.space1),
        ) {
            vm.porVencer.take(ResumenApi.MAXIMO_EN_LISTA).forEach { lote ->
                Text(
                    text = "· ${lote.producto} — ${plazoDeVencimiento(lote)}",
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
            }
        }

        val listados = minOf(vm.porVencer.size, ResumenApi.MAXIMO_EN_LISTA)
        if (vm.cuantosPorVencer > listados) {
            Text(
                text = "Y ${vm.cuantosPorVencer - listados} más.",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}

/** Cuánto le queda a un lote, dicho como lo diría una persona. */
private fun plazoDeVencimiento(lote: LotePorVencerDto): String = when {
    lote.expired -> "ya vencido"
    lote.diasParaVencer <= 0L -> "vence hoy"
    lote.diasParaVencer == 1L -> "vence mañana"
    else -> "le quedan ${lote.diasParaVencer} días"
}

/**
 * Si un monto del server es cero.
 *
 * Se pregunta por el valor y no por el texto: el server puede mandar `"0"`,
 * `"0.00"` o `"0.0000"` para la misma nada, y comparar cadenas fallaría en dos
 * de los tres casos.
 */
private fun esCero(montoDelServidor: String): Boolean =
    Dinero.deTextoDeServidor(montoDelServidor)?.let { it.unidades == 0L } ?: false
