package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SnapshotBackupTest {

    private fun ventaDemo(): VentaEnCola = VentaEnCola(
        clave = "idem-1",
        solicitud = SolicitudDeVenta(
            items = listOf(
                LineaDeVenta(
                    product = "p1",
                    productName = "tomate",
                    quantity = 2,
                    unitPrice = "1000",
                ),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = 1_700_000_000_000L,
        lineas = 1,
    )

    @Test
    fun `empaquetar y desempaquetar roundtrip`() {
        val s = armarSnapshotDesdeCola(
            tenantId = "puesto-rosa",
            createdAtUnix = 1_700_000_000L,
            pendingSales = listOf(ventaDemo()),
            rubro = "feria",
        )
        assertNull(validarSnapshot(s))
        val bytes = empaquetarSnapshot(s).getOrThrow()
        assertTrue(bytes.isNotEmpty())
        val back = desempaquetarSnapshot(bytes).getOrThrow()
        assertEquals(SNAPSHOT_VERSION, back.snapshotVersion)
        assertEquals("puesto-rosa", back.tenantId)
        assertEquals("feria", back.rubro)
        assertEquals(1, back.pendingSales.size)
        assertEquals("tomate", back.pendingSales[0].solicitud.items[0].productName)
    }

    @Test
    fun `rechaza tenant vacio y version mala`() {
        val malo = SnapshotBackupV1(
            snapshotVersion = 99,
            createdAtUnix = 1,
            tenantId = "x",
        )
        assertNotNull(validarSnapshot(malo))
        val vacio = SnapshotBackupV1(
            createdAtUnix = 1,
            tenantId = "  ",
        )
        assertNotNull(validarSnapshot(vacio))
        assertTrue(empaquetarSnapshot(vacio).isFailure)
    }

    @Test
    fun `texto imprimible tiene palabras y payload qr`() {
        val clave = claveDeDemostracion()
        val texto = textoTarjetaImprimible(clave, "Puesto-Rosa")
        assertTrue(texto.contains("tarjeta de rescate", ignoreCase = true))
        assertTrue(texto.contains("1. "))
        assertTrue(texto.contains(clave.palabras[0]))
        assertTrue(texto.contains("rutbusiness-rescue:v1:puesto-rosa:"))
        assertTrue(texto.contains("No mandes esto por WhatsApp"))
        // La frase completa junta no debe ser la única forma: numerada sí.
        assertTrue(texto.lines().any { it.trim().startsWith("1.") })
    }
}
