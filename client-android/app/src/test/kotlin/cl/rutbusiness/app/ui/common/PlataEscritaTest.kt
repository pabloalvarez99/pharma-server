package cl.rutbusiness.app.ui.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Lo que la dueña escribe en un campo de plata, en el camino de ida al server.
 *
 * Estas son las tres funciones que comparten la caja y el fiado. Si alguna deja
 * pasar basura, el server recibe un decimal que no puede leer y la pantalla
 * muestra un error donde había un monto perfectamente escrito.
 */
class PlataEscritaTest {

    // --- lo que se deja tipear -----------------------------------------------

    /**
     * El teclado de un fabricante ofrece símbolos aunque se le pida numérico, y
     * un `$` pegado al monto llega al server como decimal inválido.
     */
    @Test
    fun `se filtra todo lo que no es un monto`() {
        assertEquals("15000", soloPlata("$15000"))
        assertEquals("15000", soloPlata("15 000"))
        assertEquals("15.000", soloPlata("15.000 pesos"))
        assertEquals("1250", soloPlata("-1250"))
    }

    @Test
    fun `los dos separadores pasan porque los dos se tipean`() {
        assertEquals("12,50", soloPlata("12,50"))
        assertEquals("12.50", soloPlata("12.50"))
    }

    // --- lo que viaja --------------------------------------------------------

    /** `rust_decimal` sólo lee punto; la coma es lo que está a mano en el teclado. */
    @Test
    fun `la coma se traduce a punto`() {
        assertEquals("12.50", montoParaElServidor("12,50"))
    }

    /**
     * Ni redondeo ni cambio de escala: los dígitos que viajan son los que se
     * tipearon. Es lo que permite que la moneda del tenant tenga 0 o 2 decimales
     * sin que estas pantallas sepan cuál es.
     */
    @Test
    fun `el monto viaja digito por digito como se escribio`() {
        assertEquals("15000", montoParaElServidor("15000"))
        assertEquals("0", montoParaElServidor("0"))
        assertEquals("1490.00", montoParaElServidor("1490.00"))
    }

    /** Un cero inventado se cobra: lo que no se entiende vuelve `null`. */
    @Test
    fun `lo que no es un decimal no se convierte en cero`() {
        assertNull(montoParaElServidor(""))
        assertNull(montoParaElServidor("   "))
        assertNull(montoParaElServidor("."))
        assertNull(montoParaElServidor("1.2.3"))
    }

    // --- mayor que cero ------------------------------------------------------

    @Test
    fun `mover cero pesos no es mover plata`() {
        assertFalse(esMasQueCero("0"))
        assertFalse(esMasQueCero("0.00"))
        assertFalse(esMasQueCero(""))
        assertFalse(esMasQueCero("no es plata"))
        assertTrue(esMasQueCero("1"))
        assertTrue(esMasQueCero("0,01"))
    }
}
