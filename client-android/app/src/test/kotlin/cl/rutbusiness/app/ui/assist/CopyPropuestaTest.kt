package cl.rutbusiness.app.ui.assist

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de la tarjeta de propuesta: el momento en que la dueña lee qué
 * entendió el agente antes de que se anote algo. Si acá se cuela jerga técnica
 * o de admin, o si un "cancelar" deja de sonar tranquilo, este test lo atrapa.
 */
class CopyPropuestaTest {

    @Test
    fun `encabezado es honesto y no dice confirmacion`() {
        val encabezado = copyEncabezado()
        assertEquals("Antes de hacerlo, revisa", encabezado)
        assertFalse(encabezado.contains("confirmación", ignoreCase = true))
        assertFalse(encabezado.contains("confirmacion", ignoreCase = true))
    }

    @Test
    fun `cargando cambia de feria a retail`() {
        assertEquals("Anotándolo…", copyCargando(feria = true))
        assertEquals("Guardándolo…", copyCargando(feria = false))
    }

    @Test
    fun `cierre de cancelada tranquiliza que no cambio nada en el negocio`() {
        val feria = copyCierreCancelada(feria = true)
        val retail = copyCierreCancelada(feria = false)
        assertEquals("Listo, no lo anoté. En tu puesto no cambió nada.", feria)
        assertEquals("No lo hice. No cambió nada en tu negocio.", retail)
        assertTrue(feria.contains("no cambió nada", ignoreCase = true))
        assertTrue(retail.contains("no cambió nada", ignoreCase = true))
    }

    @Test
    fun `boton de volver a pedir suena a puesto en feria y a tramite en retail`() {
        assertEquals("Decírmelo de nuevo", copyBotonVolverAPedir(feria = true))
        assertEquals("Pedirlo de nuevo", copyBotonVolverAPedir(feria = false))
    }

    @Test
    fun `boton de confirmar en feria es una sola frase hablada sin importar la accion`() {
        val nombres = listOf("registrar_gasto", "ajustar_stock", "accion_que_no_existe_todavia")
        nombres.forEach { nombre ->
            assertEquals("Sí, anotá eso", etiquetaDeConfirmar(nombre, feria = true))
        }
    }

    @Test
    fun `boton de confirmar en retail nombra la accion exacta del server`() {
        assertEquals("Sí, registrar el gasto", etiquetaDeConfirmar("registrar_gasto"))
        assertEquals("Sí, registrar el abono", etiquetaDeConfirmar("registrar_abono"))
        assertEquals("Sí, crear el cliente", etiquetaDeConfirmar("crear_cliente"))
        assertEquals("Sí, crear el proveedor", etiquetaDeConfirmar("crear_proveedor"))
        assertEquals("Sí, crear el producto", etiquetaDeConfirmar("crear_producto_rapido"))
        assertEquals("Sí, cambiar el precio", etiquetaDeConfirmar("ajustar_precio"))
        assertEquals("Sí, ajustar el stock", etiquetaDeConfirmar("ajustar_stock"))
        assertEquals("Sí, abrir la caja", etiquetaDeConfirmar("abrir_caja"))
        assertEquals("Sí, cerrar la caja", etiquetaDeConfirmar("cerrar_caja"))
        assertEquals("Sí, crear la orden de compra", etiquetaDeConfirmar("crear_orden_compra_draft"))
        assertEquals("Sí, dispensar la receta", etiquetaDeConfirmar("dispensar_receta"))
        assertEquals("Sí, registrar la venta", etiquetaDeConfirmar("registrar_venta"))
        assertEquals("Sí, registrar el fiado", etiquetaDeConfirmar("registrar_fiado"))
    }

    @Test
    fun `una accion desconocida en retail no inventa un verbo`() {
        // Mejor genérico que mentiroso: inventar "Sí, borrar todo" en una
        // acción que en realidad crea algo sería peor que no decir nada.
        assertEquals("Sí, hazlo", etiquetaDeConfirmar("accion_que_no_existe_todavia"))
    }

    @Test
    fun `boton de cancelar es un no sin culpa, nunca Cancelar ni Rechazar`() {
        val feria = etiquetaDeCancelar(feria = true)
        val retail = etiquetaDeCancelar(feria = false)
        assertEquals("Mejor no", feria)
        assertEquals("No, déjalo así", retail)
        listOf(feria, retail).forEach { texto ->
            assertFalse(texto.contains("cancelar", ignoreCase = true))
            assertFalse(texto.contains("rechazar", ignoreCase = true))
        }
    }

    @Test
    fun `vencimiento se dice en palabras y nunca en numeros crudos`() {
        assertEquals(
            "Ya se me pasó el tiempo para anotarlo.",
            vencimientoEnPalabras(segundosRestantes = 0, feria = true),
        )
        assertEquals(
            "Queda poquito para anotarlo.",
            vencimientoEnPalabras(segundosRestantes = 30, feria = true),
        )
        assertEquals(
            "Tienes unos minutos para decirme que sí.",
            vencimientoEnPalabras(segundosRestantes = 120, feria = true),
        )
        // Retail delega en Vencimiento.enPalabras: mismos tres tramos.
        assertEquals(
            Vencimiento.enPalabras(0),
            vencimientoEnPalabras(segundosRestantes = 0, feria = false),
        )
        assertEquals(
            "Tienes unos minutos para confirmarla.",
            vencimientoEnPalabras(segundosRestantes = 120, feria = false),
        )

        listOf(0L, 30L, 45L, 46L, 120L).forEach { segundos ->
            listOf(true, false).forEach { feria ->
                val texto = vencimientoEnPalabras(segundos, feria)
                assertFalse(
                    "«$texto» no debería mostrar un número crudo de segundos",
                    texto.any(Char::isDigit),
                )
            }
        }
    }

    @Test
    fun `ningun copy de la tarjeta usa jerga tecnica ni de admin`() {
        todoCopyPropuestaUsuario().forEach(::assertSinJergaTecnica)
    }

    /**
     * Palabras de log/desarrollador o de ERP que nunca le tocan la pantalla a
     * la dueña. Incluye las prohibidas del brief de la ola ("acción",
     * "payload", "intent", "parsear", "ejecutar") más las de `CopyEscanerTest`.
     */
    private fun assertSinJergaTecnica(frase: String) {
        val jerga = listOf(
            "sku", "buffer", "driver", "endpoint", "socket", "petición",
            "request", "response", "null", "timeout", "backend", "api",
            "acción", "accion", "payload", "intent", "parsear", "ejecutar",
        )
        jerga.forEach { palabra ->
            assertFalse(
                "\"$frase\" no debería contener la palabra técnica \"$palabra\"",
                frase.contains(palabra, ignoreCase = true),
            )
        }
    }
}
