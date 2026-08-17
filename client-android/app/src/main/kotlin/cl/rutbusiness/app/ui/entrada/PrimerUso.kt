package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Las tres pantallas que se leen antes de la primera vez.
 *
 * **Por qué existe.** La app abría en un formulario que pedía la dirección de un
 * servidor. Alguien que la bajó de Play Store no tiene forma de saber qué
 * escribir ahí, y un campo que no se sabe llenar no es una pantalla difícil: es
 * una app que se desinstala en treinta segundos. Estas tres pantallas contestan
 * las tres preguntas que esa persona tiene, en el orden en que se las hace: qué
 * es esto, dónde se guarda, y qué necesitás a mano.
 *
 * Se siente un **cartel del puesto**, no un tutorial: una idea grande por paso,
 * superficie elevada, mucho aire. Un botón obvio por pantalla; saltar siempre
 * a la vista, del mismo tamaño táctil, pero secundario. Los botones no
 * scrollean. Se muestra una sola vez: la bandera se prende con el primer login
 * que funciona. Ver [PreferenciasDeEntrada].
 *
 * El copy vive en [pasosDelPrimerUso] / [ctaPrimarioPrimerUso] (paquete
 * `CopyPrimerUso`): feria/nube habla de puesto; on-prem puede nombrar el
 * computador del negocio.
 */
@Composable
fun PrimerUso(onListo: () -> Unit, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    var indice by rememberSaveable { mutableStateOf(0) }

    // La misma fuente de verdad que gobierna el botón de la pantalla de
    // entrada. Sin servicios provistos —una prueba de otra pantalla— no hay
    // Google y el texto no lo finge. `nube` del APK feria/nube es la voz del
    // puesto (sin CompositionLocal extra).
    val servicios = LocalEntrada.current
    val nube = !servicios?.nube.isNullOrBlank()
    val pasos = pasosDelPrimerUso(
        googleDisponible = servicios?.identidadGoogle?.disponible() == true,
        nube = nube,
    )

    val paso = pasos[indice]
    val ultimo = indice == pasos.lastIndex
    val colors = RbTheme.colors
    val shape = RbTheme.shapes.card
    val ctaPrimario = ctaPrimarioPrimerUso(ultimo = ultimo, nube = nube)
    val ctaSaltar = ctaSaltarPrimerUso(nube = nube)

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = paso.titulo,
            subtitle = "Paso ${indice + 1} de ${pasos.size}",
            onBack = if (indice > 0) ({ indice -= 1 }) else null,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            // Cartel del paso: se lee de lejos, como el camino primario de la puerta.
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(colors.surfaceRaised)
                    .border(dimens.focusRing, colors.outlineStrong, shape)
                    .padding(horizontal = dimens.space3, vertical = dimens.space4),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Text(
                    text = paso.encabezado,
                    style = RbTheme.typography.title,
                    color = colors.textPrimary,
                    modifier = Modifier.rbHeading(),
                )

                paso.parrafos.forEach { parrafo ->
                    Text(
                        text = parrafo,
                        style = RbTheme.typography.body,
                        color = colors.textPrimary,
                    )
                }

                if (paso.lista.isNotEmpty()) {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(dimens.space2),
                    ) {
                        paso.lista.forEachIndexed { numero, linea ->
                            LineaNumerada(numero = numero + 1, texto = linea)
                        }
                    }
                }

                paso.remate?.let { remate ->
                    Text(
                        text = remate,
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                }
            }
        }

        // La misma hairline que cierra el `RbTopBar`, arriba en vez de abajo.
        // No es decoración: al 200% el texto no entra y se corta justo debajo de
        // este borde. Sin la línea, la frase cortada se lee como una frase que
        // termina mal; con ella se lee como texto que sigue detrás del panel.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(RbTheme.colors.outline),
        )

        // Pegados abajo, siempre a la vista. Mismo criterio que el panel del
        // escáner: cuando hay más que decir, lo que se achica es el texto, no la
        // fila que el dedo va a tocar.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(RbTheme.colors.surfaceRaised)
                .windowInsetsPadding(
                    WindowInsets.safeDrawing.only(
                        WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                    ),
                )
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            RbButton(
                label = ctaPrimario,
                onClick = { if (ultimo) onListo() else indice += 1 },
                fillWidth = true,
            )
            RbButton(
                label = ctaSaltar,
                onClick = onListo,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/**
 * Una línea de la lista del último paso.
 *
 * El número va en una columna aparte y no pegado al texto con un punto, para
 * que al 200% de escala la segunda línea de cada ítem quede alineada bajo la
 * primera en vez de meterse debajo del número.
 */
@Composable
private fun LineaNumerada(numero: Int, texto: String) {
    val dimens = RbTheme.dimens

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(dimens.space2),
        verticalAlignment = Alignment.Top,
    ) {
        Text(
            text = "$numero.",
            style = RbTheme.typography.bodyStrong,
            color = RbTheme.colors.brandText,
        )
        Text(
            text = texto,
            style = RbTheme.typography.body,
            color = RbTheme.colors.textPrimary,
        )
    }
}
