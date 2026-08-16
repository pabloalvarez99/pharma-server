package cl.rutbusiness.app.ui.offline

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en la cola (ADR-0022) + tono de recado offline.
 *
 * Puro JVM: el feriante no ve "sistema" ni "computador"; sin conexión se dice
 * como recado, no como error de red. Rechazo sin motivo y pie de fila también.
 */
class CopyColaTest {

    @Test
    fun `feria cola vacia no habla del sistema del negocio ni computador`() {
        val hint = hintColaVacia(feria = true)
        assertEquals("Todas las ventas que cobraste ya se anotaron.", hint)
        assertFalse(
            "feria no menciona sistema del negocio",
            hint.lowercase().contains("sistema del negocio"),
        )
        assertFalse(
            "feria no menciona computador",
            hint.lowercase().contains("computador"),
        )
    }

    @Test
    fun `farmacia cola vacia conserva el string exacto`() {
        assertEquals(
            "Todas las ventas que cobraste ya llegaron al sistema del negocio.",
            hintColaVacia(feria = false),
        )
    }

    @Test
    fun `sin conexion se dice como recado, no como error de red`() {
        val titulo = recadoSinConexion()
        val detalle = detalleSinConexion()
        assertEquals("Sin conexión", titulo)
        assertEquals("Ves lo último que se cargó.", detalle)

        val junto = "$titulo $detalle".lowercase()
        assertFalse(junto, junto.contains("error"))
        assertFalse(junto, junto.contains("falló") || junto.contains("fallo"))
        assertFalse(junto, junto.contains("problema"))
        assertFalse(junto, junto.contains("offline"))
        assertFalse(junto, junto.contains("computador"))
        assertFalse(junto, junto.contains("192.168"))
        assertTrue(junto, junto.contains("conexión") || junto.contains("conexion"))
    }

    @Test
    fun `feria rechazo sin motivo no dice sistema ni computador`() {
        val motivo = motivoRechazoSinDetalle(feria = true)
        assertEquals("No se pudo anotar en tu día.", motivo)
        assertFalse("feria no dice sistema", motivo.lowercase().contains("sistema"))
        assertFalse("feria no dice computador", motivo.lowercase().contains("computador"))
        assertFalse("feria no dice 192.168", motivo.contains("192.168"))
    }

    @Test
    fun `retail rechazo sin motivo conserva el string exacto`() {
        assertEquals(
            "El sistema no la aceptó.",
            motivoRechazoSinDetalle(feria = false),
        )
    }

    @Test
    fun `feria ayuda de rechazo dice telefono y no sistema`() {
        val ayuda = ayudaVentaRechazada(feria = true)
        assertTrue(ayuda, ayuda.lowercase().contains("teléfono") || ayuda.lowercase().contains("telefono"))
        assertFalse("feria no dice sistema", ayuda.lowercase().contains("sistema"))
        assertFalse("feria no dice computador", ayuda.lowercase().contains("computador"))
        assertTrue(ayuda, ayuda.lowercase().contains("reintentar"))
    }

    @Test
    fun `retail ayuda de rechazo conserva el sentido sin inventar montos`() {
        val ayuda = ayudaVentaRechazada(feria = false)
        assertEquals(
            "No se va a reintentar sola. Revisa qué pasó y vuelve a cobrarla si " +
                "corresponde; recién ahí descártala de acá.",
            ayuda,
        )
        assertFalse(ayuda, ayuda.contains("$"))
    }

    @Test
    fun `feria pie de fila no nombra sistema`() {
        val pie = detalleLineasCola(lineas = 2, unidades = 3, feria = true)
        assertEquals(
            "2 productos · 3 unidades · el total se confirma cuando se anote",
            pie,
        )
        assertFalse("feria no dice sistema", pie.lowercase().contains("sistema"))
        assertFalse("feria no dice computador", pie.lowercase().contains("computador"))
        assertFalse("sin montos inventados", pie.contains("$"))
    }

    @Test
    fun `retail pie de fila conserva el string que mide OfflineEscala`() {
        assertEquals(
            "2 productos · 3 unidades · el total lo confirma el sistema al recibirla",
            detalleLineasCola(lineas = 2, unidades = 3, feria = false),
        )
    }

    @Test
    fun `etiquetas de rechazo se leen como cuaderno`() {
        assertEquals("No se anotó", etiquetaNoSeAnoto())
        assertEquals("Descartar", etiquetaDescartar())
    }
}
