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
 * Paso 4: contar la plata antes de cerrar el día (o la caja).
 *
 * Esta pantalla **no muestra cuánto debería haber**, y es a propósito. Si el
 * número está a la vista, contar deja de ser contar y pasa a ser copiar: la
 * dueña escribe lo que dice la pantalla, el cierre siempre cuadra y deja de
 * servir para lo único que sirve. El dato está en la pantalla anterior, a un
 * toque; acá el campo va en blanco.
 *
 * En feria el tono es el del cuaderno: puesto, contar la plata, cerrar el día —
 * nunca «arqueo», «cajón» ni «sesión».
 *
 * La nota va **antes** del cierre porque es cuando el server la acepta: el
 * cierre graba `closing_notes` en la misma llamada y no hay endpoint para
 * agregarla después. Por eso la ayuda del campo invita a escribir lo que ya se
 * sabe — el vuelto que se dio mal, compré cambio — sin esperar a ver la
 * diferencia.
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
        feria = vm.esFeria,
        modifier = modifier,
    )
}

/**
 * El formulario de contar la plata, sin `ViewModel` detrás.
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
 * - **El campo del monto va primero**, arriba de todo lo demás (numérico, piso
 *   táctil 56 dp del design system).
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
    feria: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val copy = copyArqueoCaja(feria)

    var confirmando by rememberSaveable { mutableStateOf(false) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        RbCard(title = copy.tituloCard) {
            Text(
                text = copy.ayuda,
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            // Campo grande de monto primero: tipografía numérica + piso 56 dp
            // (RbTextField). Al 200% de escala crece con sp; no se fija en px.
            RbTextField(
                value = montoContado,
                onValueChange = onMontoContado,
                label = copy.etiquetaMonto,
                placeholder = "0",
                supportingText = copy.ayudaMonto,
                numeric = true,
                keyboardType = tecladoDePlata(moneda),
                enabled = !guardando,
            )
        }

        RbCard(title = copy.tituloNota) {
            RbTextField(
                value = nota,
                onValueChange = onNota,
                label = copy.etiquetaNota,
                placeholder = copy.placeholderNota,
                supportingText = copy.ayudaNota,
                enabled = !guardando,
            )
        }

        error?.let { err ->
            RbErrorState(
                title = err.title,
                message = err.message,
                retryLabel = err.retryLabel,
                onRetry = if (err.retryLabel != null) onCerrar else null,
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
            label = if (guardando) copy.ctaGuardando else copy.cta,
            onClick = { confirmando = true },
            enabled = !guardando && impedimento == null,
            fillWidth = true,
        )

        RbButton(
            label = copy.ctaVolver,
            onClick = onVolver,
            variant = RbButtonVariant.Secondary,
            enabled = !guardando,
            fillWidth = true,
        )
    }

    if (confirmando) {
        val montoFormateado = moneda.formatear(montoContado.replace(',', '.'))
        RbConfirmDialog(
            title = copy.confirmarTitulo,
            // Se dice qué es lo que no se puede deshacer, sin adjetivos: cerrar
            // no es peligroso, pero es definitivo, y la dueña tiene derecho a
            // saberlo antes y no después.
            message = copyConfirmarCierre(feria, montoFormateado),
            confirmLabel = copy.confirmarLabel,
            cancelLabel = copy.cancelarLabel,
            onConfirm = {
                confirmando = false
                onCerrar()
            },
            onDismiss = { confirmando = false },
        )
    }
}
