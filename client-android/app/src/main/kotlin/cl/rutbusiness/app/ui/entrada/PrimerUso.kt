package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
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
 * es esto, por qué me pide una dirección, y de dónde saco los datos.
 *
 * Reglas de esta parte, todas del piso de hardware y del encargo:
 *
 * - **Un botón obvio por pantalla.** El de avanzar es el único primario. Saltar
 *   está siempre, del mismo tamaño táctil, pero secundario.
 * - **Cero jerga.** Ni "endpoint", ni "API", ni "instancia", ni "tenant". La
 *   palabra "servidor" aparece una vez y se explica, porque es la que le va a
 *   decir quien le instaló el sistema — no enseñarla la dejaría sin entender a
 *   la única persona que puede ayudarla.
 * - **Los botones no scrollean.** El texto sí. Al 200% de escala el cuerpo de
 *   estas pantallas no entra, y un botón que hay que ir a buscar scrolleando es
 *   un botón que la persona mayor no encuentra.
 *
 * Se muestra una sola vez: la bandera se prende con el primer login que
 * funciona, no al terminar de leer. Ver [PreferenciasDeEntrada].
 */
@Composable
fun PrimerUso(onListo: () -> Unit, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    var indice by rememberSaveable { mutableStateOf(0) }

    // La misma fuente de verdad que gobierna el botón de la pantalla de
    // entrada. Sin servicios provistos —una prueba de otra pantalla— no hay
    // Google y el texto no lo finge.
    val servicios = LocalEntrada.current
    val pasos = pasosDelPrimerUso(
        googleDisponible = servicios?.identidadGoogle?.disponible() == true,
    )

    val paso = pasos[indice]
    val ultimo = indice == pasos.lastIndex

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
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(
                text = paso.encabezado,
                style = RbTheme.typography.title,
                color = RbTheme.colors.textPrimary,
                modifier = Modifier.rbHeading(),
            )

            paso.parrafos.forEach { parrafo ->
                Text(
                    text = parrafo,
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                )
            }

            paso.lista.forEachIndexed { numero, linea ->
                LineaNumerada(numero = numero + 1, texto = linea)
            }

            paso.remate?.let { remate ->
                Text(
                    text = remate,
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }
        }

        // La misma hairline que cierra el `RbTopBar`, arriba en vez de abajo.
        // No es decoración: al 200% el texto no entra y se corta justo debajo de
        // este borde. Sin la línea, la frase cortada se lee como una frase que
        // termina mal; con ella se lee como texto que sigue detrás del panel, y
        // eso es lo que hace que alguien piense en scrollear.
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
                label = if (ultimo) "Empezar" else "Siguiente",
                onClick = { if (ultimo) onListo() else indice += 1 },
                fillWidth = true,
            )
            RbButton(
                label = "Saltar la explicación",
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

/**
 * Lo que dice cada pantalla.
 *
 * Los textos viven en una lista de datos y no repartidos en tres composables
 * para que se puedan leer de corrido —que es como hay que revisarlos— y para
 * que una prueba pueda recorrerlos sin montar la pantalla tres veces.
 */
internal data class PasoDelPrimerUso(
    val titulo: String,
    val encabezado: String,
    val parrafos: List<String>,
    val lista: List<String> = emptyList(),
    val remate: String? = null,
)

/**
 * Los tres pasos, con el paso 3 escrito según lo que este build puede hacer.
 *
 * [googleDisponible] es el **mismo** `disponible()` que decide si aparece el
 * botón de Google en la pantalla de entrada, y eso es todo el punto: si el
 * texto tuviera su propia constante, un APK compilado con client id mostraría
 * el botón andando y, dos pantallas antes, "pronto". Dos verdades sobre lo
 * mismo, y la que la persona lee primero es la falsa.
 *
 * Cuando no hay client id el texto es **exactamente** el de siempre: prometer
 * "pronto" es honesto, porque el botón todavía no está.
 */
internal fun pasosDelPrimerUso(googleDisponible: Boolean): List<PasoDelPrimerUso> = listOf(
    PasoDelPrimerUso(
        titulo = "Esto es RutAgent",
        encabezado = "El cuaderno del negocio, en tu teléfono",
        parrafos = listOf(
            "Anotás una venta con la voz o el teclado, sabés quién te debe y " +
                "cuánto hiciste hoy - más rápido que el cuaderno de mil pesos.",
            "Hablale como a un empleado de confianza. Antes de guardar nada, " +
                "te lo muestra para que lo revises.",
        ),
    ),
    // El texto de este paso se recortó dos veces a propósito. En el aparato de
    // referencia la versión larga quedaba cortada por el panel de los botones, y
    // una frase que se corta a la mitad no la termina de leer nadie: se toca
    // «Siguiente» y la explicación se pierde justo donde importaba.
    PasoDelPrimerUso(
        titulo = "Dónde se guarda todo",
        encabezado = "En el teléfono y en un respaldo cifrado",
        parrafos = listOf(
            "Vendés aunque no haya señal: lo del día queda en el aparato.",
            "Si activás el respaldo, se sube cifrado con una llave tuya. " +
                "Nosotros no podemos leerla ni recuperarla: la escribís en el " +
                "cuaderno el primer día (tarjeta de rescate).",
        ),
        remate = "Sin esa llave, el respaldo no sirve. Con ella y tu cuenta, " +
            "volvés a entrar si se te rompe el teléfono.",
    ),
    PasoDelPrimerUso(
        titulo = "Lo que necesitas a mano",
        encabezado = "Para entrar la primera vez",
        parrafos = emptyList(),
        lista = listOf(
            "La dirección del computador del negocio (si te la dieron). " +
                "Se ve así: 192.168.1.10:8080",
            "El nombre corto de tu negocio.",
            if (googleDisponible) {
                "Tu cuenta de Google, o tu correo y tu clave."
            } else {
                "Tu correo y tu clave. Pronto también con tu cuenta de Google " +
                    "(sin otra contraseña que acordarte)."
            },
        ),
        remate = "¿No los tienes? Pídeselos a quien instaló el sistema. " +
            "Quedaron anotados el día que lo instaló.",
    ),
)
