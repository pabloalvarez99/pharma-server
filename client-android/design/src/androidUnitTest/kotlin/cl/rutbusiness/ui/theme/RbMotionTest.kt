package cl.rutbusiness.ui.theme

import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.TweenSpec
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM checks on the motion budget - no Compose host, no Robolectric,
 * because none of this needs a screen: it is arithmetic on durations and
 * object identity on easing curves.
 *
 * What matters for "looks produced, not prototyped" is exactly what is
 * checked here: the roles form a coherent scale (a press answers faster than
 * a destination arrives), each role owns its curve, and the one hard rule -
 * the system's reduce-motion flag zeroes *everything*, not just some of it -
 * cannot regress silently.
 */
class RbMotionTest {

    @Test
    fun `la escala de duraciones respeta el orden por rol`() {
        val motion = RbMotion(reduced = false)
        val toque = motion.quick<Float>() as TweenSpec<Float>
        val elemento = motion.standard<Float>() as TweenSpec<Float>
        val entrada = motion.screenEnter<Float>() as TweenSpec<Float>
        val salida = motion.screenExit<Float>() as TweenSpec<Float>

        assertTrue(
            "un toque tiene que responder antes que un elemento asentándose: " +
                "${toque.durationMillis}ms vs ${elemento.durationMillis}ms",
            toque.durationMillis < elemento.durationMillis,
        )
        assertTrue(
            "una pantalla entera arribando es más evento que un elemento adentro: " +
                "${elemento.durationMillis}ms vs ${entrada.durationMillis}ms",
            elemento.durationMillis < entrada.durationMillis,
        )
        assertTrue(
            "lo que sale despeja la escena más rápido de lo que tarda lo nuevo en llegar: " +
                "${salida.durationMillis}ms vs ${entrada.durationMillis}ms",
            salida.durationMillis < entrada.durationMillis,
        )
    }

    @Test
    fun `cada rol tiene su propia curva y entrada y salida no son la misma reflejada`() {
        val motion = RbMotion(reduced = false)
        val toque = motion.quick<Float>() as TweenSpec<Float>
        val elemento = motion.standard<Float>() as TweenSpec<Float>
        val entrada = motion.screenEnter<Float>() as TweenSpec<Float>
        val salida = motion.screenExit<Float>() as TweenSpec<Float>

        assertEquals(RbEasing.standard, toque.easing)
        assertEquals(RbEasing.standard, elemento.easing)
        assertEquals(RbEasing.enter, entrada.easing)
        assertEquals(RbEasing.exit, salida.easing)
        assertTrue(
            "si arrancar y despedirse comparten curva, la salida no se distingue de la llegada",
            entrada.easing != salida.easing,
        )
    }

    @Test
    fun `factor del sistema en cero deja toda duracion animada en cero`() {
        val reducido = RbMotion(reduced = true)

        assertTrue(reducido.quick<Float>() is SnapSpec<*>)
        assertTrue(reducido.standard<Float>() is SnapSpec<*>)
        assertTrue(reducido.screenEnter<Float>() is SnapSpec<*>)
        assertTrue(reducido.screenExit<Float>() is SnapSpec<*>)

        // El contrato del spinner es distinto a propósito: no es un `tween` de
        // 0ms, es `null`, para que el que lo lee dibuje un anillo estático en
        // vez de animar un giro infinito de duración cero (que igual sería un
        // loop que nunca llega a idle).
        assertNull(
            "con reduceMotion el spinner tiene que avisar 'no animes' con null, no con un tween corto",
            reducido.spinnerSweep(),
        )
    }

    @Test
    fun `sin reduccion el sistema sigue animando, incluido el spinner infinito`() {
        val motion = RbMotion(reduced = false)

        assertTrue(motion.quick<Float>() is TweenSpec<*>)
        assertTrue(motion.standard<Float>() is TweenSpec<*>)
        assertTrue(motion.screenEnter<Float>() is TweenSpec<*>)
        assertTrue(motion.screenExit<Float>() is TweenSpec<*>)
        assertNotNull(motion.spinnerSweep())
    }

    @Test
    fun `las curvas de easing empiezan en 0, terminan en 1 y no retroceden`() {
        // "Bien definida" acá quiere decir tres cosas concretas: la curva no
        // deja el valor animado ni corto (nunca llega a 1) ni pasado (nunca
        // arranca antes de 0), y no hay un tramo donde el valor animado
        // retrocede - lo que se vería como un parpadeo hacia atrás en medio
        // de la transición.
        val curvas = listOf(RbEasing.standard, RbEasing.enter, RbEasing.exit)

        curvas.forEach { curva ->
            assertEquals(0f, curva.transform(0f), 0.001f)
            assertEquals(1f, curva.transform(1f), 0.001f)

            var anterior = curva.transform(0f)
            for (paso in 1..20) {
                val t = paso / 20f
                val actual = curva.transform(t)
                assertTrue(
                    "la curva tiene que avanzar, no retroceder, en t=$t",
                    actual >= anterior - 0.0001f,
                )
                anterior = actual
            }
        }
    }

    @Test
    fun `reduced es la unica fuente de la decision - no hay caso sin cubrir`() {
        // No existe un tercer valor de `reduced`: es un Boolean. Este test
        // documenta esa garantía en vez de dejarla implícita - si alguien
        // cambiara `reduced` a un enum de tres estados ("sistema", "on",
        // "off"), este archivo sería el primer lugar que pide actualizarse.
        assertTrue(RbMotion(reduced = true).reduced)
        assertTrue(!RbMotion(reduced = false).reduced)
    }
}
