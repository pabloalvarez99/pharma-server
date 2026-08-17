package cl.rutbusiness.app.ui.nav

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.max
import cl.rutbusiness.ui.icons.RbIcons
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * Los lugares a los que se puede ir estando adentro.
 *
 * Tres y no más. Cada destino que se agrega angosta a todos los demás, y el
 * peor caso de esta barra —tres etiquetas completas, con la letra al 200%, en
 * un panel de 360dp— ya es el límite de lo que entra sin cortar una palabra.
 *
 * El catálogo **no** es un destino: buscar un producto vive dentro de Cobrar,
 * que es donde la cajera lo necesita. Una pestaña de catálogo suelta sería una
 * segunda lista de productos que hace lo mismo con un toque más.
 *
 * **El orden ya es el del día de feria** y no hace falta reordenar por pack:
 * Cobrar ("Vender") queda antes que Resumen ("Hoy") en la declaración de
 * abajo, así que la fila de [BarraDeNavegacion] —que recorre [entries] tal
 * cual— siempre pone "vender" antes que "el día". "Quién me debe" no tiene
 * pestaña propia: vive dentro de Resumen/Hoy, y ahí también es el primer
 * bloque después de las ventas ([ordenDeBloquesHoy][cl.rutbusiness.app.ui.resumen]).
 * Agente/Hablar queda primero porque así lo pide el founder: es la puerta de
 * entrada, no el último recurso. Los tres destinos existen en los dos packs
 * -feria no esconde ninguno- así que tampoco hay una lista más corta que
 * armar para ese pack.
 */
enum class Destino {
    /**
     * El default. Directiva del founder: la dueña no navega menús, le habla al
     * negocio. Todo lo demás existe para cuando hablar no alcanza.
     */
    Agente,

    /** La pantalla que la cajera usa doscientas veces al día. */
    Cobrar,

    /**
     * "¿Cuánto vendí hoy?", la pregunta de todos los días.
     *
     * La etiqueta dice "Tu día" y no "Resumen" porque es el título que la
     * dueña ve al llegar. La pestaña decía una palabra y la pantalla otra, y
     * entonces buscar "Tu día" en la barra no encontraba nada. De paso entra
     * mejor: es una palabra más corta en el peor caso de la barra, que es
     * tres etiquetas completas al 200% en 360dp.
     */
    Resumen,
    ;

    /** Etiqueta de retail formal (pack sin `agent_home`), la de siempre. */
    val etiqueta: String get() = etiquetaPara(agentHome = false)

    /**
     * Etiqueta según pack (ADR-0022), delegada a `CopyNavegacion` para que el
     * texto se pueda fijar con un test JVM puro y no quede hardcodeado acá.
     * Nunca "Caja": la caja del día vive dentro de Hoy, no en la barra.
     */
    fun etiquetaPara(agentHome: Boolean): String = when (this) {
        Agente -> etiquetaAgente(agentHome)
        Cobrar -> etiquetaCobrar(agentHome)
        Resumen -> etiquetaResumen(agentHome)
    }
}

/**
 * La barra de abajo.
 *
 * Reglas que no son estéticas sino de piso de hardware, y por qué:
 *
 * - **El ícono nunca va solo.** Un pictograma sin palabra es una adivinanza, y
 *   el usuario objetivo es una persona mayor que no creció con esta gramática.
 *   La etiqueta está siempre, en las tres pestañas, elegida o no.
 * - **Nada distingue sólo por color.** La pestaña activa se marca por tres vías
 *   a la vez: pastilla de marca, ícono relleno y negrita. Quien no distingue el
 *   verde igual ve cuál está elegida.
 * - **La barra no tiene alto fijo.** Al 200% la etiqueta ocupa dos líneas y la
 *   barra crece; 56dp es el piso, no la medida.
 * - Los íconos (`RbIcons`, `:design`) se miden contra la tipografía y no en
 *   `dp` fijo, así que **crecen con la escala de letra** como el resto. Un
 *   vector medido en `dp` se quedaría chico justo para quien subió la letra
 *   porque ve poco.
 *
 * Está en `app/` y no en `:design` a propósito: la barra conoce los tres
 * destinos de **este** producto, y el design system no debería.
 */
