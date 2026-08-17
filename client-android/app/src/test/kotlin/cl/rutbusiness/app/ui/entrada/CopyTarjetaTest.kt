package cl.rutbusiness.app.ui.entrada

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de la tarjeta de rescate (ADR-0022): hoja del cuaderno,
 * no export técnico ni CTA de WhatsApp.
 */
class CopyTarjetaTest {

    private val jergaProhibida = listOf(
        "seed",
        "seed phrase",
        "computador",
        "servidor",
        "sistema",
        "192.168",
        "10.0.2.2",
        "mac",
        "payload",
        "driver",
        "hetzner",
    )

    private fun todosLosTextos(c: CopyTarjeta): List<String> = listOf(
        c.tituloBarra,
        c.subtituloBarra,
        c.tituloHero,
        c.cuerpoHero,
        c.tituloPalabras,
        c.tituloBloques,
        c.tituloQr,
        c.ayudaQr,
        c.pieQrOk,
        c.pieQrFallo,
        c.tituloAnotar,
        c.cuerpoAnotar,
        c.tituloPagina,
        c.ctaCopiar,
        c.ctaCopiado,
        c.ctaGuardarNota,
        c.ctaImprimir,
        c.piePagina,
        c.ctaListo,
        c.ctaSeguirSinAnotar,
    )

    @Test
    fun `tarjeta habla de puesto o cuaderno y nombra las 12 palabras`() {
        val c = copyTarjeta()
        val todo = todosLosTextos(c).joinToString(" ").lowercase()
        assertTrue(
            "debe nombrar puesto o cuaderno",
            todo.contains("puesto") || todo.contains("cuaderno"),
        )
        assertTrue(
            "debe nombrar las 12 palabras",
            todo.contains("12 palabras"),
        )
        assertTrue(c.tituloPalabras.contains("12 palabras"))
        assertTrue(c.tituloBarra.contains("tarjeta", ignoreCase = true))
        assertTrue(c.subtituloBarra.contains("cuaderno", ignoreCase = true))
        assertTrue(c.tituloHero.contains("puesto", ignoreCase = true))
    }

    @Test
    fun `CTAs suenan a papel y WhatsApp no es CTA principal`() {
        val c = copyTarjeta()
        assertTrue(c.ctaCopiar.contains("Copiar las palabras", ignoreCase = true))
        assertTrue(c.ctaGuardarNota.contains("Notas", ignoreCase = true))
        assertTrue(
            c.ctaImprimir.contains("Imprimir", ignoreCase = true) ||
                c.ctaImprimir.contains("PDF", ignoreCase = true),
        )
        assertTrue(c.ctaListo.contains("anoté", ignoreCase = true))

        // WhatsApp solo como aviso, nunca como etiqueta de botón.
        assertFalse(
            "CTA copiar no debe sugerir WhatsApp",
            c.ctaCopiar.lowercase().contains("whatsapp"),
        )
        assertFalse(
            "CTA guardar no debe sugerir WhatsApp",
            c.ctaGuardarNota.lowercase().contains("whatsapp"),
        )
        assertFalse(
            "CTA imprimir no debe sugerir WhatsApp",
            c.ctaImprimir.lowercase().contains("whatsapp"),
        )
        assertFalse(
            "CTA listo no debe sugerir WhatsApp",
            c.ctaListo.lowercase().contains("whatsapp"),
        )
        // El aviso de no mandar por WhatsApp sí puede vivir en cuerpo de ayuda.
        val avisos = listOf(c.ayudaQr, c.cuerpoAnotar).joinToString(" ").lowercase()
        assertTrue(
            "el aviso de no mandar por WhatsApp se conserva",
            avisos.contains("whatsapp"),
        )
    }

    @Test
    fun `QR y labels no suenan a lab ni a jerga de red`() {
        val c = copyTarjeta()
        assertFalse(
            "título QR no dice «Código QR de rescate» (lab)",
            c.tituloQr.equals("Código QR de rescate", ignoreCase = true),
        )
        assertTrue(
            c.tituloQr.contains("cuaderno", ignoreCase = true) ||
                c.ayudaQr.contains("escanear", ignoreCase = true),
        )
        for (texto in todosLosTextos(c)) {
            val lower = texto.lowercase()
            for (palabra in jergaProhibida) {
                assertFalse(
                    "tarjeta no debe decir «$palabra»: $texto",
                    lower.contains(palabra),
                )
            }
        }
    }
}
