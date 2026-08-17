package cl.rutbusiness.app.ui.alta

import cl.rutbusiness.ui.theme.RbDarkColors
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbLightColors
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * El catálogo de rubros de la app contra el del server, campo por campo.
 *
 * [RUBROS] es una copia de `crates/domain/src/rubro.rs`, y existe porque el
 * alta ocurre antes de tener sesión: el único endpoint que sirve el catálogo
 * (`GET /api/v1/rubro-pack`) pide JWT y además devuelve un solo pack. Sin este
 * archivo, la copia se desincroniza el día que alguien agregue un rubro en el
 * server, y el síntoma sería una app que ofrece un rubro que el server rechaza
 * con un 400 — o peor, que no ofrece el rubro nuevo y nadie se entera.
 *
 * Se lee el `.rs` con expresiones regulares y no se invoca ningún generador: es
 * una tabla de ocho filas con tres textos cada una, y montar generación de
 * código para eso costaría más de lo que cuesta cualquier desincronización que
 * pueda ocurrir.
 */
class RubrosTest {

    /**
     * `crates/domain/src/rubro.rs`, relativo al directorio del módulo (que es
     * donde Gradle corre las pruebas: `client-android/app`).
     */
    private val fuente = File("../../crates/domain/src/rubro.rs")

    private data class PackDelServer(val clave: String, val etiqueta: String, val frase: String)

