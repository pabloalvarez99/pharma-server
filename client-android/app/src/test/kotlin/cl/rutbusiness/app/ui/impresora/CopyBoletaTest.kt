package cl.rutbusiness.app.ui.impresora

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en la tarjeta de impresión (ADR-0022).
 *
 * Mismo criterio que [CopyImpresoraTest], aplicado a la mitad de la tarjeta
 * que antes no lo tenía: la de imprimir y reimprimir sin errores. Feria nunca
 * dice "boleta" -habla del papel del puesto-; retail formal la conserva.
 *
 * Los casos "retail formal" fijan además el literal exacto de siempre: son
 * los mismos textos que [ImpresoraFlujoTest] e [ImpresoraEscalaTest] buscan en
 * pantalla con Compose, y esas pruebas corren sin pack (retail por defecto,
 * ver `packActual()`). Si un literal de acá se mueve un carácter, esas
 * pruebas de Compose se caen.
 */
class CopyBoletaTest {

    @Test
    fun `feria imprimir dice papel y no boleta`() {
        assertEquals("Imprimir el papel", copyBotonImprimir(feria = true))
    }

    @Test
    fun `retail imprimir conserva el literal de siempre`() {
        assertEquals("Imprimir boleta", copyBotonImprimir(feria = false))
    }

    @Test
    fun `feria confirmacion de impresion dice papel y no boleta`() {
        assertEquals("Papel impreso.", copyConfirmacionImpresion(feria = true))
    }

    @Test
    fun `retail confirmacion conserva la frase canonica de siempre`() {
        assertEquals("Boleta impresa.", copyConfirmacionImpresion(feria = false))
    }

    @Test
    fun `feria seguir sin boleta dice papel`() {
        assertEquals("Seguir sin papel", copyBotonSeguirSinBoleta(feria = true))
    }

    @Test
    fun `retail seguir sin boleta conserva el literal de siempre`() {
        assertEquals("Seguir sin boleta", copyBotonSeguirSinBoleta(feria = false))
    }

    @Test
    fun `feria estado sin boleta habla de papel`() {
        val texto = copyEstadoSinBoleta(feria = true)
        assertEquals("Seguiste sin papel. La venta quedó guardada igual.", texto)
        assertFalse(texto.lowercase().contains("boleta"))
    }

    @Test
    fun `retail estado sin boleta conserva el literal de siempre`() {
        assertEquals(
            "Seguiste sin boleta. La venta quedó guardada igual.",
            copyEstadoSinBoleta(feria = false),
        )
    }

    @Test
    fun `feria reimpresion disponible no dice boleta`() {
        val texto = copyReimpresionDisponible(feria = true)
        assertFalse(texto.lowercase().contains("boleta"))
        assertTrue(texto.contains("papel"))
        assertTrue(texto.contains("Reimprimir no vuelve a cobrar"))
    }

    @Test
    fun `retail reimpresion disponible conserva el literal de siempre`() {
        assertEquals(
            "Sale la misma boleta de la última venta. Reimprimir no vuelve a cobrar.",
            copyReimpresionDisponible(feria = false),
        )
    }

    @Test
    fun `feria sin reimpresion disponible no dice boleta`() {
        val texto = copySinReimpresionDisponible(feria = true)
        assertFalse(texto.lowercase().contains("boleta"))
        assertTrue(texto.contains("ningún papel"))
    }

    @Test
    fun `retail sin reimpresion disponible conserva el prefijo de siempre`() {
        // ImpresoraFlujoTest busca este prefijo como substring en pantalla.
        assertTrue(
            copySinReimpresionDisponible(feria = false)
                .startsWith("Todavía no imprimiste ninguna boleta desde este teléfono."),
        )
    }

    /**
     * Barrido general: ningún copy de feria de este archivo puede colar la
     * palabra "boleta". Es la misma regla que ya prueba [CopyImpresoraTest]
     * para las fallas; acá cubre el camino sin errores.
     */
    @Test
    fun `ningun copy de feria de esta tarjeta dice boleta`() {
        val copiesFeria = listOf(
            copyBotonImprimir(feria = true),
            copyConfirmacionImpresion(feria = true),
            copyBotonSeguirSinBoleta(feria = true),
            copyEstadoSinBoleta(feria = true),
            copyReimpresionDisponible(feria = true),
            copySinReimpresionDisponible(feria = true),
        )
        copiesFeria.forEach { texto ->
            assertFalse("«$texto» no debería decir boleta", texto.lowercase().contains("boleta"))
        }
    }
}
