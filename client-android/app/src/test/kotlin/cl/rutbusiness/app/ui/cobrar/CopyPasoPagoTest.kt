package cl.rutbusiness.app.ui.cobrar

import cl.rutbusiness.core.pos.MedioDePago
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del paso cómo paga: mesa de feria, no caja de mall.
 *
 * Puro JVM. Si alguien reintroduce "Confirmar transacción", "sistema" o
 * chips en inglés, este test lo atrapa sin montar Compose.
 */
class CopyPasoPagoTest {

    @Test
    fun `feria pregunta como te pagan no como paga lab`() {
        assertEquals("¿Cómo te pagan?", copyComoPaga(feria = true))
        assertEquals("Cómo paga", copyComoPaga(feria = false))
    }

    @Test
    fun `chips feria suenan a mesa hablada no a enum de API`() {
        assertEquals("En efectivo", copyEtiquetaMedio(MedioDePago.Efectivo, feria = true))
        assertEquals("Por transferencia", copyEtiquetaMedio(MedioDePago.Transferencia, feria = true))
        assertEquals("Fiado", copyEtiquetaMedio(MedioDePago.Fiado, feria = true))
        // Retail conserva las etiquetas del enum (ya en español).
        assertEquals(MedioDePago.Efectivo.etiqueta, copyEtiquetaMedio(MedioDePago.Efectivo, feria = false))
        assertEquals(MedioDePago.Transferencia.etiqueta, copyEtiquetaMedio(MedioDePago.Transferencia, feria = false))
        assertEquals(MedioDePago.Fiado.etiqueta, copyEtiquetaMedio(MedioDePago.Fiado, feria = false))
    }

    @Test
    fun `chips no traen ingles ni jerga de red`() {
        val prohibidas = listOf("cash", "credit", "endpoint", "checkout", "submit", "payment")
        MedioDePago.entries.forEach { medio ->
            listOf(true, false).forEach { feria ->
                val s = copyEtiquetaMedio(medio, feria).lowercase()
                prohibidas.forEach { mala ->
                    assertFalse("inglés/jerga en chip ($medio feria=$feria): $s", s.contains(mala))
                }
                assertFalse("IP en chip: $s", s.contains("192.168"))
            }
        }
    }

    @Test
    fun `total pendiente no inventa numero ni dice sistema`() {
        val offlineFeria = copyTotalPendiente(feria = true, hayConexion = false)
        assertTrue(offlineFeria.contains("enviarse") || offlineFeria.contains("envía"))
        assertFalse(offlineFeria.contains("sistema", ignoreCase = true))
        assertFalse(offlineFeria.any { it.isDigit() })

        val onlineSinTotal = copyTotalPendiente(feria = true, hayConexion = true)
        assertTrue(onlineSinTotal.contains("cobrar", ignoreCase = true))
        assertFalse(onlineSinTotal.contains("sistema", ignoreCase = true))
        assertFalse(onlineSinTotal.any { it.isDigit() })
    }

    @Test
    fun `cta feria es verbo de feriante no confirmar transaccion`() {
        assertEquals(
            "Cobrar",
            copyCtaPago(feria = true, cobrando = false, hayConexion = true, medio = MedioDePago.Efectivo),
        )
        assertEquals(
            "Anotar fiado",
            copyCtaPago(feria = true, cobrando = false, hayConexion = true, medio = MedioDePago.Fiado),
        )
        assertEquals(
            "Anotar venta",
            copyCtaPago(feria = true, cobrando = false, hayConexion = false, medio = MedioDePago.Efectivo),
        )
        assertEquals(
            "Cobrando…",
            copyCtaPago(feria = true, cobrando = true, hayConexion = true, medio = MedioDePago.Efectivo),
        )
        assertEquals(
            "Anotando fiado…",
            copyCtaPago(feria = true, cobrando = true, hayConexion = true, medio = MedioDePago.Fiado),
        )

        listOf(
            copyCtaPago(true, false, true, MedioDePago.Efectivo),
            copyCtaPago(true, false, true, MedioDePago.Fiado),
            copyCtaPago(true, false, false, MedioDePago.Efectivo),
            copyCtaPago(false, false, true, MedioDePago.Efectivo),
            copyCtaPago(false, false, false, MedioDePago.Efectivo),
        ).forEach { cta ->
            assertFalse("CTA de lab: $cta", cta.contains("Confirmar", ignoreCase = true))
            assertFalse("CTA de lab: $cta", cta.contains("transacción", ignoreCase = true))
            assertFalse("CTA en inglés: $cta", cta.contains("Submit", ignoreCase = true))
            assertFalse("CTA en inglés: $cta", cta.contains("Checkout", ignoreCase = true))
        }
    }

    @Test
    fun `cta retail offline sigue siendo Guardar venta`() {
        assertEquals(
            "Guardar venta",
            copyCtaPago(feria = false, cobrando = false, hayConexion = false, medio = MedioDePago.Efectivo),
        )
        assertEquals(
            "Anotar fiado",
            copyCtaPago(feria = false, cobrando = false, hayConexion = true, medio = MedioDePago.Fiado),
        )
        assertEquals(
            "Cobrar",
            copyCtaPago(feria = false, cobrando = false, hayConexion = true, medio = MedioDePago.Transferencia),
        )
    }

    @Test
    fun `carrito vacio feria no inventa total ni habla de sistema`() {
        val titulo = copyCarritoVacioTitulo(feria = true)
        val pista = copyCarritoVacioPista(feria = true)
        assertTrue(titulo.contains("Nada", ignoreCase = true) || titulo.contains("anotado", ignoreCase = true))
        assertFalse(pista.contains("sistema", ignoreCase = true))
        assertFalse(pista.contains("computador", ignoreCase = true))
        assertFalse(pista.contains("192.168"))
        assertTrue(
            pista.contains("confirma", ignoreCase = true) ||
                pista.contains("inventa", ignoreCase = true),
        )
    }

    @Test
    fun `vuelto feria no dice sistema ni endpoint`() {
        val feria = copyAyudaVuelto(feria = true)
        assertFalse(feria.contains("sistema", ignoreCase = true))
        assertFalse(feria.contains("endpoint", ignoreCase = true))
        assertTrue(feria.contains("vuelto", ignoreCase = true))
        assertTrue(feria.contains("teléfono", ignoreCase = true) || feria.contains("cobra", ignoreCase = true))
    }

    @Test
    fun `secundario feria es sumar otra cosa`() {
        assertEquals("Sumar otra cosa", copySeguirAgregando(feria = true))
        assertEquals("Seguir agregando", copySeguirAgregando(feria = false))
    }

    @Test
    fun `clientes vacios feria manda al agente no al sistema del negocio`() {
        val pista = copyClientesVaciosPista(feria = true)
        assertFalse(pista.contains("sistema del negocio", ignoreCase = true))
        assertFalse(pista.contains("computador", ignoreCase = true))
        assertTrue(pista.contains("agente", ignoreCase = true))
        assertEquals("¿A quién se lo fías?", copyLabelClienteFiado(feria = true))
    }

    @Test
    fun `precio unitario no es subtotal inventado`() {
        assertEquals("$2.000 c/u", copyPrecioUnitario("$2.000"))
        assertFalse(copyPrecioUnitario("$1.000").contains("total", ignoreCase = true))
    }

    @Test
    fun `etiqueta total feria dice A cobrar`() {
        assertEquals("A cobrar", copyEtiquetaTotal(feria = true))
        assertEquals("Total", copyEtiquetaTotal(feria = false))
    }
}
