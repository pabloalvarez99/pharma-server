package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

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
 * **Cuál de los dos es el camino grande** lo decide un solo dato: si este
 * teléfono ya entró alguna vez. Quien ya entró viene a entrar de nuevo, no a
 * crear un segundo negocio; quien nunca entró viene a empezar. El camino
 * primario va arriba, con relleno de marca y más aire; el secundario va abajo,
 * más quieto. Misma altura táctil en los dos botones — lo que cambia es el peso
 * visual, porque dos botones iguales son una pantalla sin respuesta.
 *
 * **El tercer camino** (ADR-0023) no compite con los otros dos y por eso no es
 * una tarjeta: es la salida de quien perdió el teléfono. Va abajo, en letra
 * chica, porque el día que se necesita se busca — y arriba, compitiendo por
 * atención con los dos caminos normales, sólo le agregaría una decisión más a
 * quien viene a lo de siempre.
 *
 * @param yaEntroAlgunaVez si este teléfono tuvo sesión antes.
 * @param onRecuperar `null` cuando esta build no ofrece el rescate.
 */
@Composable
internal fun Puerta(
    yaEntroAlgunaVez: Boolean,
    onCrear: () -> Unit,
    onEntrar: () -> Unit,
    onVerExplicacion: () -> Unit,
    onRecuperar: (() -> Unit)? = null,
    /** APK de feria / URL_NUBE: no hay técnico ni dirección de computador. */
    nube: Boolean = false,
    /** Copy feria (ADR-0022): "puesto" en vez de "negocio". */
    esFeria: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val cosa = if (esFeria) "puesto" else "negocio"
    val crearEsPrimario = !yaEntroAlgunaVez

    val tituloBarra = if (esFeria) "Tu puesto" else "Tu negocio"
    val subtituloBarra = if (yaEntroAlgunaVez) {
        "Entrá de nuevo, o creá otro"
    } else {
        "Creá uno, o entrá al que ya tenés"
    }

    val cuerpoCrear = if (esFeria) {
        "Creas tu puesto acá mismo: le ponés nombre, correo y clave, y quedás adentro."
    } else {
        "Creas tu negocio acá mismo: le ponés nombre, decís a qué se dedica, " +
            "elegís correo y clave, y quedás adentro."
    }
    val cuerpoEntrar = if (nube) {
        "Lo creaste antes en este teléfono o en otro. " +
            "Entrá con el nombre corto, tu correo y tu clave."
    } else {
        "Lo creaste antes, o alguien te lo instaló y te pasó los datos. " +
            "Necesitás tu correo y tu clave."
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = tituloBarra,
            subtitle = subtituloBarra,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            // El camino que pesa más va primero: se ve sin scrollear y no compite.
            if (crearEsPrimario) {
                CaminoDePuerta(
                    titulo = "Es la primera vez",
                    cuerpo = cuerpoCrear,
                    etiqueta = "Crear mi $cosa",
                    onClick = onCrear,
                    primario = true,
                )
                CaminoDePuerta(
                    titulo = "Ya tenés un $cosa",
                    cuerpo = cuerpoEntrar,
                    etiqueta = "Entrar a mi $cosa",
                    onClick = onEntrar,
                    primario = false,
                )
            } else {
                CaminoDePuerta(
                    titulo = "Ya tenés un $cosa",
                    cuerpo = cuerpoEntrar,
                    etiqueta = "Entrar a mi $cosa",
                    onClick = onEntrar,
                    primario = true,
                )
                CaminoDePuerta(
                    titulo = "Es la primera vez",
                    cuerpo = cuerpoCrear,
                    etiqueta = "Crear mi $cosa",
                    onClick = onCrear,
                    primario = false,
                )
            }

            onRecuperar?.let { recuperar ->
                CaminoQuieto(
                    titulo = "Perdí mi teléfono",
                    cuerpo = "Si guardaste la tarjeta que la app te pidió anotar, " +
                        "tu $cosa vuelve con ella. No hace falta tu clave.",
                    etiqueta = "Recuperar con mi tarjeta",
                    onClick = recuperar,
                )
            }

            Spacer(Modifier.height(dimens.space2))

            RbButton(
                label = "Ver la explicación de nuevo",
                onClick = onVerExplicacion,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/**
 * Un camino con peso: primario (marca + más aire) o secundario (borde fino).
 *
 * Los dos botones miden ≥ 56 dp; lo que cambia es relleno, borde y color del
 * cuerpo, para que se distingan a un vistazo sin imágenes nuevas en el APK.
 */
@Composable
private fun CaminoDePuerta(
    titulo: String,
    cuerpo: String,
    etiqueta: String,
    onClick: () -> Unit,
    primario: Boolean,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(if (primario) colors.surfaceRaised else colors.surface)
            .border(
                width = if (primario) dimens.focusRing else dimens.border,
                color = if (primario) colors.outlineStrong else colors.outline,
                shape = shape,
            )
            .padding(
                horizontal = dimens.space3,
                vertical = if (primario) dimens.space4 else dimens.space3,
            ),
        verticalArrangement = Arrangement.spacedBy(
            if (primario) dimens.space3 else dimens.space2,
        ),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = cuerpo,
            style = if (primario) RbTheme.typography.body else RbTheme.typography.support,
            color = if (primario) colors.textPrimary else colors.textSecondary,
        )
        RbButton(
            label = etiqueta,
            onClick = onClick,
            variant = if (primario) {
                RbButtonVariant.Primary
            } else {
                RbButtonVariant.Secondary
            },
            fillWidth = true,
        )
    }
}

/**
 * Rescate y otros caminos de excepción: legible, sin pelear con crear/entrar.
 */
@Composable
private fun CaminoQuieto(
    titulo: String,
    cuerpo: String,
    etiqueta: String,
    onClick: () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.support,
            color = colors.textSecondary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = cuerpo,
            style = RbTheme.typography.support,
            color = colors.textTertiary,
        )
        RbButton(
            label = etiqueta,
            onClick = onClick,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}
