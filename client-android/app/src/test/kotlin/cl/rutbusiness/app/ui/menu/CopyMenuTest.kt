package cl.rutbusiness.app.ui.menu

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Gate de copy del menú del puesto (ADR-0022).
 *
 * Puro JVM, mismo criterio que `CopyResumenTest.kt`. Dos reglas se verifican
 * acá: el gate de feria de siempre (nunca sistema, cajón, arqueo, computador,
 * stock, catálogo, boleta), y una propia de este menú — "respaldo" y "cola de
 * subida" no son palabras del puesto y no pueden aparecer en **ningún** rubro,
 * porque son la razón por la que este menú existe (el encargo de la ola las
 * nombra explícitamente como lo que la dueña nunca diría).
 */
class CopyMenuTest {

    private val palabrasProhibidasFeria = listOf(
        "sistema",
        "cajón",
        "cajon",
        "arqueo",
        "computador",
        "tablero",
        "kpi",
        "dashboard",
        "boleta",
        "stock",
        "catálogo",
        "catalogo",
    )

    private val palabrasProhibidasSiempre = listOf(
        "respaldo",
        "cola de subida",
        "backup",
        "sincroniz",
    )

    @Test
    fun `titulo del menu cambia por rubro`() {
        assertEquals("Tu puesto", tituloMenu(feria = true))
        assertEquals("Más opciones", tituloMenu(feria = false))
        assertEquals("Todo lo que puedes hacer aquí", subtituloMenu())
    }

    @Test
    fun `contar la plata no dice cajon ni arqueo en feria`() {
        val titulo = tituloCaja(feria = true)
        assertEquals("Contar la plata del puesto", titulo)
        assertEquals("La caja", tituloCaja(feria = false))
        assertFalse(titulo.lowercase().contains("caja "))
        assertFalse(titulo.lowercase().contains("arqueo"))
    }

    @Test
    fun `impresora y ventas pendientes son la misma frase en los dos rubros`() {
        assertEquals("La impresora", tituloImpresora())
        assertEquals("Emparejarla o probar que imprima", subtituloImpresora())
        assertEquals("Ventas que faltan subir", tituloVentasPendientes())
        assertEquals(
            "Se guardan en el teléfono y suben solas cuando vuelva la señal",
            subtituloVentasPendientes(),
        )
    }

    @Test
    fun `cambiar de rubro`() {
        assertEquals("Cómo vendo", tituloRubro(feria = true))
        assertEquals("Tipo de negocio", tituloRubro(feria = false))
        assertEquals("Cambia si tu negocio cambió", subtituloRubro())
        assertEquals("Ahora: Feria / Calle", etiquetaRubroActual("Feria / Calle"))
        assertEquals("¿Cambiar tu tipo de negocio?", tituloConfirmarRubro())
        assertEquals("Sí, cambiar", botonConfirmarRubro())
        val mensaje = mensajeConfirmarRubro("Farmacia")
        assertEquals(
            "Vas a cambiar a \"Farmacia\". Esto cambia lo que ves en la app — el agente, " +
                "el fiado, el escáner y la impresora — para que se parezca a tu día.",
            mensaje,
        )
    }

    @Test
    fun `guardando y sin conexion`() {
        assertEquals("Guardando...", guardandoRubro())
        assertEquals(
            "No se pudo guardar sin conexión. Prueba de nuevo cuando tengas señal.",
            mensajeSinConexionRubro(),
        )
    }

    @Test
    fun `feria nunca usa jerga de tablero ni de cajon`() {
        for (copy in todoCopyFeriaUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasProhibidasFeria) {
                assertFalse(
                    "copy feria no puede decir «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    @Test
    fun `respaldo y cola de subida no son palabras del puesto en ningun rubro`() {
        for (copy in todoCopyUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasProhibidasSiempre) {
                assertFalse(
                    "el menú no puede decir «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    /** Strings de usuario en el camino feria del menú. */
    private fun todoCopyFeriaUsuario(): List<String> = listOf(
        tituloMenu(true),
        subtituloMenu(),
        tituloCaja(true),
        subtituloCaja(true),
        tituloImpresora(),
        subtituloImpresora(),
        tituloVentasPendientes(),
        subtituloVentasPendientes(),
        tituloRubro(true),
        subtituloRubro(),
        etiquetaRubroActual("Feria / Calle"),
        tituloConfirmarRubro(),
        mensajeConfirmarRubro("Minimarket / Almacén"),
        botonConfirmarRubro(),
        guardandoRubro(),
        mensajeSinConexionRubro(),
    )

    /** Todo el copy de usuario del menú, feria y retail formal, para el gate de siempre. */
    private fun todoCopyUsuario(): List<String> = todoCopyFeriaUsuario() + listOf(
        tituloMenu(false),
        tituloCaja(false),
        subtituloCaja(false),
        tituloRubro(false),
    )
}
