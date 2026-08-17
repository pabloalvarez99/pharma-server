package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import cl.rutbusiness.app.entrada.matrizQrRescate
import cl.rutbusiness.core.backup.ClaveDelNegocio
import cl.rutbusiness.core.backup.HUELLA_SIZE
import cl.rutbusiness.core.backup.claveDeDemostracion
import cl.rutbusiness.core.backup.claveNuevaDelNegocio
import cl.rutbusiness.core.backup.htmlTarjetaImprimible
import cl.rutbusiness.core.backup.huellaVisualDelPayload
import cl.rutbusiness.core.backup.payloadQrRescate
import cl.rutbusiness.core.backup.svgMatrizCodigo
import cl.rutbusiness.core.backup.textoTarjetaImprimible
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Tarjeta de rescate (ADR-0022): la clave del negocio para el backup cifrado.
 *
 * Day-1 del feriante: sin esta hoja en el cuaderno, el robo del teléfono
 * pierde la historia. La pantalla **obliga** a leer el aviso; no se esconde
 * en un menú. Se siente una **hoja del cuaderno**, no un export técnico: llave
 * grande arriba, palabras fáciles de copiar, CTA anclado abajo.
 *
 * Por defecto genera material con CSPRNG del aparato ([claveNuevaDelNegocio]).
 * Tests / previews pueden inyectar [claveDeDemostracion]. No sube nada a la red.
 *
 * El "código para el QR" es el payload estable (`rutbusiness-rescue:v1:…`);
 * dibujo ZXing en pantalla + HTML de una página para imprimir / Guardar PDF.
 */
