package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Los dos caminos de entrada, uno al lado del otro.
 *
 * **Por qué existe esta pantalla.** Antes la app abría directo en un formulario
 * que pedía la dirección de un servidor. Quien la bajó de la tienda no tiene de
 * dónde sacar esa dirección: necesita que otra persona le instale un sistema y
 * se la dicte. Eso no es un ERP gratis para cualquier negocio, es un ERP para
 * quien conoce a alguien que sepa. Acá se agrega el camino que faltaba —crear
 * el negocio desde el teléfono— sin sacar el que ya había, porque quien tiene
 * su propio computador andando sigue teniendo que poder entrar.
 *
 * **Cuál de los dos es el botón grande** lo decide un solo dato: si este
 * teléfono ya entró alguna vez. Quien ya entró viene a entrar de nuevo, no a
 * crear un segundo negocio; quien nunca entró viene a empezar. Los dos botones
 * están siempre, del mismo tamaño táctil — lo único que cambia es cuál pesa
 * más, porque una pantalla con dos botones igual de importantes es una pantalla
 * sin respuesta.
 *
 * @param yaEntroAlgunaVez si este teléfono tuvo sesión antes.
 */
@Composable
internal fun Puerta(
    yaEntroAlgunaVez: Boolean,
    onCrear: () -> Unit,
    onEntrar: () -> Unit,
    onVerExplicacion: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Empecemos",
            subtitle = "Dos caminos: elige el tuyo",
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            RbCard(title = "Es la primera vez") {
                Text(
                    text = "Creas tu negocio acá mismo: le pones nombre, dices a qué se dedica, " +
                        "eliges tu correo y tu clave, y quedas adentro.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                )
                RbButton(
                    label = "Crear mi negocio",
                    onClick = onCrear,
                    variant = if (yaEntroAlgunaVez) {
                        RbButtonVariant.Secondary
                    } else {
                        RbButtonVariant.Primary
                    },
                    fillWidth = true,
                )
            }

            RbCard(title = "Ya tienes un negocio") {
                Text(
                    text = "Lo creaste antes, o alguien te lo instaló y te pasó los datos. " +
                        "Necesitas tu correo y tu clave.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                )
                RbButton(
                    label = "Entrar a mi negocio",
                    onClick = onEntrar,
                    variant = if (yaEntroAlgunaVez) {
                        RbButtonVariant.Primary
                    } else {
                        RbButtonVariant.Secondary
                    },
                    fillWidth = true,
                )
            }

            RbButton(
                label = "Ver la explicación de nuevo",
                onClick = onVerExplicacion,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}
