package cl.rutbusiness.servidor

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * H3: la cadena entera prende — jniLibs empaquetado, dlopen del .so, JNI.
 * Corre en emulador / aparato: `./gradlew :servidor:connectedDebugAndroidTest`
 */
@RunWith(AndroidJUnit4::class)
class PuenteNativoInstrumentedTest {
    @Test
    fun nativeSaludo_devuelveH3Ok() {
        assertEquals("h3-ok", PuenteNativo.nativeSaludo())
    }
}
