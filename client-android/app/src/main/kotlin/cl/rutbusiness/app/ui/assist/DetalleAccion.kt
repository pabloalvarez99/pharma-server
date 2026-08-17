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
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull

/** Una línea del detalle: qué es, y cuánto o qué dice. */
data class LineaDetalle(val etiqueta: String, val valor: String)

/**
 * Traduce los `params` de una propuesta a las líneas que la dueña lee antes de
 * confirmar.
 *
 * Esta es la parte de la app donde una ambigüedad cuesta plata: si el detalle
 * no dice claro que son $5.000 y no $50.000, se registra el gasto equivocado.
 * De ahí las reglas:
 *
 * 1. **Los ids no se muestran, nunca.** No es una lista de nombres conocidos:
 *    cualquier clave `*_id` (o `id`) es para el server, no para la dueña. Así
 *    un id nuevo que el server agregue mañana (`po_id`, por ejemplo — hoy
 *    faltaba en la lista vieja y salía crudo en pantalla) queda cubierto sin
 *    tener que acordarse de agregarlo a mano.
 * 2. **La plata se escribe como plata, con un solo formato.** El server manda
 *    `"5000"` crudo; en pantalla va `$5.000`, siempre con `Clp.formatear`.
 *    Un número sin separadores, o dos formatos distintos en la misma
 *    tarjeta, es exactamente donde se confunde un orden de magnitud.
 * 3. **Nunca se dibuja JSON crudo.** Un arreglo, un objeto o un booleano no
 *    tienen una representación de texto obvia, así que el fallback para lo
 *    que no se sabe representar es NO MOSTRAR la línea — nunca volcar
 *    `[...]`, `{...}` o `false` en la pantalla donde se confirma dinero. La
 *    única excepción con forma propia es `lines` (el carrito): ver
 *    [filasCarrito].
 * 4. **El orden lo fija la app, no el JSON.** Primero el carrito y el monto,
 *    después el resto. El dato que más duele si está mal va arriba.
 */
object DetalleAccion {

    /**
     * `product_id`, `session_id`, `supplier_id`, `po_id`, `customer_id`… — un
     * identificador de base de datos no le dice nada a nadie y empuja hacia
     * abajo lo que sí importa. El nombre está al lado; el id es para el
     * server. Regla genérica (sufijo `_id`) en vez de una lista fija: la
     * lista fija es exactamente lo que dejó pasar `po_id` crudo a pantalla.
     */
    private fun esId(clave: String): Boolean = clave == "id" || clave.endsWith("_id")

    /** Campos que son plata y se formatean como tal — un solo formato, `$`. */
    private val PLATA = setOf(
        "amount",
        "unit_cost",
        "price",
        "old_price",
        "new_price",
        "counted",
        "expected",
        "total",
        "subtotal",
        "debt_before",
        "opening_cash",
        "monto",
    )

    /**
     * Qué se muestra primero. Lo que no está acá va después, en el orden en
     * que vino, para que un campo nuevo del server aparezca igual en vez de
     * desaparecer en silencio.
     */
    private val ORDEN = listOf(
        "amount",
        "subtotal",
        "total",
        "opening_cash",
        "new_price",
        "old_price",
        "counted",
        "expected",
        "debt_before",
        "quantity",
        "items",
        "stock",
        "old_stock",
        "new_stock",
        "set",
        "delta",
        "description",
        "name",
        "product_name",
        "supplier_name",
        "customer_name",
        "patient_name",
        "category",
        "payment_method",
        "unit_cost",
        "register_name",
        "rut",
        "patient_rut",
        "phone",
        "email",
        "warnings",
        "abre_puesto",
    )

    private val ETIQUETAS = mapOf(
        "amount" to "Monto",
        "category" to "Categoría",
        "description" to "Detalle",
        "payment_method" to "Cómo se paga",
        "supplier_name" to "Proveedor",
        "product_name" to "Producto",
        "customer_name" to "Cliente",
        "patient_name" to "Paciente",
        "name" to "Nombre",
        "quantity" to "Cantidad",
        "items" to "Productos",
        "unit_cost" to "Precio por unidad",
        "price" to "Precio",
        "old_price" to "Precio que tiene hoy",
        "new_price" to "Precio nuevo",
        "stock" to "Stock",
        "old_stock" to "Stock que hay hoy",
        "new_stock" to "Stock nuevo",
        "set" to "Stock nuevo",
        "delta" to "Cambio",
        "counted" to "Lo que contaste",
        "expected" to "Lo que debería haber",
        "subtotal" to "Subtotal",
        "total" to "Total",
        "debt_before" to "Debía antes",
        "opening_cash" to "Fondo inicial",
        "register_name" to "Caja",
        "rut" to "RUT",
        "patient_rut" to "RUT del paciente",
        "phone" to "Teléfono",
        "email" to "Correo",
        "warnings" to "Advertencias",
        "abre_puesto" to "Abre el puesto",
    )

