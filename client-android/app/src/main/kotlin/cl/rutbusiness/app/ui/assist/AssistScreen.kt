package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import cl.rutbusiness.app.ui.catalogo.abrirCatalogo
import cl.rutbusiness.app.ui.catalogo.copyCatalogo
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Hablarle al negocio.
 *
 * Según el founder ésta es la interfaz principal del producto: la dueña no
 * navega menús, le pide las cosas al agente. Todo lo demás de la app existe
 * para cuando esto no alcanza.
 *
 * La lista es un `LazyColumn` por la misma razón que todas las del producto:
 * el aparato objetivo tiene 1-2 GB de RAM y una conversación larga no puede
 * estar entera compuesta.
 *
 * Visual: conversación de puesto, no chatbot SaaS. Burbujas con aire, chips
 * tocables con tono de marca, y la lista sobre background para que surface /
 * brandContainer se lean distintos al sol.
 */
@Composable
fun AssistScreen(
    vm: AssistViewModel,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val motion = RbTheme.motion
    val lista = rememberLazyListState()

    // La Screen lee esFeria(); el VM no puede tocar CompositionLocal.
    val feria = esFeria()
    val chrome = remember(feria) { copyAssistChrome(feria) }
    LaunchedEffect(feria) {
        if (feria) vm.modoFeria(true)
    }

    // Al llegar un mensaje nuevo, se baja hasta el final. Sin esto la dueña
    // manda una pregunta y la respuesta aparece fuera de pantalla. Con
    // "reducir animaciones" el salto es instantáneo (RbTheme.motion).
    LaunchedEffect(vm.mensajes.size, motion.reduced) {
        if (vm.mensajes.isEmpty()) return@LaunchedEffect
        val ultimo = vm.mensajes.lastIndex
        if (motion.reduced) {
            lista.scrollToItem(ultimo)
        } else {
            lista.animateScrollToItem(ultimo)
        }
    }

    val abrirLoQueVendo = abrirCatalogo()
    val copyDelCatalogo = copyCatalogo(packActual())

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.background),
    ) {
        // **Buscar** un producto sigue sin estar acá: eso vive dentro de Cobrar,
        // a una pestaña de distancia, y repetirlo sería un segundo camino al
        // mismo lugar — de los que enseñan mal, porque compiten con la barra.
        //
        // **Cargar** lo que se vende es otra cosa, y hasta ahora no estaba en
        // ninguna parte: el único camino al catálogo pasaba por el escáner, que
        // en feria está apagado a propósito. Un negocio recién abierto entra por
        // esta pantalla y tiene que poder cargar su primera cosa desde acá, sin
        // que nadie le explique dónde mirar.
        RbTopBar(
            title = vm.titulo,
            actions = if (abrirLoQueVendo == null) {
                null
            } else {
                {
                    RbButton(
                        label = copyDelCatalogo.titulo,
                        onClick = abrirLoQueVendo,
                        variant = RbButtonVariant.Secondary,
                    )
                }
            },
        )

        LazyColumn(
            state = lista,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(horizontal = dimens.space3),
            // Más aire vertical: los turnos no se pegan como un feed de bot.
            contentPadding = PaddingValues(vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            if (vm.recienEmpezando) {
                item { Bienvenida(chrome = chrome, onElegir = vm::preguntar) }
            }

            items(
                count = vm.mensajes.size,
                key = { vm.mensajes[it].id },
            ) { indice ->
                MensajeEnLista(
                    vm = vm,
                    mensaje = vm.mensajes[indice],
                    reintentar = chrome.reintentar,
                )
            }

            if (vm.pensando) {
                item { RbLoadingState(label = chrome.pensando) }
            }
        }

        Redactor(
            chrome = chrome,
            texto = vm.borrador,
            habilitado = !vm.pensando,
            onEscribir = vm::escribir,
            onEnviar = vm::enviarBorrador,
            // Lo dictado entra por **el mismo camino** que lo tipeado: cae en el
            // campo y se manda. No hay un `preguntarPorVoz` en el ViewModel, a
            // propósito: un segundo camino al server es un segundo lugar donde
            // arreglar las cosas, y el día que "hablar" y "escribir" se porten
            // distinto nadie va a saber por qué.
            //
            // Se manda solo, sin un toque más, porque la mano que sostiene la
            // bolsa es la misma que tendría que apretar Enviar. Lo que protege
            // de una palabra mal entendida no es un toque extra: es que el
            // agente **propone y espera confirmación** antes de escribir nada.
            onDictado = { dicho ->
                vm.escribir(dicho)
                vm.enviarBorrador()
            },
        )
    }
}

