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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading
import kotlinx.coroutines.launch

/**
 * Las ventas que todavía no llegaron al sistema del negocio.
 *
 * **Esta pantalla es la mitad del encargo.** Una cola que anda sola pero no se
 * ve no sirve de nada: la dueña que cobró seis ventas sin señal necesita poder
 * mirar el teléfono y contar seis, o no va a creerle a la app — y con razón.
 *
 * Se lee como **cola de ventas esperando** (notas del día), no como un panel
 * de sync ni un error de red. Lo que **no** se muestra acá es plata. Ni el
 * total de cada venta ni la suma de la cola: esos números los pone el server
 * cuando la venta llegue. Se dicen los productos y la hora, que es lo que la
 * dueña usa para reconocer cuál venta es cuál.
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

/** Resultado de bajar un sobre cifrado del server (lab o bucket). */
sealed class ResultadoTraerNube {
    data class Ok(val sobreBase64: String, val backupId: String) : ResultadoTraerNube()
    data class Vacio(val mensaje: String) : ResultadoTraerNube()
    data class Error(val mensaje: String) : ResultadoTraerNube()
}

@Composable
fun PantallaDeCola(
    cola: List<VentaEnCola>,
    conectado: Boolean,
    ahora: Long,
    onIntentarAhora: suspend () -> Unit,
    onDescartar: suspend (String) -> Unit,
    onCerrar: () -> Unit,
    modifier: Modifier = Modifier,
    /**
     * Feria / ADR-0022: armar (y cifrar con la llave del cuaderno) el snapshot.
     * El texto es la frase o los bloques; vacío = solo empaquetar sin cifrar.
     */
    onPrepararRespaldo: ((materialCuaderno: String) -> Unit)? = null,
    /**
     * Restaurar local: material + sobre base64 (último preparado o pegado).
     */
    onRestaurarRespaldo: ((materialCuaderno: String, sobreBase64: String) -> Unit)? = null,
    /**
     * Lab / nube: listar y bajar ciphertext (nunca la frase).
     * El callback devuelve base64 del sobre o un mensaje de error en null path.
     */
    onTraerDeLaNube: (suspend () -> ResultadoTraerNube)? = null,
    /** Base64 del último sobre armado en este teléfono (prefill restore). */
    ultimoSobreBase64: String? = null,
    estadoRespaldo: EstadoRespaldoUi? = null,
    subiendoRespaldo: Boolean = false,
) {
    val dimens = RbTheme.dimens
    val alcance = rememberCoroutineScope()
    val feria = esFeria()
    val esperando = cola.count { it.esperando }
    var materialCuaderno by remember { mutableStateOf("") }
    var sobrePegado by remember(ultimoSobreBase64) {
        mutableStateOf(ultimoSobreBase64.orEmpty())
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Ventas esperando",
            subtitle = when {
                cola.isEmpty() -> null
                esperando > 0 && conectado -> "Se están mandando solas."
                esperando > 0 -> "Salen apenas vuelva la señal."
                else -> "Ninguna está en camino."
            },
            onBack = onCerrar,
        )

        if (cola.isEmpty() && onPrepararRespaldo == null && onRestaurarRespaldo == null) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(dimens.space3),
            ) {
                VacioDeCola(
                    hint = hintColaVacia(feria),
                    onVolver = onCerrar,
                )
            }
            return@Column
        }

        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(dimens.space3),
            // space4: se leen de a una nota, no como filas de un panel de sync.
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            if (onPrepararRespaldo != null) {
                item {
                    RbCard(title = "Respaldo del día") {
                        Text(
                            text = "Arma un paquete con las ventas de este " +
                                "teléfono. Con las palabras de tu tarjeta se " +
                                "cifra en el aparato; nosotros no las vemos.",
                            style = RbTheme.typography.body,
                            color = RbTheme.colors.textSecondary,
                        )
                        RbTextField(
                            value = materialCuaderno,
                            onValueChange = { materialCuaderno = it },
                            label = "Llave del cuaderno",
                            placeholder = "12 palabras o 5 bloques",
                            imeAction = ImeAction.Done,
                            modifier = Modifier.padding(top = dimens.space2),
                        )
                        RbButton(
                            label = if (subiendoRespaldo) {
                                "Cifrando / enviando..."
                            } else {
                                "Preparar respaldo"
                            },
                            onClick = { onPrepararRespaldo(materialCuaderno) },
                            variant = RbButtonVariant.Secondary,
                            fillWidth = true,
                            enabled = !subiendoRespaldo,
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

            if (onRestaurarRespaldo != null) {
                item {
                    RbCard(title = "Abrir un respaldo") {
                        Text(
                            text = "Si tenés el paquete cifrado y la llave del " +
                                "cuaderno, se abre acá en el teléfono. " +
                                "Nadie en la nube puede hacerlo por vos.",
                            style = RbTheme.typography.body,
                            color = RbTheme.colors.textSecondary,
                        )
                        RbTextField(
                            value = materialCuaderno,
                            onValueChange = { materialCuaderno = it },
                            label = "Llave del cuaderno",
                            placeholder = "12 palabras o 5 bloques",
                            imeAction = ImeAction.Next,
                            modifier = Modifier.padding(top = dimens.space2),
                        )
                        RbTextField(
                            value = sobrePegado,
                            onValueChange = { sobrePegado = it },
                            label = "Paquete cifrado",
                            placeholder = "Base64 del sobre (o el último preparado)",
                            imeAction = ImeAction.Done,
                            modifier = Modifier.padding(top = dimens.space2),
                        )
                        if (onTraerDeLaNube != null) {
                            RbButton(
                                label = "Traer de la nube",
                                onClick = {
                                    alcance.launch {
                                        when (val r = onTraerDeLaNube()) {
                                            is ResultadoTraerNube.Ok -> {
                                                sobrePegado = r.sobreBase64
                                            }
                                            is ResultadoTraerNube.Vacio,
                                            is ResultadoTraerNube.Error,
                                            -> {
                                                // El mensaje lo pone el contenedor en estadoRespaldo.
                                            }
                                        }
                                    }
                                },
                                variant = RbButtonVariant.Secondary,
                                fillWidth = true,
                                enabled = conectado && !subiendoRespaldo,
                                modifier = Modifier.padding(top = dimens.space2),
                            )
                        }
                        RbButton(
                            label = "Abrir respaldo",
                            onClick = {
                                onRestaurarRespaldo(materialCuaderno, sobrePegado)
                            },
                            variant = RbButtonVariant.Secondary,
                            fillWidth = true,
                            modifier = Modifier.padding(top = dimens.space2),
                        )
                    }
                }
            }

            if (cola.isEmpty()) {
                item {
                    VacioDeCola(
                        hint = hintColaVacia(feria),
                        onVolver = onCerrar,
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

/**
 * Nada en la cola: se lee como un recado aliviado, no como un empty de sistema.
 *
 * Misma vara que Hoy / fiado: aire, una frase y un solo CTA. Sin asset.
 */
@Composable
internal fun VacioDeCola(
    hint: String,
    onVolver: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard(modifier = modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = dimens.space4),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(
                text = "No hay ninguna esperando",
                style = RbTheme.typography.heading,
                color = colors.textPrimary,
                textAlign = TextAlign.Center,
                modifier = Modifier.rbHeading(),
            )
            Text(
                text = hint,
                style = RbTheme.typography.body,
                color = colors.textSecondary,
                textAlign = TextAlign.Center,
            )
            RbButton(
                label = "Volver",
                onClick = onVolver,
            )
        }
    }
}

/**
 * Una venta de la cola: se lee como una nota del día (cuándo · qué · estado),
 * no como una fila de un log de sync.
 */
@Composable
private fun FilaDeVentaEnCola(
    venta: VentaEnCola,
    ahora: Long,
    onDescartar: () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard(modifier = Modifier.fillMaxWidth()) {
        // Cuándo: callado, arriba. La dueña reconoce la venta por la hora
        // relativa, no por un título de sistema.
        Text(
            text = tituloDeVenta(venta.cobradaEn, ahora),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        Text(
            text = detalleDeLineas(venta),
            style = RbTheme.typography.body,
            color = colors.textPrimary,
            modifier = Modifier.padding(top = dimens.space1),
        )

        if (venta.rechazada) {
            RbChip(
                label = "No se anotó",
                tone = RbChipTone.Danger,
                modifier = Modifier.padding(top = dimens.space2),
            )
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
            RbChip(
                label = etiquetaDeEspera(venta, ahora),
                tone = RbChipTone.Brand,
                modifier = Modifier.padding(top = dimens.space2),
            )
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

/** Pastilla corta del estado: se lee de reojo, sin sonar a error de red. */
private fun etiquetaDeEspera(venta: VentaEnCola, ahora: Long): String = when {
    venta.intentos == 0 -> "Esperando"
    venta.proximoIntentoEn > ahora -> "A reintentar"
    else -> "En camino"
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
