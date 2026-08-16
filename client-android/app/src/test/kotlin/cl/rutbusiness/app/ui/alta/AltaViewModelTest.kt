package cl.rutbusiness.app.ui.alta

import cl.rutbusiness.app.ui.entrada.ProbadorDeServidor
import cl.rutbusiness.app.ui.entrada.Sondeo
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Las reglas del alta que no dibujan nada: cuántas preguntas hay, cuándo se
 * puede avanzar, y qué se le dice a quien no puede.
 *
 * La más importante es la primera. Que el camino de la nube tenga **tres**
 * pasos y ninguno sea una dirección es la decisión de diseño entera de este
 * trabajo, y es exactamente la clase de cosa que alguien deshace sin darse
 * cuenta agregando un campo "por si acaso".
 *
 * Robolectric por el `Context` que necesita el almacenamiento; nada de esto
 * toca la red. Construir [SessionRepository] sólo abre un `SharedPreferences`.
 */
@RunWith(RobolectricTestRunner::class)
// Robolectric no trae imagen para el `targetSdk 36` del módulo y se niega a
// arrancar. 34 es el mismo piso que usa el resto de las pruebas de la app.
@Config(sdk = [34])
class AltaViewModelTest {

    /** Nadie debería llamarlo: acá no se prueba nada que salga a la red. */
    private val nuncaSondea = object : ProbadorDeServidor {
        override suspend fun sondear(baseUrl: String): Sondeo =
            error("esta prueba no habla con ningún server")
    }

    private fun vm(nube: String?, esFeria: Boolean = false): AltaViewModel {
        val app = RuntimeEnvironment.getApplication()
        return AltaViewModel(
            sesion = SessionRepository(AlmacenamientoPlataforma(app)),
            probador = nuncaSondea,
            red = null,
            nube = nube,
            esFeria = esFeria,
        )
    }

    // --- cuántas preguntas ---------------------------------------------------

    @Test
    fun `en feria el alta pregunta por el puesto, no por el negocio`() {
        assertEquals("¿Cómo te dicen en la feria?", tituloDelPaso(PasoDelAlta.Negocio, true))
        assertEquals("¿Cómo se llama tu negocio?", tituloDelPaso(PasoDelAlta.Negocio, false))
        assertEquals("Crear mi puesto", etiquetaDelBoton(EstadoDelAlta.Preguntando, true, true))
        assertEquals("Creando tu puesto...", etiquetaDelBoton(EstadoDelAlta.Creando, true, true))
        assertEquals("Crear mi negocio", etiquetaDelBoton(EstadoDelAlta.Preguntando, true, false))
    }

    @Test
    fun `con nube compilada son tres pasos y ninguno es la direccion`() {
        val alta = vm("https://api.rutbusiness.cl")

        assertEquals(listOf(PasoDelAlta.Negocio, PasoDelAlta.Rubro, PasoDelAlta.Cuenta), alta.pasos)
        assertFalse("el camino de la nube no pregunta direcciones", PasoDelAlta.Donde in alta.pasos)
        assertEquals("https://api.rutbusiness.cl", alta.destino)
    }

    /**
     * En la nube compartida ya hay otros puestos. Preguntar
     * `GET /setup/status` y pintar "lugar ocupado" es el recuadro rojo que
     * vio el feriante: el servidor contesta, pero no para él.
     */
    @Test
    fun `con nube empezar no declara el lugar ocupado ni sale a la red`() {
        val alta = vm("https://api.rutbusiness.cl")
        alta.empezar()
        assertEquals(EstadoDelAlta.Preguntando, alta.estado)
        assertNull(alta.falla)
    }

    @Test
    fun `sin nube compilada se pregunta primero donde guardar`() {
        val alta = vm(null)

        assertEquals(
            listOf(PasoDelAlta.Donde, PasoDelAlta.Negocio, PasoDelAlta.Rubro, PasoDelAlta.Cuenta),
            alta.pasos,
        )
        assertNull("sin dirección escrita no hay destino", alta.destino)
    }

    /**
     * Una nube en blanco es una nube que no existe.
     *
     * `BuildConfig.URL_NUBE` llega como `""` cuando el APK se armó sin
     * `-Prb.urlNube`, y [cl.rutbusiness.app.AppContainer] lo traduce a `null`.
     * Que además el ViewModel aguante una cadena que no se entiende evita que
     * un typo en la línea de comandos deje el alta apuntando a la nada.
     */
    @Test
    fun `una nube que no se entiende se trata como si no hubiera`() {
        val alta = vm("no es una dirección")
        assertTrue(PasoDelAlta.Donde in alta.pasos)
    }

    @Test
    fun `la nube se normaliza igual que cualquier direccion escrita a mano`() {
        assertEquals("http://10.0.2.2:8081", vm("10.0.2.2:8081/").destino)
    }

