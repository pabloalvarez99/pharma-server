package cl.rutbusiness.app.ui.scanner

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * El filtro que decide cuántas veces se le cobra un producto al cliente.
 *
 * Se prueba en la JVM y sin cámara porque el error que evita no es de cámara:
 * es que un tarro apoyado en el mostrador entre treinta veces al carrito. La
 * cámara sólo lo reporta; contarlo bien es de acá.
 */
class AntiReboteTest {

    private val ventana = AntiRebote.VENTANA_MS

    @Test
    fun `el primer codigo entra`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801234567890", 0))
    }

    /** El caso real: el producto quieto frente al lente, frame tras frame. */
    @Test
    fun `un producto quieto se cuenta una sola vez`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801234567890", 0))

        var t = 0L
        var aceptados = 0
        // Tres segundos a 30 fps: mucho más que la ventana, pero sin que el
        // código deje de verse un solo frame.
        repeat(90) {
            t += 33
            if (filtro.aceptar("7801234567890", t)) aceptados++
        }

        assertTrue(
            "el mismo producto sin salir de cuadro entró $aceptados veces de más",
            aceptados == 0,
        )
    }

    /** Retirarlo y volver a pasarlo es cómo se cargan dos unidades iguales. */
    @Test
    fun `sacarlo de cuadro y volver a pasarlo cuenta de nuevo`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801234567890", 0))
        assertTrue(filtro.aceptar("7801234567890", ventana))
    }

    @Test
    fun `justo antes de cumplirse la ventana todavia no cuenta`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801234567890", 0))
        assertFalse(filtro.aceptar("7801234567890", ventana - 1))
    }

    /** Dos productos a la vista no se desbloquean entre ellos. */
    @Test
    fun `alternar dos codigos no duplica ninguno`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801111111111", 0))
        assertTrue(filtro.aceptar("7802222222222", 30))
        assertFalse("el primero volvió a entrar por culpa del segundo", filtro.aceptar("7801111111111", 60))
        assertFalse(filtro.aceptar("7802222222222", 90))
    }

    /**
     * El producto que se crea desde la caja ya entró al carrito, y sigue frente
     * al lente: la cámara no puede contarlo de nuevo.
     */
    @Test
    fun `anotar evita que lo recien creado se cuente dos veces`() {
        val filtro = AntiRebote()
        filtro.anotar("7801234567890", 0)
        assertFalse(filtro.aceptar("7801234567890", 200))
        // Y cuando el producto sale de cuadro, vuelve a contar como siempre.
        assertTrue(filtro.aceptar("7801234567890", ventana + 200))
    }

    @Test
    fun `olvidar deja todo como al abrir la camara`() {
        val filtro = AntiRebote()
        assertTrue(filtro.aceptar("7801234567890", 0))
        filtro.olvidar()
        assertTrue(filtro.aceptar("7801234567890", 10))
    }

    /**
     * La tabla interna no puede crecer con cada producto de una venta larga: el
     * aparato objetivo tiene 1-2 GB y esto vive mientras dura el turno.
     */
    @Test
    fun `una venta larga no acumula codigos viejos`() {
        val filtro = AntiRebote()
        var t = 0L
        repeat(500) { i ->
            t += ventana
            filtro.aceptar(codigoNumero(i), t)
        }
        // Después de la última purga sólo puede quedar lo visto dentro de la
        // ventana, o sea el último código.
        assertTrue(filtro.aceptar(codigoNumero(0), t + ventana))
    }

    private fun codigoNumero(i: Int): String = "78" + i.toString().padStart(11, '0')
}
