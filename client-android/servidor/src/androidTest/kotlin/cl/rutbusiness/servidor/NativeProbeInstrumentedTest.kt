package cl.rutbusiness.servidor

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * H4: SurrealKV en el `filesDir` de la app, desde el proceso de la app
 * (dominio SELinux `untrusted_app` + UID de la app — no se puede con un
 * binario suelto).
 *
 * `./gradlew :servidor:connectedDebugAndroidTest`
 */
@RunWith(AndroidJUnit4::class)
class NativeProbeInstrumentedTest {
    @Test
    fun nativeProbe_escribeCierraReabreYLee() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(ctx.filesDir, "h4-surreal-probe")
        if (dir.exists()) {
            dir.deleteRecursively()
        }

        val result = PuenteNativo.nativeProbe(dir.absolutePath)
        assertTrue(
            "esperado PROBE OK, obtuve: $result",
            result.startsWith("PROBE OK"),
        )
        assertTrue(
            "debe reportar bytes_on_disk > 0: $result",
            result.contains("bytes=") && !result.contains("bytes=0"),
        )

        // El motor dejó algo en disco que Kotlin también ve.
        val onDisk = dir.walkTopDown().filter { it.isFile }.sumOf { it.length() }
        assertTrue("Kotlin ve $onDisk bytes en ${dir.absolutePath}", onDisk > 0)

        // Segunda corrida sobre el mismo dir: no crashea y sigue OK.
        val again = PuenteNativo.nativeProbe(dir.absolutePath)
        assertTrue("segunda corrida: $again", again.startsWith("PROBE OK"))
    }

    @Test
    fun nativeSaludo_sigueH3() {
        assertEquals("h3-ok", PuenteNativo.nativeSaludo())
    }
}
