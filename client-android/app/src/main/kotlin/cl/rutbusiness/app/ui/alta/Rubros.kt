package cl.rutbusiness.app.ui.alta

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * Los nueve rubros del catálogo, copiados del server.
 *
 * **Fuente de verdad: `crates/domain/src/rubro.rs`, la tabla `PACKS`.** De ahí
 * salen la clave, la etiqueta y la frase de cada uno, en ese orden. La clave es
 * lo único que viaja: es la que `domain::provisioning::validate_vertical`
 * acepta, y la que después decide qué módulos prende el pack del rubro
 * (`GET /api/v1/rubro-pack`). Mandar una clave que no esté en esta lista es un
 * 400 del server, no un rubro nuevo.
 *
 * Ojo con `docs/strategy/rubro-catalog.md`: la tabla de ese documento dice que
 * las claves internas son en inglés (`pharmacy`, `restaurant`…). Eso quedó
 * viejo — el código dice `farmacia`, y el código es el que valida. La prueba
 * `validate_vertical("farmacia")` del crate lo fija.
 *
 * **Por qué está duplicado acá.** El alta ocurre *antes* de tener sesión, y el
 * único endpoint que sirve el catálogo —`GET /api/v1/rubro-pack`— pide JWT y
 * además devuelve un solo pack, el del tenant. No hay forma de pedirle la lista
 * al server sin haber entrado, así que la lista viaja en la app. Es una copia y
 * como toda copia se puede desincronizar: `RubrosTest` compara esta lista
 * contra `rubro.rs` leyéndolo del repo, así que si alguien agrega un rubro en
 * el server y no acá, el build de la app se cae.
 */
data class Rubro(
    /** Lo que se manda como `vertical`. Igual a `RubroPack.rubro` en el server. */
    val clave: String,
    /** Lo que lee la dueña. Igual a `RubroPack.label`. */
    val etiqueta: String,
    /** Una línea que ayuda a reconocerse. Igual a `RubroPack.tagline`. */
    val frase: String,
)

/**
 * En el mismo orden que `PACKS`. `feria` va primero porque es el foco activo
 * del producto (ADR-0022) y porque el primer rubro de la lista es el que se lee
 * sin desplazar en un teléfono chico. `otro` va al final y no es el elegido por
 * defecto: el alta obliga a tocar uno, porque un rubro elegido de verdad
 * configura la app y un rubro puesto por descarte no configura nada.
 */
val RUBROS: List<Rubro> = listOf(
    Rubro("feria", "Feria / Calle", "Tu puesto, sin cuaderno."),
    Rubro("farmacia", "Farmacia", "Tu farmacia, en regla."),
    Rubro("minimarket", "Minimarket / Almacén", "Tu almacén, al día."),
    Rubro("restaurant", "Restaurant / Comida", "Tu cocina, bajo control."),
    Rubro("cafe", "Café / Pastelería", "Tu café, listo cada mañana."),
    Rubro("tienda", "Tienda / Retail", "Tu tienda, ordenada."),
    Rubro("belleza", "Belleza / Estética", "Tu salón, agendado."),
    Rubro("servicios", "Servicios / Oficios", "Tu oficio, facturado."),
    Rubro("otro", "Otro", "Tu negocio, a tu manera."),
)

/** Clave del rubro que se siente natural al abrir el alta (ADR-0022). */
const val RUBRO_NATURAL: String = "feria"

/** Chip de la tarjeta libre de feria: se anuncia sin forzar la elección. */
const val CHIP_RUBRO_NATURAL: String = "El de la feria"

/** Chip cuando la dueña ya tocó un cartel. */
const val CHIP_RUBRO_ELEGIDO: String = "Elegido"

/** Texto de aire encima de los carteles. */
const val AYUDA_DE_RUBROS: String =
    "La feria va primero. Toca el cartel que más se parezca a lo tuyo: con eso " +
        "la app se acomoda — qué te pide y qué te muestra."

/** El rubro del puesto de calle: primero en la lista y en la vista. */
fun Rubro.esNatural(): Boolean = clave == RUBRO_NATURAL

/**
 * Etiqueta del chip cuando el cartel no está elegido.
 *
 * Sólo feria trae una: se lee como invitación, no como default marcado. El resto
 * no lleva chip suelto — un menú de nueve pastillas se siente a combo.
 */
fun Rubro.chipCuandoLibre(): String? =
    if (esNatural()) CHIP_RUBRO_NATURAL else null

/** Etiqueta del chip cuando el cartel está elegido. */
fun Rubro.chipCuandoElegido(): String = CHIP_RUBRO_ELEGIDO

/**
 * Los rubros como carteles apilados, no como filas de lista ni como combo.
 *
 * Cada opción es un cartel con borde grueso y piso táctil de 56dp: se lee de
 * lejos bajo el sol, se toca con el pulgar y no se confunde con un menú denso.
 * Feria va arriba y se anuncia con su chip; no viene pre-elegida.
 */
@Composable
fun CartelesDeRubro(
    elegido: Rubro?,
    onElegir: (Rubro) -> Unit,
    habilitado: Boolean,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        RUBROS.forEach { rubro ->
            CartelDeRubro(
                rubro = rubro,
                elegido = rubro.clave == elegido?.clave,
                habilitado = habilitado,
                onClick = { onElegir(rubro) },
            )
        }
    }
}

/**
 * Un solo cartel de rubro.
 *
 * Feria (el natural) y el elegido se pintan con más peso visual — borde brand y
 * superficie de marca — para que el ojo caiga ahí sin que nadie elija por
 * descarte.
 */
@Composable
internal fun CartelDeRubro(
    rubro: Rubro,
    elegido: Boolean,
    habilitado: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    val natural = rubro.esNatural()

    val fondo = when {
        elegido -> colors.brandContainer
        natural -> colors.surfaceRaised
        else -> colors.surface
    }
    val bordeColor = when {
        elegido || natural -> colors.brandText
        else -> colors.outlineStrong
    }
    val bordeAncho = when {
        elegido || natural -> dimens.focusRing
        else -> dimens.border
    }
    val chip = if (elegido) rubro.chipCuandoElegido() else rubro.chipCuandoLibre()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(fondo)
            .border(bordeAncho, bordeColor, shape)
            .rbClickable(
                onClick = onClick,
                enabled = habilitado,
                role = Role.Button,
                onClickLabel = if (elegido) null else "Elegir ${rubro.etiqueta}",
                shape = shape,
            )
            .rbTouchTarget()
            .semantics { selected = elegido }
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(dimens.space2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = rubro.etiqueta,
                style = if (natural || elegido) {
                    RbTheme.typography.bodyStrong
                } else {
                    RbTheme.typography.body
                },
                color = colors.textPrimary,
                modifier = Modifier.weight(1f),
            )
            if (chip != null) {
                RbChip(
                    label = chip,
                    tone = RbChipTone.Brand,
                    selected = elegido,
                )
            }
        }
        Text(
            text = rubro.frase,
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )
    }
}
