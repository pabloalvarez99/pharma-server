package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonPrimitive

/** Una línea del detalle: qué es, y cuánto o qué dice. */
data class LineaDetalle(val etiqueta: String, val valor: String)

/**
 * Traduce los `params` de una propuesta a las líneas que la dueña lee antes de
 * confirmar.
 *
 * Esta es la parte de la app donde una ambigüedad cuesta plata: si el detalle
 * no dice claro que son $5.000 y no $50.000, se registra el gasto equivocado.
 * De ahí las tres reglas:
 *
 * 1. **Los ids no se muestran.** `product_id`, `session_id`, `supplier_id` no
 *    le dicen nada a nadie y empujan hacia abajo lo que sí importa. El nombre
 *    del producto está al lado; el id es para el server.
 * 2. **La plata se escribe como plata.** El server manda `"5000"` crudo; en
 *    pantalla va `$5.000`. Un número sin separadores es exactamente donde se
 *    confunde un orden de magnitud.
 * 3. **El orden lo fija la app, no el JSON.** Primero el monto, después el
 *    resto. El dato que más duele si está mal va arriba.
 */
object DetalleAccion {

    /** Campos que existen para el server y no significan nada para la dueña. */
    private val OCULTOS = setOf(
        "product_id",
        "supplier_id",
        "session_id",
        "customer_id",
        "prescription_id",
        "order_id",
    )

    /** Campos que son plata y se formatean como tal. */
    private val PLATA = setOf(
        "amount",
        "unit_cost",
        "price",
        "old_price",
        "new_price",
        "counted",
        "expected",
        "total",
        "monto",
    )

    /**
     * Qué se muestra primero. Lo que no está acá va después, en el orden en
     * que vino, para que un campo nuevo del server aparezca igual en vez de
     * desaparecer en silencio.
     */
    private val ORDEN = listOf(
        "amount",
        "new_price",
        "old_price",
        "counted",
        "expected",
        "quantity",
        "stock",
        "old_stock",
        "new_stock",
        "delta",
        "description",
        "name",
        "product_name",
        "supplier_name",
        "customer_name",
        "category",
        "payment_method",
        "unit_cost",
        "register_name",
        "rut",
        "phone",
        "email",
    )

    private val ETIQUETAS = mapOf(
        "amount" to "Monto",
        "category" to "Categoría",
        "description" to "Detalle",
        "payment_method" to "Cómo se paga",
        "supplier_name" to "Proveedor",
        "product_name" to "Producto",
        "customer_name" to "Cliente",
        "name" to "Nombre",
        "quantity" to "Cantidad",
        "unit_cost" to "Precio por unidad",
        "price" to "Precio",
        "old_price" to "Precio que tiene hoy",
        "new_price" to "Precio nuevo",
        "stock" to "Stock",
        "old_stock" to "Stock que hay hoy",
        "new_stock" to "Stock nuevo",
        "delta" to "Cambio",
        "counted" to "Lo que contaste",
        "expected" to "Lo que debería haber",
        "register_name" to "Caja",
        "rut" to "RUT",
        "phone" to "Teléfono",
        "email" to "Correo",
    )

    /** Valores que el server manda en clave y se dicen en castellano. */
    private val VALORES = mapOf(
        "cash" to "efectivo",
        "card" to "tarjeta",
        "transfer" to "transferencia",
        "credit" to "fiado",
    )

    fun lineas(params: JsonObject): List<LineaDetalle> {
        val visibles = params.entries
            .filter { (clave, valor) -> clave !in OCULTOS && !esVacio(valor) }

        val porOrden = visibles.sortedBy { (clave, _) ->
            val i = ORDEN.indexOf(clave)
            if (i >= 0) i else ORDEN.size
        }

        return porOrden.map { (clave, valor) ->
            LineaDetalle(
                etiqueta = ETIQUETAS[clave] ?: humanizar(clave),
                valor = formatear(clave, textoDe(valor)),
            )
        }
    }

    private fun esVacio(valor: kotlinx.serialization.json.JsonElement): Boolean {
        val p = valor as? JsonPrimitive ?: return false
        return p.contentOrNullSeguro().isNullOrBlank()
    }

