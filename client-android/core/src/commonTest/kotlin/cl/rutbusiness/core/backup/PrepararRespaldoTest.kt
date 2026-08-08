package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class PrepararRespaldoTest {

    private fun venta(clave: String) = VentaEnCola(
        clave = clave,
        solicitud = SolicitudDeVenta(
            items = listOf(
                LineaDeVenta(
                    product = "p",
                    productName = "tomate",
                    quantity = 1,
                    unitPrice = "1000",
                ),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = 1L,
        lineas = 1,
    )

    @Test
    fun `cola vacia prepara mensaje honesto sin contenido`() {
        val r = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = emptyList(),
            createdAtUnix = 100L,
            rubro = "feria",
        ).getOrThrow()
        assertFalse(r.hayContenido)
        assertEquals(0, r.ventasEnCola)
        assertTrue(r.mensaje.contains("No hay ventas", ignoreCase = true))
    }

    @Test
    fun `con ventas empaqueta y no promete nube si no hay cifrado`() {
        val r = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta("a"), venta("b")),
            createdAtUnix = 100L,
            rubro = "feria",
            cifradoListo = false,
        ).getOrThrow()
        assertTrue(r.hayContenido)
        assertEquals(2, r.ventasEnCola)
        assertTrue(r.bytesPlaintext > 0)
        assertTrue(r.mensaje.contains("no se sube", ignoreCase = true))
        assertEquals(2, r.snapshot.pendingSales.size)
        assertEquals(null, r.sobre)
    }

    @Test
    fun `cifrado listo cambia el copy`() {
        val r = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta("a")),
            createdAtUnix = 100L,
            cifradoListo = true,
        ).getOrThrow()
        assertTrue(r.mensaje.contains("cifrar", ignoreCase = true))
    }

    @Test
    fun `con clave aes cifra de verdad`() {
        val key = ByteArray(32) { 7 }
        val r = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta("a")),
            createdAtUnix = 100L,
            claveAes32 = key,
        ).getOrThrow()
        val sobre = r.sobre
        assertTrue(sobre != null)
        assertTrue(r.mensaje.contains("Cifrado listo", ignoreCase = true))
        val plain = descifrarSobreV1(key, sobre!!.envelopeBytes).getOrThrow()
        val snap = desempaquetarSnapshot(plain).getOrThrow()
        assertEquals(1, snap.pendingSales.size)
    }
}
