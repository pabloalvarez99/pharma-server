package cl.rutbusiness.app.ui.fiado

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de la hoja de cuenta de una persona (`DetalleDeCuenta.kt`).
 *
 * Puro JVM: si alguien reintroduce «balance», «transacción», «registro» o
 * «ítem» -- jerga que la señora de feria no dice -- este test lo atrapa.
 */
class CopyCuentaTest {

    private val palabrasProhibidas = listOf(
        "balance",
        "transacción",
        "transaccion",
        "registro",
        "ítem",
        "item",
        "cuenta corriente",
        "sistema",
        "cajón",
        "cajon",
        "computador",
        "saldo",
    )

    @Test
    fun `copy de la hoja nunca usa jerga tecnica`() {
        for (copy in todoCopyCuentaUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasProhibidas) {
                assertFalse(
                    "copy de la hoja no puede decir «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    @Test
    fun `desde cuando se debe usa la voz de feria vs formal`() {
        assertEquals(
            "Te debe desde el 16-08-2026",
            debeDesde(feria = true, fecha = "16-08-2026"),
        )
        assertEquals(
            "Debe desde el 16-08-2026",
            debeDesde(feria = false, fecha = "16-08-2026"),
        )
    }

    @Test
    fun `quedo al dia se siente bien y no como un cero vacio`() {
        assertEquals("Quedó al día", chipAlDia())

        val feria = mensajeAlDia(feria = true)
        assertTrue(feria.contains("pagó"))
        assertTrue(feria.contains("Te"))
        assertFalse(feria.lowercase().contains("saldo"))

        val formal = mensajeAlDia(feria = false)
        assertTrue(formal.contains("pagó"))
        assertFalse(formal.lowercase().contains("saldo"))
    }

    @Test
    fun `hoja nueva no es un vacio sin nombre`() {
        assertEquals("Hoja nueva", chipHojaNueva())
    }
}