@Composable
private fun MensajeEnLista(
    vm: AssistViewModel,
    mensaje: Mensaje,
    reintentar: String,
) {
    when (mensaje) {
        is Mensaje.Mia -> Burbuja(texto = mensaje.texto, mia = true)

        is Mensaje.DelAgente -> Burbuja(texto = mensaje.texto, mia = false)

        is Mensaje.Propuesta -> TarjetaPropuesta(
            mensaje = mensaje,
            segundosRestantes = remember(mensaje.id) { vm.segundosRestantesDe(mensaje.propuesta) },
            onConfirmar = { vm.confirmar(mensaje.id) },
            onCancelar = { vm.cancelar(mensaje.id) },
            onVencer = { vm.marcarVencida(mensaje.id) },
            onVolverAPedir = vm::volverAPedir,
        )

        is Mensaje.Problema -> Problema(
            mensaje = mensaje,
            reintentar = reintentar,
            onReintentar = { vm.preguntar(it) },
        )
    }
}

/**
 * Un turno de la conversación.
 *
 * Lo de la dueña va a la derecha sobre brandContainer; lo del agente a la
 * izquierda sobre surface con borde. Color **y** lado: distinguir solo por
 * tinte deja afuera a quien no ve el color. textPrimary en ambos = AAA.
 */
@Composable
private fun Burbuja(texto: String, mia: Boolean) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val radio = dimens.radiusCard
    // Esquina "mordida" del lado del hablante: se lee como burbuja de charla,
    // no como tarjeta de dashboard.
    val pico = 4.dp
    val forma = if (mia) {
        RoundedCornerShape(
            topStart = radio,
            topEnd = radio,
            bottomStart = radio,
            bottomEnd = pico,
        )
    } else {
        RoundedCornerShape(
            topStart = radio,
            topEnd = radio,
            bottomStart = pico,
            bottomEnd = radio,
        )
    }
    val fondo = if (mia) colors.brandContainer else colors.surface

    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (mia) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                // 0.86 deja aire a los lados: no es un panel de ancho completo.
                .fillMaxWidth(0.86f)
                .clip(forma)
                .background(fondo)
                .then(
                    if (mia) {
                        Modifier
                    } else {
                        // Borde fuerte: surface sobre background se pierde al sol.
                        Modifier.border(dimens.border, colors.outlineStrong, forma)
                    },
                )
                .padding(horizontal = dimens.space3, vertical = dimens.space3),
        ) {
            Text(
                text = texto,
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )
        }
    }
}

/**
 * Un turno que falló.
 *
 * [RbErrorState] y no un párrafo en un fondo rojo hecho a mano: así esta
 * pantalla dice sus fallas igual que las otras seis, y sobre todo así el lector
 * de pantalla las anuncia —`rbAssertive`— en vez de dejarlas pasar como un
 * mensaje más de la conversación.
 */
@Composable
private fun Problema(
    mensaje: Mensaje.Problema,
    reintentar: String,
    onReintentar: (String) -> Unit,
) {
    RbErrorState(
        title = mensaje.titulo,
        message = mensaje.texto,
        retryLabel = mensaje.preguntaParaReintentar?.let { reintentar },
        onRetry = mensaje.preguntaParaReintentar?.let { pregunta -> { onReintentar(pregunta) } },
    )
}

/**
 * La primera vez.
 *
 * Una persona mayor frente a un campo de texto vacío no sabe qué escribir, y
 * "escribe tu pregunta" no ayuda. Las sugerencias son frases **completas y
 * tocables**: se aprieta una y pasa algo, sin tipear nada. De paso enseñan el
 * tono con que hay que pedirle las cosas.
 *
 * Están escritas con las palabras que el agente entiende de verdad — el parser
 * de `crates/assist/src/actions.rs` es determinista y busca ciertas palabras —
 * así que una sugerencia nunca termina en "no te entendí". Una sugerencia que
 * falla es peor que no ponerla.
 *
 * El bloque de chips va en una tarjeta surface con borde: se lee como "toca
 * acá", no como tags decorativos de un bot.
 */