    private fun JsonPrimitive.contentOrNullSeguro(): String? =
        if (isString || content != "null") content else null

    private fun textoDe(valor: kotlinx.serialization.json.JsonElement): String =
        runCatching { valor.jsonPrimitive.content }.getOrElse { valor.toString() }

    private fun formatear(clave: String, crudo: String): String = when {
        clave in PLATA -> Clp.formatear(crudo)
        else -> VALORES[crudo.lowercase()] ?: crudo
    }

    /** `payment_method` -> `Payment method`, para una clave que no conocemos. */
    private fun humanizar(clave: String): String =
        clave.replace('_', ' ').replaceFirstChar { it.uppercase() }
}

/**
 * La tarjeta del desglose: lo que la dueña lee línea a línea antes de decir sí.
 *
 * Vive acá (no enterrada en la tarjeta de propuesta) porque el detalle es la
 * defensa contra un cero de más, y tiene que leerse como un papel del puesto:
 * superficie clara, borde que se ve al sol, plata en monoespaciado, valor
 * más fuerte que la etiqueta. Sin `maxLines`: al 200% se envuelve, no se corta.
 *
 * @return si [lineas] viene vacío no dibuja nada (no hay un vacío decorativo).
 */
@Composable
fun TarjetaDetalleAccion(
    lineas: List<LineaDetalle>,
    modifier: Modifier = Modifier,
) {
    if (lineas.isEmpty()) return

    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    // Card del design system (radio de tarjeta, no de campo): el desglose se
    // siente un papel aparte, no un input.
    val shape = RbTheme.shapes.card

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            // outlineStrong: surface sobre brandContainer se pierde al sol con
            // un hairline; el borde de control (>= 3.0) se mantiene.
            .border(dimens.border, colors.outlineStrong, shape)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Text(
            text = "Esto es lo que voy a guardar",
            style = RbTheme.typography.label,
            color = colors.textSecondary,
        )

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline)
                .clearAndSetSemantics { },
        )

        Column(
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
            modifier = Modifier.fillMaxWidth(),
        ) {
            lineas.forEach { linea ->
                val esPlata = linea.valor.startsWith("$") || linea.valor.startsWith("-$")
                RbReflowRow(
                    spacing = dimens.space3,
                    modifier = Modifier.fillMaxWidth(),
                    content = {
                        Text(
                            text = linea.etiqueta,
                            style = RbTheme.typography.support,
                            color = colors.textSecondary,
                        )
                    },
                    trailing = {
                        // La plata en monoespaciado (numeric): los dígitos se
                        // alinean y un cero de más se pilla de un vistazo. El
                        // resto va en bodyStrong: el valor es la decisión.
                        Text(
                            text = linea.valor,
                            style = if (esPlata) {
                                RbTheme.typography.numeric
                            } else {
                                RbTheme.typography.bodyStrong
                            },
                            color = colors.textPrimary,
                        )
                    },
                )
            }
        }
    }
}

/**
 * Pesos chilenos como se escriben en Chile: `$5.000`, punto para los miles y
 * sin decimales.
 *
 * Sin `NumberFormat`: en Android el separador sale del locale del teléfono, y
 * un aparato configurado en inglés escribiría `$5,000`, que en Chile se lee
 * como cinco pesos con cero. La moneda del producto es el peso chileno y se
 * escribe siempre igual, no según cómo quedó configurado el teléfono.
 */
object Clp {

    fun formatear(crudo: String): String {
        val limpio = crudo.trim()
        val negativo = limpio.startsWith("-")
        val sinSigno = limpio.removePrefix("-").removePrefix("+")

        // El server manda Decimal como texto: puede venir "5000" o "5000.00".
        val enteros = sinSigno.substringBefore('.').ifEmpty { "0" }
        if (enteros.any { !it.isDigit() }) return crudo

        val conPuntos = enteros.reversed()
            .chunked(3)
            .joinToString(".")
            .reversed()

        return if (negativo) "-$$conPuntos" else "$$conPuntos"
    }
}