    // --- cuándo se puede avanzar --------------------------------------------

    @Test
    fun `no se avanza sin nombre de negocio`() {
        val alta = vm("https://api.rutbusiness.cl")

        assertFalse(alta.puedeAvanzar)
        assertEquals("Escribe el nombre de tu negocio.", alta.impedimento())
        assertEquals(
            "Escribe el nombre de tu puesto.",
            vm("https://api.rutbusiness.cl", esFeria = true).impedimento(),
        )

        alta.cambiarNombre("Almacén Doña Rosa")
        assertTrue(alta.puedeAvanzar)
        assertNull(alta.impedimento())
    }

    @Test
    fun `no se avanza sin elegir rubro, y ninguno viene elegido`() {
        val alta = vm("https://api.rutbusiness.cl")
        alta.cambiarNombre("Almacén Doña Rosa")
        alta.avanzar()

        assertEquals(PasoDelAlta.Rubro, alta.paso)
        assertNull("ningún rubro puede venir elegido de fábrica", alta.rubro)
        assertFalse(alta.puedeAvanzar)

        alta.elegirRubro(RUBROS.first { it.clave == "minimarket" })
        assertTrue(alta.puedeAvanzar)
    }

    @Test
    fun `la cuenta exige correo con forma y clave de ocho`() {
        val alta = enElPasoDeLaCuenta()

        alta.cambiarEmail("rosa")
        assertNotNull("un correo sin arroba se marca en el campo", alta.errorDeCorreo)
        assertFalse(alta.puedeAvanzar)

        alta.cambiarEmail("rosa@almacen.cl")
        assertNull(alta.errorDeCorreo)
        alta.cambiarClave("corta")
        assertNotNull(alta.errorDeClave)
        assertFalse(alta.puedeAvanzar)

        alta.cambiarClave("almacen2026")
        assertNull(alta.errorDeClave)
        assertTrue(alta.puedeAvanzar)
    }

    /**
     * El campo vacío no se marca en rojo.
     *
     * Marcar un error en un campo que la persona todavía no llenó la acusa de
     * algo que no hizo, y en la primera pantalla del producto eso se lee como
     * "esta app ya está enojada conmigo".
     */
    @Test
    fun `un campo todavia vacio no se marca en rojo`() {
        val alta = enElPasoDeLaCuenta()
        assertNull(alta.errorDeCorreo)
        assertNull(alta.errorDeClave)
    }

    // --- ir y volver ---------------------------------------------------------

    @Test
    fun `retroceder desde el primer paso devuelve false para que salga la pantalla`() {
        val alta = vm("https://api.rutbusiness.cl")
        assertFalse("en el primer paso no hay a dónde retroceder adentro del alta", alta.retroceder())
    }

    @Test
    fun `retroceder conserva lo ya escrito`() {
        val alta = vm("https://api.rutbusiness.cl")
        alta.cambiarNombre("Almacén Doña Rosa")
        alta.avanzar()
        alta.elegirRubro(RUBROS.first())
        alta.avanzar()

        assertEquals(PasoDelAlta.Cuenta, alta.paso)
        assertTrue(alta.retroceder())
        assertEquals(PasoDelAlta.Rubro, alta.paso)
        assertEquals(RUBROS.first().clave, alta.rubro?.clave)
        assertTrue(alta.retroceder())
        assertEquals("Almacén Doña Rosa", alta.nombre)
    }

    /**
     * La tecla de acción del teclado no mira si el botón está apagado.
     *
     * Sin este camino, apretar «Listo» con una clave de cuatro letras no hace
     * nada y la pantalla se ve colgada: la persona toca otra vez, y otra. El
     * caso llega desde el teclado y no desde el botón, así que ningún test que
     * sólo apriete el botón lo ve.
     */
    @Test
    fun `crear con clave corta contesta que la clave es corta, sin salir a la red`() {
        val alta = enElPasoDeLaCuenta()
        alta.cambiarEmail("rosa@almacen.cl")
        alta.cambiarClave("1234")

        alta.crear()

        assertTrue(alta.falla is FallaDeAlta.ClaveDebil)
        // Y no salió a ninguna parte: el probador de mentira habría explotado.
        assertEquals(EstadoDelAlta.Preguntando, alta.estado)
    }

    private fun enElPasoDeLaCuenta(): AltaViewModel {
        val alta = vm("https://api.rutbusiness.cl")
        alta.cambiarNombre("Almacén Doña Rosa")
        alta.avanzar()
        alta.elegirRubro(RUBROS.first { it.clave == "minimarket" })
        alta.avanzar()
        check(alta.paso == PasoDelAlta.Cuenta)
        return alta
    }
}
