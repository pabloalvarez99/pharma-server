package cl.rutbusiness.app.ui.resumen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import cl.rutbusiness.app.ui.agente.irAlAgente
import cl.rutbusiness.app.ui.gente.RecadoParaGente
import cl.rutbusiness.app.ui.gente.etiquetaCompartirDia
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
import cl.rutbusiness.ui.icons.RbIcons
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * @param onIrALaCaja abre el ritual de la caja. Sale del bloque "En la caja",
 *   que es donde la dueña ya está mirando cuando se lo pregunta.
 * @param onIrAlFiado abre "quién me debe", desde el bloque que muestra el total.
 * @param onAbrirMenu abre el menú del puesto (caja en feria, impresora, ventas
 *   pendientes, cambiar de rubro). Vive en la barra de arriba, al lado de
 *   "Actualizar" — es el único lugar de toda la app donde alcanza sin pasar
 *   por Cobrar ni por el agente, y por eso lo carga "Tu día" y no una pestaña
 *   propia (la barra de abajo está tope en tres, ver `BarraDeNavegacion.kt`).
 * @param recargasPedidas cambia cada vez que se vuelve de una de esas
 *   pantallas. El `ViewModel` sobrevive al cambio y su carga inicial ya corrió,
 *   así que sin esto el resumen mostraría la caja y la deuda de hace un rato.
 */
