package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Reglas de feria en [CajaViewModel] sin HTTP real.
 *
 * Sin sesión activa no se puede abrir de verdad; lo que se fija acá es el
 * gate de apertura (blank = $0 en feria) y el flag [CajaViewModel.modoFeria].
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class CajaFeriaViewModelTest {

    private fun vm(): CajaViewModel {
        val app = RuntimeEnvironment.getApplication()
        return CajaViewModel(SessionRepository(AlmacenamientoPlataforma(app)))
    }

    @Test
    fun `sin feria el monto en blanco impide abrir con copy de cajon`() {
        val caja = vm()
        assertEquals(false, caja.esFeria)
        val motivo = caja.impedimentoParaAbrir()
        assertNotNull(motivo)
        assertTrue(motivo!!.lowercase().contains("cajón"))
    }

    @Test
    fun `con modoFeria el monto en blanco no impide abrir`() {
        val caja = vm()
        caja.modoFeria(true)
        assertTrue(caja.esFeria)
        assertNull(caja.impedimentoParaAbrir())
    }

    @Test
    fun `con modoFeria un monto basura sigue bloqueando`() {
        val caja = vm()
        caja.modoFeria(true)
        caja.cambiarMontoDeApertura("..")
        assertNotNull(caja.impedimentoParaAbrir())
    }

    @Test
    fun `farmacia con monto valido no bloquea`() {
        val caja = vm()
        caja.cambiarMontoDeApertura("0")
        assertNull(caja.impedimentoParaAbrir())
    }
}