    /** Valores que el server manda en clave y se dicen en castellano. */
    private val VALORES = mapOf(
        "cash" to "efectivo",
        "card" to "tarjeta",
        "transfer" to "transferencia",
        "credit" to "fiado",
        "pos_cash" to "efectivo",
        "pos_fiado" to "fiado",
    )

    fun lineas(params: JsonObject): List<LineaDetalle> {
        // El carrito es la información más valiosa de la tarjeta y no es un
        // campo escalar: se arma aparte, una fila por producto, y va primero.
        val carrito = params["lines"]?.let(::filasCarrito).orEmpty()

        val visibles = params.entries
            .filter { (clave, valor) -> clave != "lines" && !esId(clave) && !esVacio(valor) }

        val porOrden = visibles.sortedBy { (clave, _) ->
            val i = ORDEN.indexOf(clave)
            if (i >= 0) i else ORDEN.size
        }

        val resto = porOrden.mapNotNull { (clave, valor) ->
            val texto = valorLegible(clave, valor) ?: return@mapNotNull null
            LineaDetalle(etiqueta = ETIQUETAS[clave] ?: humanizar(clave), valor = texto)
        }

        return carrito + resto
    }

    /**
     * Un carrito son varias líneas producto/cantidad/precio: se merecen una
     * fila de verdad por producto, no una cadena con todo el arreglo
     * aplastado. Cada línea que no trae lo mínimo para armar la fila
     * (nombre, cantidad, precio) se descarta en silencio — mejor un carrito
     * con una fila de menos que una fila con JSON crudo adentro.
     *
     * `product_id` de cada línea nunca se usa acá: es el mismo criterio que
     * el resto de la tarjeta, solo que aplicado adentro del arreglo en vez de
     * a nivel de `params`.
     */
    private fun filasCarrito(valor: JsonElement): List<LineaDetalle> {
        val arreglo = valor as? JsonArray ?: return emptyList()
        return arreglo.mapNotNull { elemento ->
            val objeto = elemento as? JsonObject ?: return@mapNotNull null
            val nombre = objeto["product_name"]?.textoPlanoOrNull() ?: return@mapNotNull null
            val cantidad = objeto["quantity"]?.textoPlanoOrNull() ?: return@mapNotNull null
            val precio = objeto["unit_price"]?.textoPlanoOrNull()?.let(Clp::formatear)
                ?: return@mapNotNull null
            val total = objeto["line_total"]?.textoPlanoOrNull()?.let(Clp::formatear)
            val valorTexto = if (total != null) {
                "$cantidad × $precio c/u = $total"
            } else {
                "$cantidad × $precio c/u"
            }
            LineaDetalle(etiqueta = nombre, valor = valorTexto)
        }
    }

    private fun JsonElement.textoPlanoOrNull(): String? =
        (this as? JsonPrimitive)?.contentOrNullSeguro()

    /**
     * Nunca hay vacío decorativo: un id, un texto vacío/null, un booleano en
     * `false` o un arreglo vacío no le dicen nada a la dueña y no ocupan una
     * línea. Un objeto anidado (que no sea `lines`, ya tratado aparte) tampoco
     * tiene una forma de texto obvia — se trata igual que vacío, nunca se
     * vuelca crudo.
     */
    private fun esVacio(valor: JsonElement): Boolean = when (valor) {
        is JsonArray -> valor.isEmpty()
        is JsonObject -> true
        is JsonPrimitive -> when (valor.booleanOrNull) {
            false -> true
            else -> valor.contentOrNullSeguro().isNullOrBlank()
        }
        else -> false
    }

    private fun JsonPrimitive.contentOrNullSeguro(): String? =
        if (isString || content != "null") content else null

    /**
     * El texto final de una línea, o `null` si no hay forma segura de
     * mostrarla — nunca un `toString()` de un arreglo o un objeto. Un
     * arreglo de puros strings (`warnings`) se lee como lista; cualquier otra
     * cosa que no sea texto plano se oculta.
     */
    private fun valorLegible(clave: String, valor: JsonElement): String? = when (valor) {
        is JsonArray -> textoDeTextos(valor)
        is JsonObject -> null
        is JsonPrimitive -> formatear(clave, valor)
        else -> null
    }

    /** Une un arreglo de strings en una lista legible; `null` si algún elemento no es texto. */
    private fun textoDeTextos(arreglo: JsonArray): String? {
        val textos = arreglo.map { elemento ->
            (elemento as? JsonPrimitive)?.takeIf { it.isString }?.content
                ?: return null
        }
        return textos.joinToString("\n") { "• $it" }
    }

    /**
     * Un booleano nunca se dibuja como `true`/`false`: en `false` ya se filtró
     * en [esVacio]; en `true` se dice «Sí», la única forma en que un booleano
     * es información útil para la dueña.
     */
    private fun formatear(clave: String, primitivo: JsonPrimitive): String {
        if (primitivo.booleanOrNull == true) return "Sí"
        val crudo = primitivo.content
        return when {
            clave in PLATA -> Clp.formatear(crudo)
            else -> VALORES[crudo.lowercase()] ?: crudo
        }
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
