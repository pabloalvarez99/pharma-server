package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.rbAmountStyle
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Paso 5: la caja quedó cerrada y ésta es la diferencia.
 *
 * La pantalla más delicada del día. Lo que se muestra es lo que el server grabó
 * en el cierre — `closing_cash_counted`, `closing_cash_expected` y
 * `discrepancia` — sin volver a restarlos acá. Si el teléfono recalculara y le
 * errara por un peso, estaría acusando a alguien de un peso que no falta.
 *
 * El texto lo arma [copyDeDiferencia], que es una función pura y está probada
 * palabra por palabra.
 */
@Composable
fun PasoCajaCerrada(vm: CajaViewModel, onListo: () -> Unit, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val cerrada = vm.cierre?.session

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        TarjetaDeDiferencia(
            moneda = vm.moneda,
            contadoDelServidor = cerrada?.contado,
            esperadoDelServidor = cerrada?.esperado,
            discrepanciaDelServidor = cerrada?.discrepancia,
            notaDeCierre = cerrada?.notaDeCierre,
        )

        RbButton(
            label = "Listo",
            onClick = onListo,
            fillWidth = true,
        )
    }
}

/**
 * La tarjeta con la diferencia.
 *
 * Recibe datos y no el `ViewModel` a propósito: es la parte que hay que poder
 * medir al 200% de escala y leer palabra por palabra sin un server detrás.
 *
 * @param contadoDelServidor `closing_cash_counted`, en el texto decimal exacto
 *   que grabó el cierre.
 * @param esperadoDelServidor `closing_cash_expected`, ídem.
 * @param discrepanciaDelServidor `discrepancia` = contado − esperado, ídem.
 */
@Composable
internal fun TarjetaDeDiferencia(
    moneda: Moneda,
    contadoDelServidor: String?,
    esperadoDelServidor: String?,
    discrepanciaDelServidor: String?,
    notaDeCierre: String? = null,
) {
    val colors = RbTheme.colors
    val copy = copyDeDiferencia(
        moneda = moneda,
        contadoDelServidor = contadoDelServidor,
        esperadoDelServidor = esperadoDelServidor,
        discrepanciaDelServidor = discrepanciaDelServidor,
    )

    RbCard {
        // "Faltan $2.500" va en **una sola frase**, grande, y no como un título
        // con el monto repetido debajo. Dos veces el mismo número obliga a
        // mirarlo dos veces para confirmar que es el mismo, y en la pantalla más
        // delicada del día esa duda de medio segundo es exactamente lo que no
        // puede pasar. Además, así el lector de pantalla la anuncia entera en
        // vez de leer "Faltan" y después un número suelto.
        //
        // Ni un chip de estado ni un color de alarma: la palabra ya dice de qué
        // lado quedó, y un rojo convertiría el cierre en una acusación. El
        // producto necesita que la dueña cierre caja todos los días, no que le
        // tenga miedo a esta pantalla.
        Text(
            text = copy.titular,
            style = rbAmountStyle(RbAmountEmphasis.Headline),
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )

        Text(
            text = copy.explicacion,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )

        Text(
            text = copy.calma,
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        notaDeCierre?.takeIf { it.isNotBlank() }?.let { nota ->
            Text(
                text = "Anotaste: «$nota»",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}
