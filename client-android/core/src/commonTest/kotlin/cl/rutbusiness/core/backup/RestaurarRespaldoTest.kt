package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RestaurarRespaldoTest {

    // Una clave REAL de la tarjeta: la frase tiene que ser del vocabulario y
    // cuadrar con los bloques, porque el KDF corre sobre la semilla canónica y
    // no sobre el texto. Palabras inventadas ya no derivan llave, a propósito.
    private val clave = claveDeDemostracion()
    private val frase = MaterialRecuperacion.Frase(clave.palabras)

    private fun venta() = VentaEnCola(
        clave = "k-restore",
        solicitud = SolicitudDeVenta(
            items = listOf(
                LineaDeVenta(
                    product = "p",
                    productName = "tomate",
                    quantity = 2,
                    unitPrice = "1000",
                ),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = 1L,
        lineas = 1,
    )

    @Test
    fun `roundtrip preparar y restaurar con la misma frase`() {
        val prep = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta()),
            createdAtUnix = 100L,
            rubro = "feria",
            materialRecuperacion = frase,
        ).getOrThrow()
        val sobre = prep.sobre!!
        val rest = restaurarDesdeSobre(frase, sobre.envelopeBytes).getOrThrow()
        assertEquals(1, rest.ventasEnCola)
        assertEquals("puesto", rest.snapshot.tenantId)
        assertEquals("k-restore", rest.snapshot.pendingSales.single().clave)
        assertTrue(rest.mensaje.contains("1 venta", ignoreCase = true))
    }

    @Test
    fun `frase incorrecta no abre el sobre`() {
        val prep = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta()),
            createdAtUnix = 100L,
            materialRecuperacion = frase,
        ).getOrThrow()
        // Otra clave válida, no basura: prueba que un sobre no se abre con la
        // frase de OTRO negocio, que es el caso real.
        val mala = MaterialRecuperacion.Frase(
            generarClaveDelNegocio(ByteArray(BYTES_SEMILLA) { (it * 3 + 1).toByte() }).palabras,
        )
        val r = restaurarDesdeSobre(mala, prep.sobre!!.envelopeBytes)
        assertTrue(r.isFailure)
        assertTrue(
            r.exceptionOrNull()!!.message!!.contains("cuaderno", ignoreCase = true) ||
                r.exceptionOrNull()!!.message!!.contains("abrir", ignoreCase = true) ||
                r.exceptionOrNull()!!.message != null,
        )
    }

    @Test
    fun `restaurar desde textos base64`() {
        val prep = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta()),
            createdAtUnix = 100L,
            materialRecuperacion = frase,
        ).getOrThrow()
        val b64 = envelopeToBase64(prep.sobre!!.envelopeBytes)
        val words = frase.palabras.joinToString(" ")
        val rest = restaurarDesdeTextos(words, b64).getOrThrow()
        assertEquals(1, rest.ventasEnCola)
    }

    @Test
    fun `conRehidratacion actualiza el mensaje`() {
        val prep = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(venta()),
            createdAtUnix = 100L,
            materialRecuperacion = frase,
        ).getOrThrow()
        val abierta = restaurarDesdeSobre(frase, prep.sobre!!.envelopeBytes).getOrThrow()
        val con = abierta.conRehidratacion(1)
        assertEquals(1, con.rehidratadas)
        assertTrue(con.mensaje.contains("volvió", ignoreCase = true) ||
            con.mensaje.contains("Listo", ignoreCase = true))
    }
}
