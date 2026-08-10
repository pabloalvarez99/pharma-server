package cl.rutbusiness.app.ui.entrada

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
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Elegí tu rubro (day-1). Feria primero (ADR-0022).
 *
 * No habla con el server todavía: guarda en preferencias locales y al primer
 * login se empuja `business.vertical` + se carga el pack.
 */
@Composable
fun ElegirRubro(
    onElegir: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Tu negocio",
            subtitle = "¿Cómo trabajás?",
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
                text = "Elegí lo que más se parece a tu día",
                style = RbTheme.typography.title,
                color = RbTheme.colors.textPrimary,
                modifier = Modifier.rbHeading(),
            )
            Text(
                text = "Después se puede cambiar. Esto define si ves escáner e " +
                    "impresora, o el agente y el fiado primero.",
                style = RbTheme.typography.body,
                color = RbTheme.colors.textSecondary,
            )

            OPCIONES.forEach { opcion ->
                RbCard(title = opcion.titulo) {
                    Text(
                        text = opcion.tagline,
                        style = RbTheme.typography.body,
                        color = RbTheme.colors.textSecondary,
                    )
                    RbButton(
                        label = if (opcion.recomendado) "Elegir (recomendado)" else "Elegir",
                        onClick = { onElegir(opcion.rubro) },
                        variant = if (opcion.recomendado) {
                            RbButtonVariant.Primary
                        } else {
                            RbButtonVariant.Secondary
                        },
                        fillWidth = true,
                        modifier = Modifier.padding(top = dimens.space2),
                    )
                }
            }
        }

        Column(
            modifier = Modifier
                .fillMaxWidth()
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
