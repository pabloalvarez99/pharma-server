package cl.rutbusiness.app.ui.assist

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * El detalle de la tarjeta es lo que la dueña lee para decidir si el agente
 * entendió bien. Un cero de más acá es un gasto mal registrado, así que se
 * prueba como código de plata y no como formateo cosmético.
 */
class DetalleAccionTest {

    private fun params(json: String): JsonObject =
        Json.parseToJsonElement(json) as JsonObject

    /** Los `params` reales de `registrar_gasto`, copiados de `actions.rs`. */
    private val gasto = params(
        """
        {
          "category": "arriendo",
          "description": "arriendo",
          "amount": "5000",
          "payment_method": "cash"
        }
        """,
    )

    @Test
    fun `la plata se escribe como plata`() {
        val lineas = DetalleAccion.lineas(gasto)
        val monto = lineas.first { it.etiqueta == "Monto" }
        assertEquals("$5.000", monto.valor)
    }

    @Test
    fun `el monto va primero`() {
        // El dato que más duele si está mal es el primero que se lee.
        assertEquals("Monto", DetalleAccion.lineas(gasto).first().etiqueta)
    }

    @Test
    fun `la forma de pago se dice en castellano`() {
        val lineas = DetalleAccion.lineas(gasto)
        val pago = lineas.first { it.etiqueta == "Cómo se paga" }
        assertEquals("efectivo", pago.valor)
    }

    @Test
    fun `los ids del server no se muestran`() {
        val conIds = params(
            """
            {
              "product_id": "product:abc123",
              "product_name": "Paracetamol 500 mg",
              "old_stock": "12",
              "new_stock": "20"
            }
            """,
        )
        val etiquetas = DetalleAccion.lineas(conIds).map { it.etiqueta }
        assertTrue(
            "un id de base de datos no le dice nada a la dueña: $etiquetas",
            etiquetas.none { it.contains("id", ignoreCase = true) },
        )
        assertTrue("el nombre del producto sí se muestra", etiquetas.contains("Producto"))
    }

    /**
     * Un campo que el server agregue mañana tiene que aparecer igual. Que se
     * caiga en silencio sería peor que verlo con una etiqueta fea: la dueña
     * estaría confirmando algo que la tarjeta no le mostró.
     */
    @Test
    fun `un campo desconocido se muestra igual`() {
        val nuevo = params("""{ "campo_nuevo_del_server": "algo" }""")
        val lineas = DetalleAccion.lineas(nuevo)
        assertEquals(1, lineas.size)
        assertEquals("Campo nuevo del server", lineas.first().etiqueta)
        assertEquals("algo", lineas.first().valor)
    }

    @Test
    fun `los campos vacios no ocupan una linea`() {
        val conVacios = params("""{ "name": "Rosa", "rut": "", "phone": "912345678" }""")
        val etiquetas = DetalleAccion.lineas(conVacios).map { it.etiqueta }
        assertEquals(listOf("Nombre", "Teléfono"), etiquetas)
    }

    // ---- el caso real que motivó esta reescritura: la venta de la captura ----

    /**
     * Los `params` reales de `vender`, copiados tal cual de la captura del
     * bug: la tarjeta de confirmación de venta mostraba JSON crudo. Este es
     * el caso que no puede volver a pasar.
     */
    private val venta = params(
        """
        {
          "lines": [
            {
              "product_id": "product:zoyrw299djv9372hhb2r",
              "product_name": "Tomate",
              "quantity": 1,
              "unit_price": "2000",
              "line_total": "2000"
            }
          ],
          "subtotal": "2000",
          "total": "2000",
          "payment_method": "pos_cash",
          "customer_id": null,
          "customer_name": null,
          "warnings": [],
          "abre_puesto": false
        }
        """,
    )

    /**
     * La prueba general: sea cual sea la clave, en ninguna línea de la
     * tarjeta puede aparecer un carácter de JSON crudo, un booleano en texto,
     * un id interno o la palabra en inglés `Lines`/`pos_cash`. Barrer
     * caracteres en vez de fijar frase por frase: así una clave nueva del
     * server queda cubierta igual, no solo las de esta captura.
     */
    @Test
    fun `la venta de la captura no muestra nada crudo`() {
        val lineas = DetalleAccion.lineas(venta)
        val todoElTexto = lineas.joinToString("\n") { "${it.etiqueta}: ${it.valor}" }

        assertTrue("no debe aparecer un id de producto: $todoElTexto", "product:" !in todoElTexto)
        assertTrue("no debe aparecer un arreglo crudo: $todoElTexto", "[" !in todoElTexto)
        assertTrue("no debe aparecer un objeto crudo: $todoElTexto", "{" !in todoElTexto)
        assertTrue("no debe aparecer un booleano crudo: $todoElTexto", "false" !in todoElTexto)
        assertTrue("no debe aparecer el método de pago crudo: $todoElTexto", "pos_cash" !in todoElTexto)
        assertTrue("no debe aparecer la etiqueta en inglés: $todoElTexto", "Lines" !in todoElTexto)
    }