    /**
     * Saca las filas de `static PACKS`.
     *
     * Cada pack abre con `rubro: "x"` y sigue con `label:` y `tagline:` en ese
     * orden. Los `label:` de los `AttrField` vienen después del `tagline:`, así
     * que tomar el primero de cada uno tras el `rubro:` es exacto.
     */
    private fun leerElServer(): List<PackDelServer> {
        val lineas = fuente.readLines()
        val packs = mutableListOf<PackDelServer>()
        var i = 0
        val clave = Regex("""^\s*rubro:\s*"([^"]+)"""")
        val etiqueta = Regex("""^\s*label:\s*"([^"]+)"""")
        val frase = Regex("""^\s*tagline:\s*"([^"]+)"""")

        while (i < lineas.size) {
            val c = clave.find(lineas[i])
            if (c == null) { i += 1; continue }
            var e: String? = null
            var f: String? = null
            var j = i + 1
            while (j < lineas.size && (e == null || f == null)) {
                if (e == null) etiqueta.find(lineas[j])?.let { e = it.groupValues[1] }
                if (f == null) frase.find(lineas[j])?.let { f = it.groupValues[1] }
                j += 1
            }
            packs += PackDelServer(c.groupValues[1], e.orEmpty(), f.orEmpty())
            i = j
        }
        return packs
    }

    /**
     * El repo entero está presente o no hay contra qué comparar.
     *
     * Se salta y no falla cuando el crate no está: eso pasa si alguien saca el
     * módulo Android del monorepo, y en ese escenario no hay desincronización
     * posible porque no hay dos copias.
     */
    private fun exigirElCrate() {
        assumeTrue(
            "no se encontró ${fuente.absolutePath}; nada que comparar",
            fuente.isFile,
        )
    }

    @Test
    fun `el catalogo de la app es exactamente el del server`() {
        exigirElCrate()
        val delServer = leerElServer()

        assertTrue("no se pudo leer ningún pack de ${fuente.path}", delServer.isNotEmpty())
        assertEquals(
            "sobran o faltan rubros respecto de rubro.rs",
            delServer.map { it.clave },
            RUBROS.map { it.clave },
        )
        delServer.zip(RUBROS).forEach { (server, app) ->
            assertEquals("etiqueta distinta para ${server.clave}", server.etiqueta, app.etiqueta)
            assertEquals("frase distinta para ${server.clave}", server.frase, app.frase)
        }
    }

    /**
     * `otro` tiene que existir y tiene que ir al final.
     *
     * Es la caída de [AltaViewModel.crear] cuando por alguna razón no hay rubro
     * elegido, y es también la regla del pivote del server: el rubro por
     * defecto es el genérico, **nunca** farmacia
     * (`DEFAULT_RUBRO` en `rubro.rs`).
     */
    @Test
    fun `el generico es el ultimo y no es farmacia`() {
        assertEquals("otro", RUBROS.last().clave)
    }

    /** Ocho claves distintas: una repetida haría invisible a la de abajo. */
    @Test
    fun `no hay claves repetidas`() {
        assertEquals(RUBROS.size, RUBROS.map { it.clave }.toSet().size)
    }

    /**
     * Feria es el primer cartel: se lee sin scrollear en un teléfono chico y es
     * el foco de producto (ADR-0022). Si alguien reordena el catálogo del
     * server, este test y el de sincronía fallan juntos.
     */
    @Test
    fun `feria es el primer cartel y el natural`() {
        assertEquals(RUBRO_NATURAL, RUBROS.first().clave)
        assertTrue(RUBROS.first().esNatural())
        assertFalse(RUBROS.last().esNatural())
        assertTrue(RUBROS.none { it.clave != RUBRO_NATURAL && it.esNatural() })
    }

    /**
     * Sólo feria anuncia chip cuando nadie eligió todavía: el resto de carteles
     * van limpios. Si todos trajeran pastilla, la pantalla se siente a combo.
     */
    @Test
    fun `solo feria trae chip cuando el cartel esta libre`() {
        assertEquals(CHIP_RUBRO_NATURAL, RUBROS.first().chipCuandoLibre())
        RUBROS.drop(1).forEach { rubro ->
            assertNull("el cartel ${rubro.clave} no debe anunciar chip suelto", rubro.chipCuandoLibre())
        }
    }

    /** Elegido se dice igual en todos: no hay jerga ni "selected". */
    @Test
    fun `elegido se dice en espanol en cualquier cartel`() {
        RUBROS.forEach { rubro ->
            assertEquals(CHIP_RUBRO_ELEGIDO, rubro.chipCuandoElegido())
        }
    }

    /** La ayuda del paso habla de cartel y de feria, no de combo ni wizard. */
    @Test
    fun `la ayuda del paso habla de cartel y de feria`() {
        assertTrue(AYUDA_DE_RUBROS.contains("feria", ignoreCase = true))
        assertTrue(AYUDA_DE_RUBROS.contains("cartel", ignoreCase = true))
        assertFalse(AYUDA_DE_RUBROS.contains("combo", ignoreCase = true))
        assertFalse(AYUDA_DE_RUBROS.contains("wizard", ignoreCase = true))
    }

    /**
     * El cartel de feria (el natural, no elegido) no puede pintarse con el
     * borde de "elegido".
     *
     * Es la prueba del bug real: un `uiautomator dump` en el emulador mostró
     * el cartel de feria con borde verde grueso y `checked="false"` a la vez —
     * Siguiente seguía apagado hasta tocarlo nuevamente. La causa no era un
     * estado desincronizado (`semantics { selected = elegido }` ya decía la
     * verdad) sino que [bordeColorDeCartel] y [bordeAnchoDeCartel] antes
     * miraban también si el rubro era el natural, no sólo si estaba elegido.
     * Sin Compose de por medio: son funciones puras sobre [RbLightColors] /
     * [RbDarkColors] y [RbDefaultDimens], los mismos valores que usa la
     * pantalla real.
     */
    @Test
    fun `el cartel natural sin elegir no toma el borde de elegido`() {
        listOf(RbLightColors, RbDarkColors).forEach { colores ->
            val bordeLibre = bordeColorDeCartel(elegido = false, colors = colores)
            val bordeElegido = bordeColorDeCartel(elegido = true, colors = colores)

            assertEquals(
                "un cartel no elegido tiene que llevar el borde neutro",
                colores.outlineStrong,
                bordeLibre,
            )
            assertNotEquals(
                "el borde de un cartel libre no puede ser el de uno elegido",
                bordeElegido,
                bordeLibre,
            )
        }

        val anchoLibre = bordeAnchoDeCartel(elegido = false, dimens = RbDefaultDimens)
        val anchoElegido = bordeAnchoDeCartel(elegido = true, dimens = RbDefaultDimens)
        assertEquals(RbDefaultDimens.border, anchoLibre)
        assertNotEquals(
            "el ancho de un cartel libre no puede ser el ancho de foco de uno elegido",
            anchoElegido,
            anchoLibre,
        )
    }

    /** Elegir sí trae el borde y el ancho de foco: la señal existe, sólo que ahora la manda la única fuente correcta. */
    @Test
    fun `el cartel elegido si toma el borde de foco`() {
        assertEquals(RbLightColors.brandText, bordeColorDeCartel(elegido = true, colors = RbLightColors))
        assertEquals(RbDefaultDimens.focusRing, bordeAnchoDeCartel(elegido = true, dimens = RbDefaultDimens))
    }
}