@Composable
fun ResumenRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onIrALaCaja: () -> Unit = {},
    onIrAlFiado: () -> Unit = {},
    onAbrirMenu: () -> Unit = {},
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

    ResumenScreen(vm = vm, onIrALaCaja = onIrALaCaja, onIrAlFiado = onIrAlFiado, onAbrirMenu = onAbrirMenu)
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
    onAbrirMenu: () -> Unit,
) {
    val dimens = RbTheme.dimens

    val feria = esFeria()

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = tituloHoy(feria),
            // Cuando lo que se muestra es la copia guardada, el subtítulo lo
            // dice ahí mismo: es la primera línea que se lee y no se puede
            // saltar. "Cómo va el negocio hoy" arriba de cifras de anoche sería
            // la mentira más cara de esta pantalla.
            subtitle = vm.guardadoEn?.let { guardadoEn ->
                "Datos guardados en el teléfono, " +
                    Fechado(Unit, guardadoEn).antiguedad(vm.relojDePared)
            } ?: vm.nombreDelPuesto?.takeIf { feria && it.isNotBlank() }
                ?: subtituloHoy(feria),
            actions = {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(dimens.space2),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RbButton(
                        label = labelActualizarHoy(feria),
                        onClick = vm::cargar,
                        variant = RbButtonVariant.Secondary,
                        enabled = !vm.cargando,
                    )
                    // Puerta al menú del puesto: sin esto la impresora, las
                    // ventas que faltan subir y cambiar de rubro no tenían
                    // ningún camino desde la interfaz. Botón chico y al lado
                    // de "Actualizar" — nunca en Cobrar, que es la pantalla
                    // que no se puede ensuciar.
                    RbButton(
                        label = labelAbrirMenuHoy(),
                        onClick = onAbrirMenu,
                        variant = RbButtonVariant.Secondary,
                    )
                }
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
                RbLoadingState(label = cargandoHoy(feria))
                RbSkeletonLines(lines = 4, modifier = Modifier.padding(dimens.space3))
            }

            // `weight(1f)` y no `fillMaxSize()`: adentro de la columna que
            // empieza con la barra de arriba, `fillMaxSize` pide el alto entero
            // de la pantalla y la lista termina dibujada encima del título.
            else -> LazyColumn(
                modifier = Modifier.weight(1f),
                // space4 entre tarjetas: se leen de a una, no como grilla de KPI.
                contentPadding = PaddingValues(dimens.space3),
                verticalArrangement = Arrangement.spacedBy(dimens.space4),
            ) {
                // El orden es [ordenDeBloquesHoy]: cuánto entró, cuánto le
                // deben y —solo en retail formal— qué falta cerrar. Fijado ahí
                // y no acá para que un test lo pueda verificar sin Compose.
                ordenDeBloquesHoy(feria).forEach { bloque ->
                    when (bloque) {
                        "ventas" -> item("ventas") { VendidoHoy(vm) }
                        "fiado" -> item("fiado") { TeDeben(vm, onIrAlFiado) }
                        "caja" -> item("caja") { EnCaja(vm, onIrALaCaja) }
                        "faltantes" -> item("faltantes") { SePorAcabar(vm) }
                        "vencimientos" -> item("vencimientos") { PorVencer(vm) }
                    }
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
 * [HoySinVentas]: ver "$0 · 0 ventas · menos que ayer" es enterarse tres veces
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
    val feria = esFeria()
    if (feria && boletas == 0L) {
        HoySinVentas(onHablarleAlAgente = irAlAgente())
        return
    }

    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    // Aire extra: la cifra es lo único que se mira de lejos; el resto baja
    // callado. No es un tablero de filas — es una tarjeta de puesto.
    RbCard {
        Text(
            text = tituloVendidoHoy(),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
            modifier = Modifier.rbHeading(),
        )

        RbAmount(
            amount = moneda.formatear(vendidoHoy),
            emphasis = RbAmountEmphasis.Headline,
            modifier = Modifier.padding(vertical = dimens.space2),
        )

        Text(
            text = copyTarjetaConteo(feria = feria, conteo = boletas),
            style = RbTheme.typography.body,
            color = colors.textSecondary,
        )

        Column(
            modifier = Modifier.padding(top = dimens.space2),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            RbChip(
                label = etiquetaComparacion(comparacion),
                // El tono acompaña, no informa: la palabra ya lo dice completo.
                // Quien no distingue los colores lee exactamente lo mismo.
                tone = when (comparacion) {
                    Comparacion.Mejor -> RbChipTone.Brand
                    Comparacion.Peor -> RbChipTone.Warn
                    else -> RbChipTone.Neutral
                },
            )

            Text(
                text = pistaComparacion(
                    feria = feria,
                    vendidoAyerFormateado = vendidoAyer?.let { moneda.formatear(it) },
                ),
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        // Contar cómo va el día — al socio, a la casa, al hijo que atiende el
        // otro puesto. Sólo con al menos una venta: el vacío de "Hoy" enseña a
        // anotar la primera, y ofrecer ahí "compartí que no vendiste nada" es
        // una broma pesada.
        //
        // Va como recado (nota + CTA humano), no como "exportar el resumen".
        // Sin puerto de plataforma el bloque no se dibuja.
        if (boletas > 0L) {
            RecadoParaGente(
                mensaje = mensajeHoy(
                    copyResumenDelDia(
                        feria = feria,
                        monto = moneda.formatear(vendidoHoy),
                        conteo = boletas,
                    ),
                    feria = feria,
                ),
                etiqueta = etiquetaCompartirDia(feria),
            )
        }
    }
}

/**
 * "Hoy", antes de la primera venta del día.
 *
 * Un cuaderno en blanco tampoco dice nada, y sin embargo la dueña sabe qué hacer
 * con él: anotar. Las cuatro partes de un vacío producido — la marca de
 * [RbIcons.resumenContorno] (antes esta pantalla no tenía ninguna), el título,
 * la frase de [pistaHoySinVentas] que ya enseña tanto el ejemplo como la
 * ganancia ("acá vas a ver cuánto llevas, sin sumar a mano"), y el botón que
 * lleva a decirlo. Sin asset de ilustración (pesa en el APK del teléfono
 * viejo); el vacío se resuelve con marca, tipografía y espacio.
 *
 * Delegado en [RbEmptyState] y sin [RbCard] alrededor: el card ya aporta su
 * propio `padding`, y sumarle el del vacío encima duplicaba el aire en vez de
 * ocuparlo con algo. La tarjeta blanca lisa sin marca era justo el "tarjeta
 * arriba, resto en blanco liso" que señaló la auditoría de capturas.
 *
 * Sin [onHablarleAlAgente] el botón no se dibuja. La frase sigue enseñando el
 * paso, y en feria el agente está a un toque en la barra de abajo; un botón que
 * no lleva a ninguna parte sería peor que no tenerlo.
 */
@Composable
private fun HoySinVentas(onHablarleAlAgente: (() -> Unit)?) {
    RbEmptyState(
        title = tituloHoySinVentas(),
        icon = RbIcons.resumenContorno,
        hint = pistaHoySinVentas(),
        actionLabel = onHablarleAlAgente?.let { ctaHablarleAlAgenteHoy() },
        onAction = onHablarleAlAgente,
    )
}

// --- 3: cuánta plata hay en caja -------------------------------------------

@Composable
private fun EnCaja(vm: ResumenViewModel, onIrALaCaja: () -> Unit) {
    val colors = RbTheme.colors
    val enCaja = vm.enCaja
    // Card solo se monta en retail formal, pero el helper acepta feria por si
    // algún día se muestra: copyEnCajaExplicacion ya es honesto en ambos.
    val feria = esFeria()

    RbCard(title = tituloEnCajaHoy()) {
        when {
            enCaja != null -> {
                RbAmount(amount = vm.moneda.formatear(enCaja))
                Text(
                    text = copyEnCajaExplicacion(
                        feria = feria,
                        nombreDeCaja = vm.nombreDeCaja,
                    ),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }

            vm.sinCajaAbierta -> Text(
                text = sinCajaAbiertaHoy(),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            // "Arqueo" es la palabra del contador, no la de la dueña. Y el
            // mensaje tenía que decir qué hacer, no sólo qué faltó.
            else -> Text(
                text = errorEnCajaHoy(feria),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
        }

        // Abrir la caja es lo primero que hace la dueña en el día, así que
        // cuando no hay ninguna abierta el botón es el principal de la tarjeta y
        // no una acción secundaria escondida.
        RbButton(
            label = ctaCajaHoy(sinCajaAbierta = vm.sinCajaAbierta),
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
    val dimens = RbTheme.dimens
    val deuda = vm.porCobrar
    val hayDeuda = deuda != null && deuda.deudores > 0 && !esCero(deuda.total)
    val feria = esFeria()

    // Franja humana, no ERP: plata que te deben personas, no "cuentas por cobrar".
    RbCard(title = tituloTeDebenHoy()) {
        when {
            deuda == null -> Text(
                text = errorFiadoHoy(feria),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            // En feria fiar no es un botón de la pantalla de cobro: es una
            // frase. El vacío la enseña acá, que es donde la dueña se está
            // preguntando por la deuda, y no en una pantalla más adentro.
            !hayDeuda -> Text(
                text = vacioFiadoHoy(feria),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            else -> {
                RbAmount(
                    amount = vm.moneda.formatear(deuda.total),
                    modifier = Modifier.padding(vertical = dimens.space1),
                )
                Text(
                    text = cuantosTeDebenHoy(deuda.deudores),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        }

        // El botón está incluso cuando nadie debe: es la única puerta al fiado, y
        // esconderla dejaría sin forma de llegar a la cuenta de alguien que ya
        // terminó de pagar.
        RbButton(
            label = ctaFiadoHoy(hayDeuda),
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
                text = vacioSePorAcabar(ResumenApi.UMBRAL_STOCK_BAJO),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        Text(
            text = tituloSePorAcabar(vm.cuantosConStockBajo),
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
                text = masSePorAcabar(vm.cuantosConStockBajo - vm.stockBajo.size),
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
        text = filaSePorAcabar(producto.name, producto.stock),
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
                text = vacioPorVencer(ResumenApi.DIAS_DE_VENCIMIENTO),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        Text(
            text = tituloPorVencer(vm.cuantosPorVencer),
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
        )

        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(dimens.space1),
        ) {
            vm.porVencer.take(ResumenApi.MAXIMO_EN_LISTA).forEach { lote ->
                Text(
                    text = filaPorVencer(
                        producto = lote.producto,
                        plazo = plazoDeVencimiento(lote.diasParaVencer, lote.expired),
                    ),
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
            }
        }

        val listados = minOf(vm.porVencer.size, ResumenApi.MAXIMO_EN_LISTA)
        if (vm.cuantosPorVencer > listados) {
            Text(
                text = masPorVencer(vm.cuantosPorVencer - listados),
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
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