@Composable
fun BarraDeNavegacion(
    actual: Destino,
    onElegir: (Destino) -> Unit,
    modifier: Modifier = Modifier,
    /** Pack feria: copy "Hablar" / "Vender" / "Hoy" (ADR-0022). */
    agentHome: Boolean = false,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .fillMaxWidth()
            // surfaceRaised: la barra se lee como el piso del puesto, no se
            // funde con el canvas de la pantalla de arriba.
            .background(colors.surfaceRaised)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            ),
    ) {
        // Hairline de cierre (misma idea que `RbTopBar`): sin ella, en tema
        // claro la barra se pega al contenido.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline),
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
        ) {
            Destino.entries.forEach { destino ->
                Pestana(
                    destino = destino,
                    etiqueta = destino.etiquetaPara(agentHome),
                    elegida = destino == actual,
                    onClick = { onElegir(destino) },
                    // `weight` reparte el ancho en partes iguales: la pestaña
                    // elegida no puede ser más ancha, o la barra se movería
                    // debajo del dedo cada vez que se cambia de pantalla.
                    modifier = Modifier.weight(1f),
                )
            }
        }

        // Un respiro bajo las etiquetas para que la última línea no quede
        // pegada al borde inferior del aparato.
        Box(modifier = Modifier.height(dimens.space2))
    }
}

@Composable
private fun Pestana(
    destino: Destino,
    etiqueta: String,
    elegida: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    val color = if (elegida) colors.brandText else colors.textSecondary

    // El ícono se mide contra el cuerpo de texto y nunca baja de iconSize:
    // así sube con la escala de letra y no se vuelve un puntito al sol.
    val tamanoIcono: Dp = with(LocalDensity.current) {
        max(RbTheme.typography.body.fontSize.toDp(), dimens.iconSize)
    }

    Column(
        modifier = modifier
            // `selected` va antes del clickable para que TalkBack anuncie
            // "pestaña, seleccionada" y no sólo "botón".
            .semantics { selected = elegida }
            .rbClickable(
                onClick = onClick,
                role = Role.Tab,
                shape = RbTheme.shapes.field,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space1, vertical = dimens.space1),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(dimens.space1, Alignment.CenterVertically),
    ) {
        // Indicador de marca arriba de la pastilla: no mueve el layout porque
        // el no-elegido también reserva la franja (transparente).
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(3.dp)
                .padding(horizontal = dimens.space3)
                .clip(RbTheme.shapes.pill)
                .background(if (elegida) colors.brandFill else Color.Transparent),
        )

        Box(
            modifier = Modifier
                .clip(RbTheme.shapes.pill)
                // El fondo cambia; el padding no. Si sólo la elegida tuviera
                // relleno, la fila entera saltaría al cambiar de pestaña.
                .background(if (elegida) colors.brandContainer else Color.Transparent)
                .padding(horizontal = dimens.space2, vertical = dimens.space1),
            contentAlignment = Alignment.Center,
        ) {
            IconoDestino(
                destino = destino,
                color = color,
                tamano = tamanoIcono,
                relleno = elegida,
            )
        }

        Text(
            text = etiqueta,
            style = RbTheme.typography.label,
            color = color,
            textAlign = TextAlign.Center,
            // Sin `maxLines` a propósito: al 200% una etiqueta de dos palabras
            // no entra en un tercio de 360dp y tiene que poder envolver. Cortar
            // la palabra sería dejar la pestaña sin nombre justo para quien más
            // lo lee.
            fontWeight = if (elegida) FontWeight.Bold else FontWeight.Medium,
        )
    }
}

/**
 * Los tres pictogramas de la barra, tomados de `RbIcons` (`:design`) en vez de
 * dibujados a mano acá.
 *
 * La razón detrás del `Canvas` original seguía siendo válida —
 * `material-icons-extended` pesa megas en un aparato barato para tres
 * glifos— pero la conclusión no: la salida es un set propio de `ImageVector`,
 * que pesa kilobytes de bytecode y nada en tiempo de ejecución, no dibujar
 * primitivas por pantalla. `RbIcons` es ese set.
 *
 * Decorativos para el lector de pantalla: el nombre ya lo dice la etiqueta de
 * abajo, y anunciarlo dos veces es ruido.
 *
 * [relleno]: la pestaña elegida va llena (no sólo contorno) para que se vea
 * sin depender del color de marca — las tres pestañas tienen las dos
 * variantes en `RbIcons`, Resumen incluido (el `Canvas` original la dejaba
 * siempre sólida, rompiendo esta regla para esa pestaña sola).
 */
@Composable
private fun IconoDestino(
    destino: Destino,
    color: Color,
    tamano: Dp,
    relleno: Boolean,
) {
    Icon(
        imageVector = destino.imageVector(relleno),
        contentDescription = null,
        tint = color,
        modifier = Modifier
            .size(tamano)
            .clearAndSetSemantics { },
    )
}

/** El vector de `RbIcons` para cada destino, en su variante de contorno o de
 *  relleno según si la pestaña está elegida. */
private fun Destino.imageVector(relleno: Boolean): ImageVector = when (this) {
    Destino.Agente -> if (relleno) RbIcons.agenteRelleno else RbIcons.agenteContorno
    Destino.Cobrar -> if (relleno) RbIcons.cobrarRelleno else RbIcons.cobrarContorno
    Destino.Resumen -> if (relleno) RbIcons.resumenRelleno else RbIcons.resumenContorno
}
