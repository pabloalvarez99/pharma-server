package cl.rutbusiness.ui.showcase

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbConfirmDialog
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorKind
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.components.rbErrorCopy
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * The catalogue.
 *
 * Every component on one scrolling screen, so the set can be reviewed at a
 * glance and - the real reason it exists - so the whole system can be put under
 * a 200% font scale at once and watched for the three failures that matter:
 * text that clips, layout that breaks, and a control that goes off-screen.
 *
 * It is a `LazyColumn`, not a `Column(verticalScroll)`. Same rule as every list
 * in the product: the reference device has 1-2 GB of RAM.
 *
 * `RbFontScaleTest` renders this screen at 1.0x and 2.0x, scrolls it end to
 * end and asserts those three properties; this composable is the fixture that
 * test drives, so a component missing from here is a component nobody checks.
 */
@Composable
fun RbShowcaseScreen(modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens

    var rut by remember { mutableStateOf("") }
    var amount by remember { mutableStateOf("") }
    var badRut by remember { mutableStateOf("12.345.678-K") }
    var selectedChip by remember { mutableStateOf(0) }
    var dialogVisible by remember { mutableStateOf(false) }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Catálogo de componentes",
            subtitle = "Almacén Doña Rosa",
            onBack = {},
            actions = { RbChip(label = "Demo", tone = RbChipTone.Brand) },
        )

        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .windowInsetsPadding(
                    WindowInsets.safeDrawing.only(
                        WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                    ),
                )
                .padding(horizontal = dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(
                vertical = dimens.space4,
            ),
        ) {
            item {
                SectionTitle("Botones")
                RbCard {
                    RbButton(
                        label = "Cobrar",
                        onClick = {},
                        variant = RbButtonVariant.Primary,
                        fillWidth = true,
                    )
                    RbButton(
                        label = "Seguir comprando",
                        onClick = {},
                        variant = RbButtonVariant.Secondary,
                        fillWidth = true,
                    )
                    RbButton(
                        label = "Anular esta venta",
                        onClick = { dialogVisible = true },
                        variant = RbButtonVariant.Destructive,
                        fillWidth = true,
                    )
                    RbButton(
                        label = "No disponible",
                        onClick = {},
                        enabled = false,
                        fillWidth = true,
                    )
                }
            }

            item {
                SectionTitle("Campos de texto")
                RbCard {
                    RbTextField(
                        value = rut,
                        onValueChange = { rut = it },
                        label = "RUT del cliente",
                        placeholder = "12.345.678-9",
                        supportingText = "Sirve para dejar la venta a su nombre.",
                        numeric = true,
                    )
                    RbTextField(
                        value = amount,
                        onValueChange = { amount = it },
                        label = "Monto a cobrar",
                        placeholder = "$0",
                        numeric = true,
                        keyboardType = KeyboardType.Number,
                    )
                    RbTextField(
                        value = badRut,
                        onValueChange = { badRut = it },
                        label = "RUT con problema",
                        errorMessage = "Ese RUT no existe. Revisa el número y el dígito del final.",
                        numeric = true,
                    )
                    RbTextField(
                        value = "Bloqueado",
                        onValueChange = {},
                        label = "Campo deshabilitado",
                        enabled = false,
                    )
                }
            }

            item {
                SectionTitle("Chips")
                RbCard {
                    RbChipRow {
                        RbChip(label = "Al día", tone = RbChipTone.Brand)
                        RbChip(label = "Por vencer", tone = RbChipTone.Warn)
                        RbChip(label = "Vencido", tone = RbChipTone.Danger)
                    }
                    RbChipRow {
                        listOf("Hoy", "Semana", "Mes").forEachIndexed { index, label ->
                            RbChip(
                                label = label,
                                selected = selectedChip == index,
                                onClick = { selectedChip = index },
                            )
                        }
                    }
                }
            }

            item {
                SectionTitle("Lista")
                RbCard {
                    RbListRow(
                        title = "Paracetamol 500 mg",
                        subtitle = "Quedan 12 cajas",
                        value = "$1.290",
                        onClick = {},
                    )
                    RbDivider()
                    RbListRow(
                        title = "Ibuprofeno 400 mg",
                        subtitle = "Vence en 9 días",
                        value = "$2.450",
                        trailing = { RbChip(label = "Por vencer", tone = RbChipTone.Warn) },
                        onClick = {},
                    )
                    RbDivider()
                    RbListRow(
                        title = "Alcohol gel 250 ml",
                        subtitle = "Sin stock",
                        value = "$1.990",
                        onClick = {},
                    )
                }
            }

            item {
                SectionTitle("Cargando")
                RbCard {
                    RbLoadingState(label = "Buscando tus ventas de hoy...")
                    RbSkeletonLines(lines = 4)
                }
            }

            item {
                SectionTitle("Vacío")
                RbCard {
                    RbEmptyState(
                        title = "Todavía no has hecho ninguna venta",
                        hint = "Toca Cobrar, elige los productos y listo: la venta " +
                            "queda guardada aunque se corte internet.",
                        actionLabel = "Hacer la primera venta",
                        onAction = {},
                    )
                }
            }

            item {
                SectionTitle("Error")
                val offline = rbErrorCopy(RbErrorKind.Offline, "tus ventas")
                RbErrorState(
                    title = offline.title,
                    message = offline.message,
                    retryLabel = offline.retryLabel,
                    onRetry = {},
                )
            }

            item {
                val forbidden = rbErrorCopy(RbErrorKind.Forbidden, "el informe de caja")
                RbErrorState(
                    title = forbidden.title,
                    message = forbidden.message,
                    // Null retry: nothing the user can press would change this.
                    retryLabel = forbidden.retryLabel,
                    onRetry = null,
                )
            }

            item {
                SectionTitle("Diálogo")
                RbCard {
                    RbButton(
                        label = "Abrir confirmación",
                        onClick = { dialogVisible = true },
                        variant = RbButtonVariant.Secondary,
                        fillWidth = true,
                    )
                }
            }
        }
    }

    if (dialogVisible) {
        RbConfirmDialog(
            title = "¿Anular esta venta?",
            message = "Se va a devolver el stock y la boleta queda sin efecto. " +
                "Esto no se puede deshacer.",
            confirmLabel = "Sí, anular la venta",
            destructive = true,
            onConfirm = { dialogVisible = false },
            onDismiss = { dialogVisible = false },
        )
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text = text,
        style = RbTheme.typography.heading,
        color = RbTheme.colors.textSecondary,
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = RbTheme.dimens.space2)
            .rbHeading(),
    )
}
