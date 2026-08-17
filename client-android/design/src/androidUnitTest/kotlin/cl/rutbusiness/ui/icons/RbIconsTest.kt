package cl.rutbusiness.ui.icons

import androidx.compose.ui.graphics.vector.ImageVector
import cl.rutbusiness.ui.theme.RbDefaultDimens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Proves what construction alone can prove.
 *
 * No [androidx.compose.ui.test.junit4.createComposeRule], no `@RunWith(RobolectricTestRunner::class)`
 * — plain JUnit. [ImageVector.Builder] is pure Kotlin object construction
 * with nothing underneath that calls into `android.*`, so this runs on the
 * host JVM exactly like `RbContrastTest` does for the color ramp. It cannot
 * catch a lopsided triangle or a mistimed `quadTo`; it catches the mistakes a
 * copy-paste across thirty-plus vectors actually makes: a viewport that
 * doesn't match its siblings, a filled variant that's secretly the outline
 * one again, an icon nobody registered.
 */
class RbIconsTest {

    /** The three concepts [cl.rutbusiness.app.ui.nav.BarraDeNavegacion] can
     *  show, and the only ones the brief requires an outline/filled pair for. */
    private val conceptosDeLaBarra = listOf("agente", "cobrar", "resumen")

    @Test
    fun `the catalogue is not empty`() {
        assertTrue("RbIcons.all has no entries", RbIcons.all.isNotEmpty())
    }

    @Test
    fun `every icon shares the same viewport and default size`() {
        RbIcons.all.forEach { (key, vector) ->
            assertEquals("$key.viewportWidth", 24f, vector.viewportWidth, 0f)
            assertEquals("$key.viewportHeight", 24f, vector.viewportHeight, 0f)
            assertEquals("$key.defaultWidth", RbDefaultDimens.iconSize, vector.defaultWidth)
            assertEquals("$key.defaultHeight", RbDefaultDimens.iconSize, vector.defaultHeight)
        }
    }

    @Test
    fun `every icon is named after its own catalogue key`() {
        RbIcons.all.forEach { (key, vector) ->
            assertEquals(
                "the icon registered under \"$key\" carries a different internal name",
                "RbIcons.$key",
                vector.name,
            )
        }
    }

    @Test
    fun `the catalogue has no duplicate entries`() {
        val identities = RbIcons.all.values.map { System.identityHashCode(it) }
        assertEquals(
            "two catalogue keys point at the exact same ImageVector instance",
            RbIcons.all.size,
            identities.distinct().size,
        )
    }

    @Test
    fun `the bar concepts expose a distinct outline and filled variant`() {
        conceptosDeLaBarra.forEach { concepto ->
            val contorno = RbIcons.all["${concepto}Contorno"]
            val relleno = RbIcons.all["${concepto}Relleno"]
            assertTrue("$concepto has no outline variant registered", contorno != null)
            assertTrue("$concepto has no filled variant registered", relleno != null)
            assertNotSame(
                "$concepto's outline and filled variants must be different drawings",
                contorno,
                relleno,
            )
        }
    }

    /**
     * Walks [RbIcons]'s public zero-arg getters via `java.lang.reflect` — no
     * `kotlin-reflect` dependency needed — and asserts every `ImageVector` one
     * of them returns is reachable through [RbIcons.all]. Guards against an
     * icon declared as a `val` above and never wired into the registry: that
     * icon would compile clean and sit there unused and untested forever.
     */
    @Test
    fun `no icon is declared without being registered in the catalogue`() {
        val getters = RbIcons::class.java.methods.filter { method ->
            method.parameterCount == 0 &&
                method.name.startsWith("get") &&
                ImageVector::class.java.isAssignableFrom(method.returnType)
        }
        assertTrue(
            "found no ImageVector-returning getters on RbIcons - the reflection probe itself is broken",
            getters.isNotEmpty(),
        )

        val registered = RbIcons.all.values.map { System.identityHashCode(it) }.toSet()
        val orphans = getters.filter { getter ->
            val value = getter.invoke(RbIcons) as ImageVector
            System.identityHashCode(value) !in registered
        }
        assertTrue(
            "these ImageVector properties are not in RbIcons.all: " +
                orphans.joinToString { it.name },
            orphans.isEmpty(),
        )
    }
}
