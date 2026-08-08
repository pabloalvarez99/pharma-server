package cl.rutbusiness.core.offline

import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest

/**
 * La cola tiene una sola promesa: **una venta cobrada no se pierde y no se
 * cobra dos veces**. Todo lo que se prueba acá es alguna cara de eso.
 */
class ColaDeVentasTest {

    private var reloj = 1_000_000L

    private fun venta(clave: String) = VentaEnCola(
        clave = clave,
        solicitud = SolicitudDeVenta(
            items = listOf(
                LineaDeVenta(
                    product = "product:x",
                    productName = "Arroz",
                    quantity = 2,
                    unitPrice = "1490",
                ),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = reloj,
        lineas = 1,
    )

    @Test
    fun `la venta sobrevive a que la app se cierre`() = runTest {
        val disco = AlmacenDeMentira()
        val antes = ColaDeVentas(disco) { reloj }
        antes.cargar()
        assertTrue(antes.encolar(venta("abc")))

        // El proceso muere y la app se abre de nuevo: mismo disco, cola nueva.
        val despues = ColaDeVentas(disco.trasReiniciarLaApp()) { reloj }
        despues.cargar()

        assertEquals(1, despues.ventas.value.size)
        assertEquals("abc", despues.ventas.value.first().clave)
        // Y la clave de idempotencia es la misma, que es lo que hace que el
        // reintento después del reinicio no cobre dos veces.
        assertEquals("abc", despues.proxima()?.clave)
    }

    @Test
    fun `queda en disco antes de que nadie intente mandarla`() = runTest {
        val disco = AlmacenDeMentira()
        val cola = ColaDeVentas(disco) { reloj }
        cola.cargar()

        assertEquals(0, disco.escrituras)
        cola.encolar(venta("abc"))
        // Encolar escribe. Si escribiera después del primer intento, el hueco
        // entre el POST y la falla sería el momento en que se pierde la venta.
        assertEquals(1, disco.escrituras)
    }

    @Test
    fun `la misma clave no entra dos veces`() = runTest {
        val cola = ColaDeVentas(AlmacenDeMentira()) { reloj }
        cola.cargar()

        assertTrue(cola.encolar(venta("abc")))
        assertTrue(cola.encolar(venta("abc")))

        // Un doble toque en "Guardar venta" es una sola venta.
        assertEquals(1, cola.ventas.value.size)
    }

    @Test
    fun `la espera crece y se planta en cinco minutos`() {
        assertEquals(5_000L, ColaDeVentas.esperaTras(1))
        assertEquals(10_000L, ColaDeVentas.esperaTras(2))
        assertEquals(20_000L, ColaDeVentas.esperaTras(3))
        assertEquals(40_000L, ColaDeVentas.esperaTras(4))
        assertEquals(80_000L, ColaDeVentas.esperaTras(5))
        assertEquals(160_000L, ColaDeVentas.esperaTras(6))
        // De acá para arriba no sube más: reintentar en loop apretado quema
        // batería y megas de datos que la dueña paga, y no llega antes.
        assertEquals(ColaDeVentas.ESPERA_MAXIMA_MS, ColaDeVentas.esperaTras(7))
        assertEquals(ColaDeVentas.ESPERA_MAXIMA_MS, ColaDeVentas.esperaTras(50))
    }

    @Test
    fun `postergar corre el turno y apurar lo devuelve`() = runTest {
        val cola = ColaDeVentas(AlmacenDeMentira()) { reloj }
        cola.cargar()
        cola.encolar(venta("abc"))

        cola.postergar("abc")
        assertNull(cola.proxima(), "recién postergada no le toca todavía")

        cola.apurar()
        assertNotNull(cola.proxima(), "el botón 'Intentar ahora' borra la espera")
    }

    @Test
    fun `una venta rechazada deja de intentarse pero sigue a la vista`() = runTest {
        val cola = ColaDeVentas(AlmacenDeMentira()) { reloj }
        cola.cargar()
        cola.encolar(venta("abc"))

        cola.rechazar("abc", "No queda stock de Arroz.")

        assertNull(cola.proxima(), "no se reintenta sola")
        assertEquals(0, cola.cuantasEsperan)
        // Pero sigue en la lista: una venta que desaparece sin que nadie la
        // mire es plata que nadie supo que se perdió.
        assertEquals(1, cola.ventas.value.size)
        assertEquals("No queda stock de Arroz.", cola.ventas.value.first().motivo)
    }

    @Test
    fun `solo se puede descartar una rechazada`() = runTest {
        val cola = ColaDeVentas(AlmacenDeMentira()) { reloj }
        cola.cargar()
        cola.encolar(venta("esperando"))
        cola.encolar(venta("rechazada"))
        cola.rechazar("rechazada", "El cliente ya no existe.")

        cola.descartar("esperando")
        cola.descartar("rechazada")

        assertEquals(
            listOf("esperando"),
            cola.ventas.value.map { it.clave },
            "una venta que todavía se puede mandar no se borra desde ninguna pantalla",
        )
    }

    @Test
    fun `la cola llena avisa en vez de tragarse la venta`() = runTest {
        val cola = ColaDeVentas(AlmacenDeMentira()) { reloj }
        cola.cargar()
        repeat(ColaDeVentas.MAXIMO) { assertTrue(cola.encolar(venta("clave-$it"))) }

        assertFalse(
            cola.encolar(venta("la-que-sobra")),
            "llegar al tope tiene que decirse, no acumularse en silencio",
        )
        assertEquals(ColaDeVentas.MAXIMO, cola.ventas.value.size)
    }
}
