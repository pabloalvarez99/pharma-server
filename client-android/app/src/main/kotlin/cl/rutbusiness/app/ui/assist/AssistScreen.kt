package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.ImeAction
import cl.rutbusiness.app.ui.catalogo.abrirCatalogo
import cl.rutbusiness.app.ui.catalogo.copyCatalogo
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
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
 */
@Composable
fun AssistScreen(
    vm: AssistViewModel,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val lista = rememberLazyListState()

    // La Screen lee esFeria(); el VM no puede tocar CompositionLocal.
    val feria = esFeria()
    LaunchedEffect(feria) {
        if (feria) vm.modoFeria(true)
    }

    // Al llegar un mensaje nuevo, se baja hasta el final. Sin esto la dueña
    // manda una pregunta y la respuesta aparece fuera de pantalla.
    LaunchedEffect(vm.mensajes.size) {
        if (vm.mensajes.isNotEmpty()) lista.animateScrollToItem(vm.mensajes.lastIndex)
    }

    val abrirLoQueVendo = abrirCatalogo()
    val copyDelCatalogo = copyCatalogo(packActual())

    Column(modifier = modifier.fillMaxSize()) {
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
            contentPadding = PaddingValues(vertical = dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            if (vm.recienEmpezando) {
                item { Bienvenida(onElegir = vm::preguntar) }
            }

            items(
                count = vm.mensajes.size,
                key = { vm.mensajes[it].id },
            ) { indice ->
                MensajeEnLista(vm = vm, mensaje = vm.mensajes[indice])
            }

            if (vm.pensando) {
                item { RbLoadingState(label = "Estoy revisando tus datos…") }
            }
        }

        Redactor(
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
private fun MensajeEnLista(vm: AssistViewModel, mensaje: Mensaje) {
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
            onReintentar = { vm.preguntar(it) },
        )
    }
}

/**
 * Un turno de la conversación.
 *
 * Lo de la dueña va alineado a la derecha y con fondo de marca; lo del agente
 * a la izquierda sobre la superficie. La diferencia es de color **y** de lado:
 * distinguir por color solo deja afuera a quien no lo distingue.
 */
@Composable
private fun Burbuja(texto: String, mia: Boolean) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (mia) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth(0.92f)
                .clip(RbTheme.shapes.card)
                .background(if (mia) colors.brandContainer else colors.surface)
                .padding(dimens.space3),
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
private fun Problema(mensaje: Mensaje.Problema, onReintentar: (String) -> Unit) {
    RbErrorState(
        title = mensaje.titulo,
        message = mensaje.texto,
        retryLabel = mensaje.preguntaParaReintentar?.let { "Volver a preguntar" },
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
 */
@Composable
private fun Bienvenida(onElegir: (String) -> Unit) {
    val dimens = RbTheme.dimens
    val pack = packActual()
    val sugerencias = AssistSugerencias.para(pack)
    val intro = AssistSugerencias.intro(pack)

    // Deliberadamente compacto, y sin `RbEmptyState`: ese componente reserva
    // 40dp arriba y abajo para el vacío de una lista, y acá esa respiración
    // empujaba la primera sugerencia debajo del pliegue. Una sugerencia que hay
    // que buscar scrolleando no enseña nada — el punto entero es que la dueña
    // vea algo que tocar sin hacer nada.
    Column(verticalArrangement = Arrangement.spacedBy(dimens.space3)) {
        Text(
            text = "Pídeme lo que necesites",
            style = RbTheme.typography.heading,
            color = RbTheme.colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )

        Text(
            text = intro,
            style = RbTheme.typography.body,
            color = RbTheme.colors.textSecondary,
        )

        Text(
            text = "Toca una para empezar",
            style = RbTheme.typography.label,
            color = RbTheme.colors.textSecondary,
        )

        RbChipRow {
            sugerencias.forEach { sugerencia ->
                RbChip(label = sugerencia, onClick = { onElegir(sugerencia) })
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
    texto: String,
    habilitado: Boolean,
    onEscribir: (String) -> Unit,
    onEnviar: () -> Unit,
    onDictado: (String) -> Unit,
) {
    val dimens = RbTheme.dimens
    val dictado = LocalDictadoDeVoz.current?.recordarSesion(onTexto = onDictado)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(RbTheme.colors.surface)
            // `imePadding` sube el campo cuando aparece el teclado; sin esto el
            // teclado tapa justo lo que se está escribiendo.
            .imePadding()
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            )
            .padding(dimens.space3),
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
                    label = "¿Qué necesitas?",
                    placeholder = if (dictado == null) {
                        "Escribe o toca una sugerencia"
                    } else {
                        "Habla o escribe"
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
                color = RbTheme.colors.textSecondary,
            )
        }

        RbButton(
            label = "Enviar",
            onClick = onEnviar,
            enabled = habilitado && texto.isNotBlank(),
            fillWidth = true,
        )
    }
}