@Composable
private fun Bienvenida(
    chrome: CopyAssistChrome,
    onElegir: (String) -> Unit,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val shapes = RbTheme.shapes
    val pack = packActual()
    val sugerencias = AssistSugerencias.para(pack)
    val intro = AssistSugerencias.intro(pack)

    // Sin `RbEmptyState`: ese componente reserva 40dp arriba y abajo para el
    // vacío de una lista, y acá esa respiración empujaba la primera sugerencia
    // debajo del pliegue. Una sugerencia que hay que buscar scrolleando no
    // enseña nada — el punto es que la dueña vea algo que tocar sin hacer nada.
    Column(verticalArrangement = Arrangement.spacedBy(dimens.space4)) {
        Text(
            text = chrome.tituloBienvenida,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )

        Text(
            text = intro,
            style = RbTheme.typography.body,
            color = colors.textSecondary,
        )

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(shapes.card)
                .background(colors.surface)
                .border(dimens.border, colors.outlineStrong, shapes.card)
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(
                text = chrome.etiquetaChips,
                style = RbTheme.typography.label,
                color = colors.brandText,
            )

            RbChipRow {
                sugerencias.forEach { sugerencia ->
                    // Brand + borde de marca: se ven tocables, no grises de filtro.
                    RbChip(
                        label = sugerencia,
                        tone = RbChipTone.Brand,
                        onClick = { onElegir(sugerencia) },
                    )
                }
            }
        }
    }
}

/**
 * El campo de abajo, y las dos formas de llenarlo.
 *
 * El micrófono va **al lado del campo**, no en lugar de nada: los chips de
 * ejemplo siguen arriba y el teclado sigue siendo el camino que nunca falla.
 * Cuando no hay con qué escuchar -un teléfono sin motor de reconocimiento, un
 * test- el control no se dibuja y esta pantalla es exactamente la de ayer.
 *
 * La fila es [RbReflowRow] y no un `Row`: al 200% de escala de letra, cuando el
 * botón deja al campo más angosto que la palabra más larga de su etiqueta, el
 * botón baja solo a su propio renglón. Es la misma regla que sostiene la barra
 * de título y las filas de la lista, y no tiene ningún umbral de escala que
 * alguien tenga que adivinar.
 */
@Composable
private fun Redactor(
    chrome: CopyAssistChrome,
    texto: String,
    habilitado: Boolean,
    onEscribir: (String) -> Unit,
    onEnviar: () -> Unit,
    onDictado: (String) -> Unit,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val dictado = LocalDictadoDeVoz.current?.recordarSesion(onTexto = onDictado)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.surface)
            // `imePadding` sube el campo cuando aparece el teclado; sin esto el
            // teclado tapa justo lo que se está escribiendo.
            .imePadding()
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            ),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        // Línea superior: el redactor se siente barra del puesto, no otro
        // bloque de chat.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(dimens.border)
                .background(colors.outline),
        )
        Column(
            modifier = Modifier.padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            RbReflowRow(
                spacing = dimens.space2,
                content = {
                    RbTextField(
                        value = texto,
                        onValueChange = { escrito ->
                            // Empezar a escribir baja el aviso del dictado: ya
                            // eligió el otro camino, la línea dejó de servirle.
                            dictado?.olvidarAviso()
                            onEscribir(escrito)
                        },
                        label = chrome.etiquetaCampo,
                        placeholder = if (dictado == null) {
                            chrome.placeholderSinVoz
                        } else {
                            chrome.placeholderConVoz
                        },
                        enabled = habilitado,
                        imeAction = ImeAction.Send,
                        onImeAction = onEnviar,
                    )
                },
                trailing = {
                    if (dictado != null) {
                        BotonDeVoz(
                            escuchando = dictado.escuchando,
                            // Mientras el agente piensa no se dicta, por la misma
                            // razón por la que no se envía: la respuesta que está
                            // por llegar es a la pregunta anterior.
                            habilitado = habilitado,
                            onTocar = dictado::alternar,
                        )
                    }
                },
            )

            // Lo único que el dictado tiene para decir cuando algo no sale. Una
            // línea, del color del texto de apoyo, y siempre termina en cómo seguir.
            dictado?.aviso?.let { linea ->
                Text(
                    text = linea,
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }

            RbButton(
                label = chrome.enviar,
                onClick = onEnviar,
                enabled = habilitado && texto.isNotBlank(),
                fillWidth = true,
            )
        }
    }
}
