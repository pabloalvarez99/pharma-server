package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
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

    @Test
    fun `html de una pagina escapa y lleva palabras y payload`() {
        val clave = claveDeDemostracion()
        val html = htmlTarjetaImprimible(clave, "Puesto-Rosa")
        assertTrue(html.startsWith("<!DOCTYPE html>"))
        assertTrue(html.contains("Puesto-Rosa".lowercase()) || html.contains("puesto-rosa"))
        assertTrue(html.contains(clave.palabras[0]))
        assertTrue(html.contains(clave.bloquesCompletos()))
        assertTrue(html.contains("rutbusiness-rescue:v1:puesto-rosa:"))
        assertTrue(html.contains("No la mandes por WhatsApp") || html.contains("WhatsApp"))
        // No mete la frase cruda como un solo nodo sin numerar (lista <li>).
        assertTrue(html.contains("<li>"))
        assertTrue(html.contains("&lt;") || !html.contains("<script"))
    }

    /**
     * La hoja se presenta como **RutAgent**, que es el nombre que la dueña ve en
     * el teléfono. Va acá, en `core`, y no sólo en la prueba de UI de la app,
     * porque el nombre es el valor por defecto de estas dos funciones: quien lo
     * cambie va a estar editando este archivo, no una pantalla.
     *
     * De los tres lugares, el pie es el que se pierde: título y advertencia se
     * leen al revisar la hoja, el pie está abajo de todo y no lo mira nadie
     * hasta que la tarjeta aparece meses después, con un nombre que ya no
     * existe, en manos de alguien que necesita saber qué app reinstalar.
     *
     * El payload `rutbusiness-rescue:v1:` no entra en esto —lo fijan las pruebas
     * de arriba— y no se renombra: es formato de cable, y cambiarlo invalidaría
     * las tarjetas ya impresas.
     */
    @Test
    fun `la tarjeta se presenta como RutAgent y no como el nombre interno`() {
        val clave = claveDeDemostracion()
        val texto = textoTarjetaImprimible(clave, "Puesto-Rosa")
        val html = htmlTarjetaImprimible(clave, "Puesto-Rosa")

        assertTrue(texto.startsWith("RutAgent - tarjeta de rescate"))
        assertTrue(html.contains("<h1>RutAgent - tarjeta de rescate</h1>"))
        assertTrue(html.contains("<title>RutAgent - tarjeta de rescate</title>"))
        assertTrue(html.contains("RutAgent no puede recuperarla"))
        assertTrue(html.contains("RutAgent · tarjeta de rescate"))

        // Y que no quede el viejo en ninguna de las dos, salvo el payload.
        val sinPayload = { s: String -> s.replace("rutbusiness-rescue:v1:", "") }
        assertFalse(sinPayload(texto).contains("RutBusiness", ignoreCase = true))
        assertFalse(sinPayload(html).contains("RutBusiness", ignoreCase = true))
    }

    @Test
    fun `html con svg qr inserta markup confiable`() {
        val clave = claveDeDemostracion()
        // Mini matriz 3x3 con un módulo.
        val grilla = arrayOf(
            booleanArrayOf(true, false, true),
            booleanArrayOf(false, true, false),
            booleanArrayOf(true, false, true),
        )
        val svg = assertNotNull(svgMatrizCodigo(grilla))
        assertTrue(svg.contains("<svg"))
        assertTrue(svg.contains("""x="0" y="0""""))
        val html = htmlTarjetaImprimible(clave, "demo", svgQr = svg)
        assertTrue(html.contains("<svg"))
        assertTrue(html.contains("viewBox="))
    }

    @Test
    fun `escaparHtml neutraliza tags`() {
        assertEquals("&lt;b&gt;x&amp;y&lt;/b&gt;", escaparHtml("<b>x&y</b>"))
        assertEquals("a&quot;b&#39;c", escaparHtml("a\"b'c"))
    }
}
