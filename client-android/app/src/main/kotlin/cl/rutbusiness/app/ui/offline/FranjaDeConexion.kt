package cl.rutbusiness.app.ui.offline

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbPolite
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * La franja de arriba: sin conexión, y cuántas ventas están esperando.
 *
 * **Nunca tapa la pantalla.** El encargo lo pide y además es lo correcto: la
 * dueña sin señal sigue necesitando mirar lo que ya cargó, y un modal encima de
 * eso convierte "no hay internet" en "no puedo hacer nada". Son dos renglones
 * arriba, y desaparecen solos cuando no hay nada que decir.
 *
 * Cuando hay ventas esperando **se puede tocar**, y ahí mide 56 dp de alto como
 * cualquier cosa tocable de esta app. Cuando el único mensaje es "sin conexión"
 * no hay nada que tocar y no paga esa altura. El alto depende del estado a
 * propósito: no hay a dónde ir, así que no hay blanco al que apuntar.
 */
@Composable
fun FranjaDeConexion(
    conectado: Boolean,
    cola: List<VentaEnCola>,
    onVerCola: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val esperando = cola.count { it.esperando }
    val rechazadas = cola.count { it.rechazada }
    if (conectado && esperando == 0 && rechazadas == 0) return

    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val hayCola = esperando > 0 || rechazadas > 0

    // Rojo sólo cuando el sistema rechazó una venta, porque eso pide una
    // decisión de la dueña. El resto —sin señal, ventas en camino— es molesto,
    // no roto, y va en el tono de marca. Verde no existe acá: la franja que no
    // molesta es la que no está.
    val hayProblema = rechazadas > 0
    val fondo = if (hayProblema) colors.dangerContainer else colors.brandContainer
    val filo = if (hayProblema) colors.dangerText else colors.brandText

    val titulo = when {
        !conectado && esperando > 0 -> "Sin conexión · ${ventas(esperando)} esperando"
        !conectado -> "Sin conexión"
        rechazadas > 0 && esperando == 0 -> "${ventas(rechazadas)} no ${if (rechazadas == 1) "se pudo" else "se pudieron"} enviar"
        esperando > 0 -> "Enviando ${ventas(esperando)}..."
        else -> "${ventas(rechazadas)} no ${if (rechazadas == 1) "se pudo" else "se pudieron"} enviar"
    }

    // Corto a propósito. Cada renglón que se lleva la franja se lo saca a la
    // lista de productos que hay abajo, y en un panel de 640dp con la barra de
    // arriba, el buscador y la barra de total, la lista se queda sin lugar
    // antes que nadie. Se midió en un emulador API 23: con el texto largo, la
    // lista quedaba en cero.
    val detalle = when {
        rechazadas > 0 -> "Tócalo para ver cuáles."
        !conectado && esperando > 0 -> "Salen solas al volver la señal. Tócalo para verlas."
        !conectado -> "Ves lo último que se cargó."
        else -> "Tócalo para verlas."
    }

    Column(modifier = modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(fondo)
                .then(
                    if (hayCola) {
                        Modifier
                            .rbClickable(
                                onClick = onVerCola,
                                onClickLabel = "Ver las ventas que están esperando",
                                shape = RectangleShape,
                            )
                            // Sólo el alto mínimo: el ancho ya lo da
                            // `fillMaxWidth`, y el mínimo de 56 dp que pone el
                            // default pelearía con él en una pantalla angosta.
                            .rbTouchTarget(minWidth = 0.dp)
                    } else {
                        Modifier
                    },
                )
                .padding(horizontal = dimens.space3, vertical = dimens.space1)
                // Se anuncia con calma y no como alerta: es información de
                // estado, y TalkBack interrumpiendo a media venta sería peor
                // que el problema del que avisa.
                .rbPolite(),
            verticalArrangement = Arrangement.spacedBy(dimens.space1),
        ) {
            Text(
                text = titulo,
                style = RbTheme.typography.bodyStrong,
                color = colors.textPrimary,
            )
            Text(
                text = detalle,
                style = RbTheme.typography.support,
                color = colors.textPrimary,
            )
        }

        // El filo de abajo separa la franja del contenido sin sumarle altura a
        // la franja, y lleva el color del estado: es lo que se ve de reojo.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(dimens.border)
                .background(filo),
        )
    }
}

/** "1 venta" / "3 ventas". El singular importa: lo lee una persona. */
private fun ventas(cuantas: Int): String =
    if (cuantas == 1) "1 venta" else "$cuantas ventas"
