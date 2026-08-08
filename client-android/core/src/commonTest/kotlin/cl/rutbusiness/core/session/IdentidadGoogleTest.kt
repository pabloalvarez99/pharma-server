package cl.rutbusiness.core.session

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.coroutines.runBlocking

class IdentidadGoogleTest {

    @Test
    fun `stub nunca se marca disponible`() {
        assertFalse(IdentidadGoogleNoCableada.disponible())
    }

    @Test
    fun `copy feria menciona cuaderno y Google`() {
        val copy = IdentidadGoogleNoCableada.copyPromocion(rubroEsFeria = true)
        assertTrue(copy.contains("Google"), copy)
        assertTrue(copy.contains("cuaderno"), copy)
        assertTrue(copy.contains("correo"), copy)
        // Cero jerga de OAuth / consola.
        assertFalse(copy.contains("OAuth", ignoreCase = true))
        assertFalse(copy.contains("client id", ignoreCase = true))
        assertFalse(copy.contains("token", ignoreCase = true))
    }

    @Test
    fun `copy formal no habla de cuaderno`() {
        val copy = IdentidadGoogleNoCableada.copyPromocion(rubroEsFeria = false)
        assertTrue(copy.contains("Google"), copy)
        assertFalse(copy.contains("cuaderno"), copy)
    }

    @Test
    fun `pedirCuenta stub devuelve NoDisponible con mensaje usable`() = runBlocking {
        val r = IdentidadGoogleNoCableada.pedirCuenta()
        assertTrue(r is ResultadoGoogle.NoDisponible)
        val n = r as ResultadoGoogle.NoDisponible
        assertTrue(n.mensajeUsuario.contains("correo"), n.mensajeUsuario)
        assertEquals("google_sign_in_not_wired", n.razonInterna)
    }

    @Test
    fun `etiqueta del boton es estable para tests de UI`() {
        assertEquals("Continuar con Google", IdentidadGoogleNoCableada.etiquetaBoton())
    }
}
