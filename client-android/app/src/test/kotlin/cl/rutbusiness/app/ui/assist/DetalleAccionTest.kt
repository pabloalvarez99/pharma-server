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
