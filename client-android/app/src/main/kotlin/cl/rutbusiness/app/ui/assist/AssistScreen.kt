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
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbLoadingState
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
    onVerCatalogo: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val lista = rememberLazyListState()

    // Al llegar un mensaje nuevo, se baja hasta el final. Sin esto la dueña
    // manda una pregunta y la respuesta aparece fuera de pantalla.
    LaunchedEffect(vm.mensajes.size) {
        if (vm.mensajes.isNotEmpty()) lista.animateScrollToItem(vm.mensajes.lastIndex)
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Pídele a tu negocio",
            actions = {
                RbButton(
                    label = "Productos",
                    onClick = onVerCatalogo,
                    variant = RbButtonVariant.Secondary,
                )
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

@Composable
private fun Problema(mensaje: Mensaje.Problema, onReintentar: (String) -> Unit) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RbTheme.shapes.card)
            .background(colors.dangerContainer)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Text(
            text = mensaje.texto,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )
        mensaje.preguntaParaReintentar?.let { pregunta ->
            RbButton(
                label = "Volver a preguntar",
                onClick = { onReintentar(pregunta) },
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
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
            text = "Escríbeme como le hablarías a un empleado de confianza. " +
                "Antes de guardar nada, te lo muestro para que lo revises.",
            style = RbTheme.typography.body,
            color = RbTheme.colors.textSecondary,
        )

        Text(
            text = "Toca una para empezar",
            style = RbTheme.typography.label,
            color = RbTheme.colors.textSecondary,
        )

        RbChipRow {
            SUGERENCIAS.forEach { sugerencia ->
                RbChip(label = sugerencia, onClick = { onElegir(sugerencia) })
            }
        }
    }
}

/**
 * Lo que se le puede pedir, en las palabras que el parser reconoce.
 *
 * Mezcla a propósito preguntas de mirar y órdenes de anotar: la dueña tiene
 * que descubrir temprano que el agente además **hace** cosas, porque ésa es la
 * parte del producto que no se adivina.
 */
private val SUGERENCIAS = listOf(
    "¿Cuánto vendí hoy?",
    "¿Quién me debe plata?",
    "¿Qué se está por acabar?",
    "Registra un gasto de 5000 en arriendo",
    "¿Cuánto vendí este mes?",
)

/** El campo de abajo. */
@Composable
private fun Redactor(
    texto: String,
    habilitado: Boolean,
    onEscribir: (String) -> Unit,
    onEnviar: () -> Unit,
) {
    val dimens = RbTheme.dimens

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
        RbTextField(
            value = texto,
            onValueChange = onEscribir,
            label = "¿Qué necesitas?",
            placeholder = "Escribe o toca una sugerencia",
            enabled = habilitado,
            imeAction = ImeAction.Send,
            onImeAction = onEnviar,
        )
        RbButton(
            label = "Enviar",
            onClick = onEnviar,
            enabled = habilitado && texto.isNotBlank(),
            fillWidth = true,
        )
    }
}
