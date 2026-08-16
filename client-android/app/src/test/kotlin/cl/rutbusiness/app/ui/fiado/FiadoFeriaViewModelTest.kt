package cl.rutbusiness.app.ui.fiado

import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Reglas de feria en [FiadoViewModel] sin HTTP real.
 *
 * Sin sesión activa no se abre el puesto de verdad; lo que se fija es el flag
 * [FiadoViewModel.modoFeria], el default de efectivo y que el copy de error no
 * mande a prender un computador.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class FiadoFeriaViewModelTest {

    private fun vm(): FiadoViewModel {
        val app = RuntimeEnvironment.getApplication()
        return FiadoViewModel(SessionRepository(AlmacenamientoPlataforma(app)))
    }

    @Test
    fun `modoFeria enciende el flag sin cambiar el constructor`() {
        val fiado = vm()
        assertEquals(false, fiado.esFeria)
        fiado.modoFeria(true)
        assertTrue(fiado.esFeria)
    }

    @Test
    fun `con modoFeria el abono en efectivo queda ON por defecto`() {
        val fiado = vm()
        fiado.modoFeria(true)
        // Sin caja real el retail apagaría el toggle; feria lo deja ON y
        // reintenta abrir el puesto cuando haya red/sesión.
        assertTrue(fiado.entraALaCaja)
        fiado.irAlAbono()
        assertTrue(fiado.entraALaCaja)
        assertEquals(PasoDeFiado.Abono, fiado.paso)
    }

    @Test
    fun `modoFeria no exige computador en el copy de cuenta`() {
        // El flag de feria no tiene por qué hablar de computador: el copy de
        // error de cuenta se resuelve con errorSinCuenta(feria), no con un
        // mensaje del ViewModel.
        val copy = errorSinCuenta(feria = true)
        assertFalse(
            "modoFeria no exige computador",
            copy.lowercase().contains("computador"),
        )
        assertTrue(copy.contains("señal") || copy.contains("intentá"))
    }

    @Test
    fun `sin feria marcar efectivo sin caja no deja entraALaCaja`() {
        val fiado = vm()
        assertEquals(false, fiado.esFeria)
        // Sin sesión de caja, el retail no puede meter billete al arqueo.
        fiado.cambiarEntraALaCaja(true)
        assertFalse(fiado.entraALaCaja)
    }
}