    @Test
    fun `el carrito se dibuja como una fila por producto, no como un bloque`() {
        val lineas = DetalleAccion.lineas(venta)
        val filaTomate = lineas.first { it.etiqueta == "Tomate" }
        assertEquals("1 × $2.000 c/u = $2.000", filaTomate.valor)
    }

    @Test
    fun `subtotal y total usan el mismo formato de plata`() {
        val lineas = DetalleAccion.lineas(venta)
        assertEquals("$2.000", lineas.first { it.etiqueta == "Subtotal" }.valor)
        assertEquals("$2.000", lineas.first { it.etiqueta == "Total" }.valor)
    }

    @Test
    fun `pos_cash se dice efectivo`() {
        val lineas = DetalleAccion.lineas(venta)
        assertEquals("efectivo", lineas.first { it.etiqueta == "Cómo se paga" }.valor)
    }

    @Test
    fun `un booleano en false no ocupa una linea`() {
        val lineas = DetalleAccion.lineas(venta)
        assertTrue(
            "abre_puesto en false no es información para la dueña",
            lineas.none { it.etiqueta == "Abre el puesto" },
        )
    }

    @Test
    fun `un arreglo vacio no ocupa una linea`() {
        val lineas = DetalleAccion.lineas(venta)
        assertTrue("warnings vacío no debe dibujarse", lineas.none { it.etiqueta == "Advertencias" })
    }

    @Test
    fun `un booleano en true se dice Si, nunca la palabra true`() {
        val conApertura = params(
            """
            { "lines": [], "subtotal": "0", "total": "0", "abre_puesto": true }
            """,
        )
        val fila = DetalleAccion.lineas(conApertura).first { it.etiqueta == "Abre el puesto" }
        assertEquals("Sí", fila.valor)
    }

    @Test
    fun `un arreglo de strings no vacio se lee como lista, no como JSON`() {
        val conAdvertencias = params(
            """
            { "warnings": ["interactúa con otro medicamento"] }
            """,
        )
        val fila = DetalleAccion.lineas(conAdvertencias).first { it.etiqueta == "Advertencias" }
        assertTrue(fila.valor.contains("interactúa con otro medicamento"))
        assertTrue("[" !in fila.valor && "]" !in fila.valor)
    }

    @Test
    fun `un id nuevo que la lista vieja no conocia tambien se oculta`() {
        // po_id no estaba en la lista fija de ids ocultos: con la regla
        // genérica (sufijo _id) queda cubierto igual.
        val conPoId = params(
            """{ "po_id": "purchase_order:xyz789", "items": 3, "total": "15000" }""",
        )
        val etiquetas = DetalleAccion.lineas(conPoId).map { it.etiqueta }
        val valores = DetalleAccion.lineas(conPoId).map { it.valor }
        assertTrue(etiquetas.none { it.contains("id", ignoreCase = true) })
        assertTrue(valores.none { it.contains("purchase_order:") })
    }

    @Test
    fun `una linea de carrito sin nombre de producto se descarta en silencio`() {
        // Mejor una fila de menos que una fila con JSON crudo adentro.
        val lineaIncompleta = params(
            """{ "lines": [ { "quantity": 1, "unit_price": "2000" } ] }""",
        )
        assertTrue(DetalleAccion.lineas(lineaIncompleta).isEmpty())
    }

    @Test
    fun `los miles llevan punto, no coma`() {
        // Un teléfono configurado en inglés escribiría "$1,250,000", que en
        // Chile se lee como un peso con veinticinco. La moneda del producto se
        // escribe siempre igual, no según el locale del aparato.
        assertEquals("$1.250.000", Clp.formatear("1250000"))
        assertEquals("$5.000", Clp.formatear("5000.00"))
        assertEquals("$0", Clp.formatear("0"))
        assertEquals("$999", Clp.formatear("999"))
        assertEquals("-$1.500", Clp.formatear("-1500"))
    }

    @Test
    fun `un monto que no es numero se muestra tal cual en vez de romperse`() {
        assertEquals("cinco mil", Clp.formatear("cinco mil"))
    }

    /** Cada acción del server tiene que tener un verbo, no un "Aceptar". */
    @Test
    fun `el boton nombra la accion`() {
        assertEquals("Sí, registrar el gasto", etiquetaDeConfirmar("registrar_gasto"))
        assertEquals("Sí, ajustar el stock", etiquetaDeConfirmar("ajustar_stock"))
        assertEquals("Sí, cerrar la caja", etiquetaDeConfirmar("cerrar_caja"))
    }

    @Test
    fun `una accion que no conocemos no inventa un verbo equivocado`() {
        // Mejor genérico que mentiroso: "Sí, borrar todo" en una acción que en
        // realidad crea algo sería peor que no decir nada.
        assertEquals("Sí, hazlo", etiquetaDeConfirmar("accion_que_no_existe_todavia"))
    }
}
