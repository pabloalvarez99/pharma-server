package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbConfirmDialog
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 4: contar la plata antes de cerrar.
 *
 * Esta pantalla **no muestra cuánto debería haber**, y es a propósito. Si el
 * número está a la vista, contar deja de ser contar y pasa a ser copiar: la
 * dueña escribe lo que dice la pantalla, el cierre siempre cuadra y la caja deja
 * de servir para lo único que sirve. El dato está en la pantalla anterior, a un
 * toque, para quien lo quiera mirar; acá el campo va en blanco.
 *
 * La nota va **antes** del cierre porque es cuando el server la acepta: el
 * cierre graba `closing_notes` en la misma llamada y no hay endpoint para
 * agregarla después. Por eso la ayuda del campo invita a escribir lo que ya se
 * sabe — el vuelto que se dio mal, la compra que no se anotó — sin esperar a ver
 * la diferencia.
 */
@Composable
fun PasoArqueo(vm: CajaViewModel, modifier: Modifier = Modifier) {
    FormularioDeArqueo(
        moneda = vm.moneda,
        montoContado = vm.montoContado,
        onMontoContado = vm::cambiarMontoContado,
        nota = vm.notaDeCierre,
        onNota = vm::cambiarNotaDeCierre,
        impedimento = vm.impedimentoParaCerrar(),
        error = vm.errorDeAccion,
        guardando = vm.guardando,
        onCerrar = vm::cerrarCaja,
        onVolver = vm::volverAlEstado,
        modifier = modifier,
    )
}

/**
 * El formulario del arqueo, sin `ViewModel` detrás.
 *
 * Recibe datos y callbacks para poder montarse en una prueba sin server ni
 * caja abierta — mismo criterio que
 * [cl.rutbusiness.app.ui.resumen.TarjetaDelDia]. Acá importa más que en
 * cualquier otra pantalla: es la que hay que medir al 200% de escala y con el
 * teclado numérico arriba, y es la que tiene números que no se pueden cortar ni
 * confundir.
 *
 * Tres cosas del layout que no son estéticas:
 *
 * - **El campo del monto va primero**, arriba de todo lo demás.
 * - La columna **scrollea** y lleva `imePadding`, así el teclado numérico empuja
 *   el contenido en vez de taparlo y el botón de cerrar sigue alcanzable.
 * - Los botones van **uno debajo del otro y a todo el ancho**: en fila, al 200%,
 *   sus etiquetas se cortan.
 */
@Composable
internal fun FormularioDeArqueo(
    moneda: Moneda,
    montoContado: String,
    onMontoContado: (String) -> Unit,
    nota: String,
    onNota: (String) -> Unit,
    impedimento: String?,
    error: RbErrorCopy?,
    guardando: Boolean,
    onCerrar: () -> Unit,
    onVolver: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    var confirmando by rememberSaveable { mutableStateOf(false) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = "¿Cuánta plata hay en el cajón?") {
            Text(
                text = "Saca la plata, cuéntala tranquila y escribe el total. Recién después de " +
                    "cerrar te mostramos cómo quedó contra lo que el sistema tenía anotado.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            RbTextField(
                value = montoContado,
                onValueChange = onMontoContado,
                label = "Plata contada",
                placeholder = "0",
                supportingText = "Cuenta billetes y monedas. Si el cajón quedó vacío, escribe 0.",
                numeric = true,
                keyboardType = tecladoDePlata(moneda),
                enabled = !guardando,
            )
        }

        RbCard(title = "¿Algo que anotar?") {
            RbTextField(
                value = nota,
                onValueChange = onNota,
                label = "Nota del cierre (opcional)",
                placeholder = "Le di mal el vuelto a un cliente",
                supportingText = "Si ya sabes que algo no va a cuadrar, escríbelo acá: después de " +
                    "cerrar no se puede agregar.",
                enabled = !guardando,
            )
        }

        error?.let { copy ->
            RbErrorState(
                title = copy.title,
                message = copy.message,
                retryLabel = copy.retryLabel,
                onRetry = if (copy.retryLabel != null) onCerrar else null,
            )
        }

        impedimento?.let { motivo ->
            Text(
                text = motivo,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        RbButton(
            label = if (guardando) "Cerrando..." else "Cerrar la caja",
            onClick = { confirmando = true },
            enabled = !guardando && impedimento == null,
            fillWidth = true,
        )

        RbButton(
            label = "Todavía no",
            onClick = onVolver,
            variant = RbButtonVariant.Secondary,
            enabled = !guardando,
            fillWidth = true,
        )
    }

    if (confirmando) {
        RbConfirmDialog(
            title = "¿Cerramos la caja?",
            // Se dice qué es lo que no se puede deshacer, sin adjetivos: cerrar
            // no es peligroso, pero es definitivo, y la dueña tiene derecho a
            // saberlo antes y no después.
            message = "La caja queda cerrada con los " +
                "${moneda.formatear(montoContado.replace(',', '.'))} que contaste. Después no " +
                "se puede volver a abrir la misma caja: mañana se abre una nueva.",
            confirmLabel = "Sí, cerrar",
            cancelLabel = "Seguir contando",
            onConfirm = {
                confirmando = false
                onCerrar()
            },
            onDismiss = { confirmando = false },
        )
    }
}
