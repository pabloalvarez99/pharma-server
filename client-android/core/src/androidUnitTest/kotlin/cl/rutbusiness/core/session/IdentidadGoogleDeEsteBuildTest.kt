package cl.rutbusiness.core.session

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlinx.coroutines.runBlocking

/**
 * El gate del carril: **un build sin client id se comporta igual que hoy.**
 *
 * Es el test que no se puede romper. Si se rompe, se rompió el único camino de
 * entrada que hoy funciona de verdad — correo y clave — en nombre de uno que
 * todavía no tiene credenciales en la nube.
 *
 * Por eso no alcanza con que `disponible()` devuelva `false`: se afirma que el
 * objeto es **el mismo** [IdentidadGoogleNoCableada] que la app ya usaba, y que
 * llegar a él no necesita ni una Activity.
 */
class IdentidadGoogleDeEsteBuildTest {

    /** Revienta si alguien la evalúa. Un build sin Google no la toca nunca. */
    private val contextoQueNoDebeUsarse: () -> android.content.Context = {
        error("un build sin client id no puede necesitar un Context de Android")
    }

    @Test
    fun `sin client id es exactamente el stub de siempre`() {
        assertSame(
            IdentidadGoogleNoCableada,
            identidadGoogleDeEsteBuild(clientId = null, contexto = contextoQueNoDebeUsarse),
        )
    }

    /**
     * `BuildConfig.GOOGLE_CLIENT_ID` es `""` cuando no se pasó
     * `-Prb.googleClientId`, no `null`. Si el gate mirara sólo `null`, el APK
     * del repo intentaría abrir el selector de cuentas con un client id vacío:
     * Google rechaza y la persona ve un error donde antes veía un camino que
     * funcionaba.
     */
    @Test
    fun `client id vacio o en blanco cuenta como sin client id`() {
        for (vacio in listOf("", "   ", "\t\n")) {
            assertSame(
                IdentidadGoogleNoCableada,
                identidadGoogleDeEsteBuild(clientId = vacio, contexto = contextoQueNoDebeUsarse),
                "\"$vacio\" tiene que contar como sin client id",
            )
        }
    }

    @Test
    fun `sin client id el boton no aparece`() {
        val identidad = identidadGoogleDeEsteBuild(null, contextoQueNoDebeUsarse)
        assertFalse(identidad.disponible(), "el botón de Google no puede aparecer")
    }

    /**
     * Y si algo igual lo llamara, tampoco inventa una sesión: contesta
     * `NoDisponible` con el texto que manda a correo y clave.
     */
    @Test
    fun `sin client id pedir cuenta no abre nada y manda a correo y clave`() = runBlocking {
        val identidad = identidadGoogleDeEsteBuild(null, contextoQueNoDebeUsarse)
        val r = identidad.pedirCuenta()
        assertTrue(r is ResultadoGoogle.NoDisponible, "no puede haber picker sin client id")
        assertTrue((r as ResultadoGoogle.NoDisponible).mensajeUsuario.contains("correo"))
    }

    /**
     * El copy del stub habla en futuro ("vas a poder"). Con client id, el de la
     * implementación real habla en presente: prometer algo que el build no hace
     * es exactamente el bug que este gate evita, y al revés también.
     */
    @Test
    fun `el copy sin client id no promete algo que este build ya haga`() {
        val copy = identidadGoogleDeEsteBuild(null, contextoQueNoDebeUsarse)
            .copyPromocion(rubroEsFeria = true)
        assertTrue(copy.contains("Más adelante") || copy.contains("pronto"), copy)
    }
}
