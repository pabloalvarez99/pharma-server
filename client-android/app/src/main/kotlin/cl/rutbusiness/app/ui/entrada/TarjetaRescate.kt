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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import cl.rutbusiness.core.backup.ClaveDelNegocio
import cl.rutbusiness.core.backup.HUELLA_SIZE
import cl.rutbusiness.core.backup.claveDeDemostracion
import cl.rutbusiness.core.backup.claveNuevaDelNegocio
import cl.rutbusiness.core.backup.huellaVisualDelPayload
import cl.rutbusiness.core.backup.payloadQrRescate
import cl.rutbusiness.core.backup.textoTarjetaImprimible
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Tarjeta de rescate (ADR-0022): la clave del negocio para el backup cifrado.
 *
 * Day-1 del feriante: sin esta hoja en el cuaderno, el robo del teléfono
 * pierde la historia. La pantalla **obliga** a leer el aviso; no se esconde
 * en un menú.
 *
 * Por defecto genera material con CSPRNG del aparato ([claveNuevaDelNegocio]).
 * Tests / previews pueden inyectar [claveDeDemostracion]. No sube nada a la red.
 *
 * El "código para el QR" es el payload estable (`rutbusiness-rescue:v1:…`);
 * el dibujo del QR / PDF llega cuando el capitán pida la librería de códigos.
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
    val qrPayload = remember(clave, tenantSlug) {
        payloadQrRescate(tenantSlug, clave.bloques)
    }
    val textoPagina = remember(clave, tenantSlug) {
        textoTarjetaImprimible(clave, tenantSlug)
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Tu tarjeta de rescate",
            subtitle = "Escribila en el cuaderno",
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
                text = "Esta es la llave de tu negocio",
                style = RbTheme.typography.title,
                color = colors.textPrimary,
                modifier = Modifier.rbHeading(),
            )

            Text(
                text = "Si te roban o se te rompe el teléfono, con esta llave " +
                    "y tu cuenta de Google volvés a entrar a tus ventas y deudas. " +
                    "Sin ella, el respaldo es basura: nosotros no podemos " +
                    "recuperarla por vos.",
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )

            RbCard(title = "Palabras (12)") {
                Text(
                    text = clave.fraseCompleta(),
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                    fontFamily = FontFamily.Monospace,
                )
            }

            RbCard(title = "Bloques (más fáciles con lápiz)") {
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
                RbCard(title = "Código para el QR / PDF") {
                    Text(
                        text = "Cuando imprimamos la tarjeta, este texto va en el " +
                            "código. Hoy podés copiarlo al cuaderno si querés.",
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                    // Huella visual: se ve como código; NO es QR escaneable.
                    val grilla = remember(qrPayload) { huellaVisualDelPayload(qrPayload) }
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = dimens.space2),
                        contentAlignment = Alignment.Center,
                    ) {
                        HuellaVisualCanvas(
                            grilla = grilla,
                            oscuro = colors.textPrimary,
                            claro = colors.surface,
                            modifier = Modifier
                                .size(168.dp)
                                .border(1.dp, colors.outlineStrong, RbTheme.shapes.card)
                                .padding(6.dp),
                        )
                    }
                    Text(
                        text = "Huella visual del código (no es un QR escaneable). " +
                            "El texto de abajo es lo que importa si lo copiás.",
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

            // Marco grueso: se ve al sol como "hoja importante".
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(2.dp, colors.brandText, RbTheme.shapes.card)
                    .background(colors.brandContainer, RbTheme.shapes.card)
                    .padding(dimens.space3),
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                    Text(
                        text = "Anotá esto YA en tu cuaderno",
                        style = RbTheme.typography.bodyStrong,
                        color = colors.textPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "Pegá esta hoja o copiá las palabras. " +
                            "Si las perdés, perdés el historial del respaldo. " +
                            "No las mandes por WhatsApp.",
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

            // Página de texto lista para copiar al cuaderno / nota (sin PDF
            // ni lib de QR todavía). Mismas palabras + payload del código.
            RbCard(title = "Texto para el cuaderno (una página)") {
                Text(
                    text = textoPagina,
                    style = RbTheme.typography.label,
                    color = colors.textPrimary,
                    fontFamily = FontFamily.Monospace,
                )
            }

            Text(
                text = "El QR escaneable y el PDF llegan después. " +
                    "Hoy alcanza con copiar este texto al cuaderno " +
                    "(y mirar la huella para reconocer la tarjeta).",
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
                label = "Ya la anoté en el cuaderno",
                onClick = onListo,
                fillWidth = true,
            )
            RbButton(
                label = "Seguir sin anotar (no recomendado)",
                onClick = onListo,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
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
    Canvas(modifier = modifier.aspectRatio(1f)) {
        val n = HUELLA_SIZE.toFloat()
        val cell = size.minDimension / n
        // Fondo claro.
        drawRect(color = claro, size = size)
        for (y in 0 until HUELLA_SIZE) {
            for (x in 0 until HUELLA_SIZE) {
                if (grilla[y][x]) {
                    drawRect(
                        color = oscuro,
                        topLeft = Offset(x * cell, y * cell),
                        size = Size(cell, cell),
                    )
                }
            }
        }
    }
}
