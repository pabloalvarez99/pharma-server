package cl.rutbusiness.app.ui.gente

import cl.rutbusiness.core.money.Moneda
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * El texto que sale hacia la gente, probado sin `Activity`.
 *
 * Es todo el punto de que [TextoParaGente] sea puro: el mensaje que le llega a
 * Don Juan se puede afirmar acá, en milisegundos y sin Robolectric, en vez de
 * mirarlo a ojo en un teléfono. Lo que Android hace después —abrir el
 * selector— no cambia ni un carácter.
 */
class TextoParaGenteTest {

    // --- deuda ---------------------------------------------------------------

    @Test
    fun `el recordatorio nombra a la persona y el monto que se ve en pantalla`() {
        assertEquals(
            "Hola Don Juan, me debes $5.000.",
            mensajeDeuda(nombre = "Don Juan", monto = "$5.000"),
        )
    }

    @Test
    fun `sin nombre el saludo no queda con una coma suelta`() {
        // Alguien fiado sin nombre completo existe: se anota "el de la esquina"
        // o directamente nada. "Hola , me debes" delata que el texto lo armó una
        // máquina, y eso es lo que hace que no se mande.
        assertEquals("Hola, me debes $5.000.", mensajeDeuda(nombre = "", monto = "$5.000"))
        assertEquals("Hola, me debes $5.000.", mensajeDeuda(nombre = "   ", monto = "$5.000"))
    }

    @Test
    fun `el nombre llega recortado porque se escribe a mano en el mostrador`() {
        assertEquals(
            "Hola Rosa, me debes $1.200.",
            mensajeDeuda(nombre = "  Rosa  ", monto = "$1.200"),
        )
    }

    @Test
    fun `con pesos enteros se escribe es-CL, miles con punto`() {
        assertEquals("Hola Juan, me debes $5.000.", mensajeDeuda("Juan", 5_000L))
        assertEquals("Hola Juan, me debes $1.234.567.", mensajeDeuda("Juan", 1_234_567L))
        assertEquals("Hola Juan, me debes $999.", mensajeDeuda("Juan", 999L))
        assertEquals("Hola Juan, me debes $0.", mensajeDeuda("Juan", 0L))
    }

    @Test
    fun `el signo va antes del simbolo y no mete un punto de mas`() {
        // Agrupar con el "-" adentro da "-.1.234": el separador se cuela justo
        // después del signo en cuanto el número pasa de cuatro dígitos.
        assertEquals("-$1.234", pesosEsCl(-1_234L))
        assertEquals("-$999", pesosEsCl(-999L))
    }

    @Test
    fun `el monto no se recalcula, se usa el que ya mostro la pantalla`() {
        // La regla de plata del proyecto: el monto viene del server como texto y
        // lo escribe la Moneda del tenant. Un negocio en soles manda el mensaje
        // en soles sin que este archivo sepa que existen los soles.
        val soles = Moneda.de("PEN")
        val texto = mensajeDeuda(nombre = "Rosa", monto = soles.formatear("1234.50"))

        assertTrue(texto, texto.contains("1.234,50"))
        assertFalse("no puede aparecer el peso chileno", texto.contains("$1.234"))
    }

    // --- el día --------------------------------------------------------------

    @Test
    fun `el dia se cuenta con el resumen ya formateado`() {
        assertEquals(
            "Hoy en el negocio: $45.000 en 12 boletas",
            mensajeHoy("$45.000 en 12 boletas"),
        )
    }

    @Test
    fun `sin resumen no se inventa un numero`() {
        // El modo de fallar que esto evita: un "Hoy en el negocio: $0" mandado a
        // la casa un día que el server no contestó. El cero se cree.
        assertEquals("Hoy en el negocio.", mensajeHoy(""))
        assertEquals("Hoy en el negocio.", mensajeHoy("   "))
    }

    @Test
    fun `el resumen no arrastra espacios del borde`() {
        assertEquals("Hoy en el negocio: $45.000 en 1 boleta", mensajeHoy("  $45.000 en 1 boleta "))
    }
}
