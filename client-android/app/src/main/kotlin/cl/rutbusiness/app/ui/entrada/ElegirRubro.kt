package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
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
 * Elegí tu rubro (day-1). Feria primero (ADR-0022).
 *
 * Se siente una **mesa del puesto**, no un catálogo de productos: la opción
 * recomendada (feria) va arriba con más aire y relleno; el resto, más quieto.
 * Misma altura táctil en todos los botones (≥ 56 dp). No habla con el server:
 * guarda en preferencias locales y al primer login se empuja `business.vertical`.
 */
@Composable
fun ElegirRubro(
    onElegir: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "¿Cómo trabajas?",
            subtitle = "Elige lo que más se parece a tu día",
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            Text(
                text = "Después se puede cambiar. Esto define si ves el agente y el " +
                    "fiado primero, o escáner e impresora.",
                style = RbTheme.typography.body,
                color = RbTheme.colors.textPrimary,
            )

            OPCIONES.forEach { opcion ->
                CaminoDeRubro(
                    titulo = opcion.titulo,
                    cuerpo = opcion.tagline,
                    etiqueta = if (opcion.recomendado) {
                        "Elegir (recomendado)"
                    } else {
                        "Elegir"
                    },
                    onClick = { onElegir(opcion.rubro) },
                    primario = opcion.recomendado,
                )
            }
        }

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
        ) {
            RbButton(
                label = "Decidir después",
                onClick = { onElegir("otro") },
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/**
 * Un rubro con peso: el recomendado (feria) se lee de lejos; los demás no
 * pelean por atención. Misma vara visual que [CaminoDePuerta] en la puerta.
 */
@Composable
private fun CaminoDeRubro(
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
            style = if (primario) RbTheme.typography.heading else RbTheme.typography.bodyStrong,
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

private data class OpcionRubro(
    val rubro: String,
    val titulo: String,
    val tagline: String,
    val recomendado: Boolean = false,
)

/** Orden: feria beachhead primero. */
private val OPCIONES = listOf(
    OpcionRubro(
        rubro = "feria",
        titulo = "Feria / Calle",
        tagline = "Puesto, voz, fiado. Sin escáner ni impresora el día 1.",
        recomendado = true,
    ),
    OpcionRubro(
        rubro = "minimarket",
        titulo = "Minimarket / Almacén",
        tagline = "Caja, productos y stock del local.",
    ),
    OpcionRubro(
        rubro = "farmacia",
        titulo = "Farmacia",
        tagline = "Recetas, lotes, códigos de barra y boleta.",
    ),
    OpcionRubro(
        rubro = "tienda",
        titulo = "Tienda / Retail",
        tagline = "Ropa, accesorios, tallas y códigos.",
    ),
)
