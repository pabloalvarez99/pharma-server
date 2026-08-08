package cl.rutbusiness.app.ui.offline

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.coroutines.launch
// EstadoRespaldoUi vive en este mismo package.

/**
 * Las ventas que todavía no llegaron al sistema del negocio.
 *
 * **Esta pantalla es la mitad del encargo.** Una cola que anda sola pero no se
 * ve no sirve de nada: la dueña que cobró seis ventas sin señal necesita poder
 * mirar el teléfono y contar seis, o no va a creerle a la app — y con razón.
 *
 * Lo que **no** se muestra acá es plata. Ni el total de cada venta ni la suma
 * de la cola: esos números los pone el server cuando la venta llegue, y un
 * total calculado en el teléfono al lado de una venta que todavía no se cobró
 * en el sistema sería exactamente el número inventado que este producto no
 * muestra. Se dicen los productos y la hora, que es lo que la dueña usa para
 * reconocer cuál venta es cuál.
 */
/**
 * Estado del último "Preparar respaldo" (ADR-0022).
 * Solo UI: el core no guarda esto.
 */
data class EstadoRespaldoUi(
    val mensaje: String,
    val bytes: Int,
    val ventas: Int,
)

@Composable
fun PantallaDeCola(
    cola: List<VentaEnCola>,
    conectado: Boolean,
    ahora: Long,
    onIntentarAhora: suspend () -> Unit,
    onDescartar: suspend (String) -> Unit,
    onCerrar: () -> Unit,
    modifier: Modifier = Modifier,
    /** Feria / ADR-0022: armar snapshot de la cola sin subir plaintext. */
    onPrepararRespaldo: (() -> Unit)? = null,
    estadoRespaldo: EstadoRespaldoUi? = null,
) {
    val dimens = RbTheme.dimens
    val alcance = rememberCoroutineScope()
    val esperando = cola.count { it.esperando }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Ventas por enviar",
            subtitle = when {
                cola.isEmpty() -> null
                esperando > 0 && conectado -> "Se están mandando solas."
                esperando > 0 -> "Salen apenas vuelva la señal."
                else -> "Ninguna está en camino."
            },
            onBack = onCerrar,
        )

        if (cola.isEmpty() && onPrepararRespaldo == null) {
            RbEmptyState(
                title = "No hay ninguna esperando",
                hint = "Todas las ventas que cobraste ya llegaron al sistema del negocio.",
                actionLabel = "Volver",
                onAction = onCerrar,
            )
            return@Column
        }

        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            if (onPrepararRespaldo != null) {
                item {
                    RbCard(title = "Respaldo del día") {
                        Text(
                            text = "Arma un paquete con las ventas de este " +
                                "teléfono. Se cifra con tu llave del cuaderno " +
                                "antes de subir (cuando el cifrado esté listo).",
                            style = RbTheme.typography.body,
                            color = RbTheme.colors.textSecondary,
                        )
                        RbButton(
                            label = "Preparar respaldo",
                            onClick = onPrepararRespaldo,
                            variant = RbButtonVariant.Secondary,
                            fillWidth = true,
                            modifier = Modifier.padding(top = dimens.space2),
                        )
                        estadoRespaldo?.let { est ->
                            Text(
                                text = est.mensaje,
                                style = RbTheme.typography.support,
                                color = RbTheme.colors.textPrimary,
                                modifier = Modifier.padding(top = dimens.space2),
                            )
                            if (est.ventas > 0) {
                                Text(
                                    text = "${est.ventas} venta(s) · ${est.bytes} bytes",
                                    style = RbTheme.typography.label,
                                    color = RbTheme.colors.textSecondary,
                                )
                            }
                        }
                    }
                }
            }

            if (cola.isEmpty()) {
                item {
                    RbEmptyState(
                        title = "No hay ninguna esperando",
                        hint = "Todas las ventas que cobraste ya llegaron al sistema del negocio.",
                        actionLabel = "Volver",
                        onAction = onCerrar,
                    )
                }
                return@LazyColumn
            }

            if (esperando > 0) {
                item {
                    RbButton(
                        label = "Intentar ahora",
                        onClick = { alcance.launch { onIntentarAhora() } },
                        // Sin señal el botón no promete nada que pueda cumplir.
                        // Dejarlo tocable para que falle enseguida sería el
                        // dead-end que este producto no tiene.
                        enabled = conectado,
                        fillWidth = true,
                    )
                }
            }

            items(cola, key = { it.clave }) { venta ->
                FilaDeVentaEnCola(
                    venta = venta,
                    ahora = ahora,
                    onDescartar = { alcance.launch { onDescartar(venta.clave) } },
                )
            }
        }
    }
}

@Composable
private fun FilaDeVentaEnCola(
    venta: VentaEnCola,
    ahora: Long,
    onDescartar: () -> Unit,
) {
    val colors = RbTheme.colors

    RbCard(
        title = tituloDeVenta(venta.cobradaEn, ahora),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = detalleDeLineas(venta),
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )

        if (venta.rechazada) {
            Text(
                text = venta.motivo ?: "El sistema no la aceptó.",
                style = RbTheme.typography.body,
                color = colors.dangerText,
            )
            Text(
                text = "No se va a reintentar sola. Revisa qué pasó y vuelve a cobrarla si " +
                    "corresponde; recién ahí descártala de acá.",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
            RbButton(
                label = "Descartar",
                onClick = onDescartar,
                variant = RbButtonVariant.Destructive,
            )
        } else {
            Text(
                text = estadoDeEspera(venta, ahora),
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}

/**
 * Cómo se nombra cada venta de la lista.
 *
 * Cuánto hace, no a qué hora: `commonMain` no tiene husos horarios, y una hora
 * de reloj en UTC delante de alguien en Chile diría las 17:32 de una venta de
 * las 13:32. "Hace 20 minutos" no necesita huso y además es lo que la dueña usa
 * para reconocer cuál venta es cuál.
 *
 * El caso de recién se dice aparte porque "Venta de recién" suena a error de
 * tipeo. Es la venta que más se va a mirar -la que se acaba de cobrar- así que
 * no puede ser la que está mal escrita.
 */
private fun tituloDeVenta(cobradaEn: Long, ahora: Long): String {
    val antiguedad = Fechado(Unit, cobradaEn).antiguedad(ahora)
    return if (antiguedad == "recién") "Venta recién cobrada" else "Venta de $antiguedad"
}

/** "3 productos" — sin plata, que la pone el server cuando la venta llegue. */
private fun detalleDeLineas(venta: VentaEnCola): String {
    val unidades = venta.solicitud.items.sumOf { it.quantity }
    val productos = if (venta.lineas == 1) "1 producto" else "${venta.lineas} productos"
    val piezas = if (unidades == 1) "1 unidad" else "$unidades unidades"
    return "$productos · $piezas · el total lo confirma el sistema al recibirla"
}

private fun estadoDeEspera(venta: VentaEnCola, ahora: Long): String = when {
    venta.intentos == 0 -> "Esperando salir."
    venta.proximoIntentoEn > ahora -> {
        val segundos = ((venta.proximoIntentoEn - ahora) / 1000L).coerceAtLeast(1L)
        if (segundos < 60L) {
            "No se pudo mandar. Vuelve a intentar en $segundos segundos."
        } else {
            "No se pudo mandar. Vuelve a intentar en ${segundos / 60L} minutos."
        }
    }
    else -> "Intentando mandarla."
}