@Composable
fun TarjetaRescate(
    onListo: () -> Unit,
    modifier: Modifier = Modifier,
    clave: ClaveDelNegocio = remember { claveNuevaDelNegocio() },
    /** Slug del negocio para el payload QR (vacío = solo palabras/bloques). */
    tenantSlug: String = "mi-puesto",
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val shape = RbTheme.shapes.card
    val clipboard = LocalClipboardManager.current
    val compartir = LocalCompartirTarjeta.current
    val copy = remember { copyTarjeta() }
    var copiado by remember { mutableStateOf(false) }
    val qrPayload = remember(clave, tenantSlug) {
        payloadQrRescate(tenantSlug, clave.bloques)
    }
    val textoPagina = remember(clave, tenantSlug) {
        textoTarjetaImprimible(clave, tenantSlug)
    }
    val matrizQr = remember(qrPayload) {
        qrPayload?.let { matrizQrRescate(it) }
    }
    val htmlPagina = remember(clave, tenantSlug, matrizQr) {
        val svg = matrizQr?.let { svgMatrizCodigo(it) }
        htmlTarjetaImprimible(clave, tenantSlug, svgQr = svg)
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = copy.tituloBarra,
            subtitle = copy.subtituloBarra,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            // Hero: la llave del puesto, no un título de settings.
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
                    // Day-1 feria (ADR-0022): "puesto", no "negocio".
                    text = copy.tituloHero,
                    style = RbTheme.typography.title,
                    color = colors.textPrimary,
                    modifier = Modifier.rbHeading(),
                )
                Text(
                    text = copy.cuerpoHero,
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
            }

            SeccionTarjeta(titulo = copy.tituloPalabras) {
                Text(
                    text = clave.fraseCompleta(),
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                    fontFamily = FontFamily.Monospace,
                )
            }

            SeccionTarjeta(titulo = copy.tituloBloques) {
                Text(
                    text = clave.bloquesCompletos(),
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                    fontFamily = FontFamily.Monospace,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            if (qrPayload != null) {
                SeccionTarjeta(titulo = copy.tituloQr) {
                    Text(
                        text = copy.ayudaQr,
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                    // QR real (ZXing). Si falla el encode, huella visual de respaldo.
                    val grillaHuella = remember(qrPayload) {
                        huellaVisualDelPayload(qrPayload)
                    }
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = dimens.space2),
                        contentAlignment = Alignment.Center,
                    ) {
                        if (matrizQr != null) {
                            MatrizCodigoCanvas(
                                grilla = matrizQr,
                                oscuro = colors.textPrimary,
                                claro = colors.surface,
                                modifier = Modifier
                                    .size(200.dp)
                                    .border(1.dp, colors.outlineStrong, shape)
                                    .padding(8.dp)
                                    .background(colors.surface),
                            )
                        } else {
                            HuellaVisualCanvas(
                                grilla = grillaHuella,
                                oscuro = colors.textPrimary,
                                claro = colors.surface,
                                modifier = Modifier
                                    .size(168.dp)
                                    .border(1.dp, colors.outlineStrong, shape)
                                    .padding(6.dp),
                            )
                        }
                    }
                    Text(
                        text = if (matrizQr != null) copy.pieQrOk else copy.pieQrFallo,
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                        modifier = Modifier.padding(top = dimens.space2),
                    )
                    Text(
                        text = qrPayload,
                        style = RbTheme.typography.label,
                        color = colors.textPrimary,
                        fontFamily = FontFamily.Monospace,
                        modifier = Modifier.padding(top = dimens.space2),
                    )
                }
            }

            // Marco grueso de marca: se ve al sol como "hoja importante".
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .border(dimens.focusRing, colors.brandText, shape)
                    .background(colors.brandContainer, shape)
                    .padding(dimens.space3),
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                    Text(
                        text = copy.tituloAnotar,
                        style = RbTheme.typography.bodyStrong,
                        color = colors.textPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = copy.cuerpoAnotar,
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                    // Filas de 3 palabras para copiar a mano.
                    clave.palabras.chunked(3).forEachIndexed { fila, tres ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            tres.forEachIndexed { j, palabra ->
                                val n = fila * 3 + j + 1
                                Text(
                                    text = "$n. $palabra",
                                    style = RbTheme.typography.label,
                                    color = colors.textPrimary,
                                    fontFamily = FontFamily.Monospace,
                                )
                            }
                        }
                    }
                }
            }

            // Una página: copiar, guardar en Notas, o imprimir / PDF.
            SeccionTarjeta(titulo = copy.tituloPagina) {
                Text(
                    text = textoPagina,
                    style = RbTheme.typography.label,
                    color = colors.textPrimary,
                    fontFamily = FontFamily.Monospace,
                )
                RbButton(
                    label = if (copiado) copy.ctaCopiado else copy.ctaCopiar,
                    onClick = {
                        clipboard.setText(AnnotatedString(textoPagina))
                        copiado = true
                    },
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space2),
                )
                // Sin plataforma detrás (preview, test, iOS todavía) el botón no
                // se dibuja: vale más que falte a que exista y no haga nada.
                if (compartir != null) RbButton(
                    label = copy.ctaGuardarNota,
                    onClick = {
                        compartir.compartirTexto(
                            asunto = "RutAgent - tarjeta de rescate",
                            texto = textoPagina,
                        )
                    },
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space1),
                )
                if (compartir != null) RbButton(
                    label = copy.ctaImprimir,
                    onClick = {
                        compartir.imprimirHtml(htmlPagina)
                    },
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space1),
                )
            }

            Text(
                text = copy.piePagina,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline),
        )

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.surfaceRaised)
                .windowInsetsPadding(
                    WindowInsets.safeDrawing.only(
                        WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                    ),
                )
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            RbButton(
                label = copy.ctaListo,
                onClick = onListo,
                fillWidth = true,
            )
            RbButton(
                label = copy.ctaSeguirSinAnotar,
                onClick = onListo,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/** Bloque de contenido de la tarjeta: quieto, legible al sol. */
@Composable
private fun SeccionTarjeta(
    titulo: String,
    content: @Composable () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outline, shape)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        content()
    }
}

/**
 * Dibuja la huella 21×21. No es un QR: no se escanea.
 */
@Composable
private fun HuellaVisualCanvas(
    grilla: Array<BooleanArray>,
    oscuro: Color,
    claro: Color,
    modifier: Modifier = Modifier,
) {
    MatrizCodigoCanvas(
        grilla = grilla,
        oscuro = oscuro,
        claro = claro,
        modifier = modifier,
        sizeFija = HUELLA_SIZE,
    )
}

/**
 * Matriz booleana (QR ZXing o huella) dibujada a celda por celda.
 */
@Composable
private fun MatrizCodigoCanvas(
    grilla: Array<BooleanArray>,
    oscuro: Color,
    claro: Color,
    modifier: Modifier = Modifier,
    sizeFija: Int? = null,
) {
    val filas = sizeFija ?: grilla.size
    val cols = sizeFija ?: (grilla.firstOrNull()?.size ?: 1)
    Canvas(modifier = modifier.aspectRatio(1f)) {
        val cellW = size.minDimension / cols.toFloat()
        val cellH = size.minDimension / filas.toFloat()
        drawRect(color = claro, size = size)
        val yMax = minOf(filas, grilla.size)
        for (y in 0 until yMax) {
            val row = grilla[y]
            val xMax = minOf(cols, row.size)
            for (x in 0 until xMax) {
                if (row[x]) {
                    drawRect(
                        color = oscuro,
                        topLeft = Offset(x * cellW, y * cellH),
                        size = Size(cellW, cellH),
                    )
                }
            }
        }
    }
}
