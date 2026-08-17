package cl.rutbusiness.ui.components

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La geometría de [RbChart], sin Compose ni Android — puro JVM.
 *
 * Cada test acá es uno de los bordes que un gráfico de barras rompe en
 * silencio: normalizar mal contra el máximo dibuja una semana entera plana,
 * un solo día con ventas puede esconder los otros seis, y todos los días en
 * cero puede dividir por cero y tumbar el `Canvas`.
 */
class RbChartTest {

    private val ancho = 350f
    private val alto = 100f
    private val minAlto = 4f

    @Test
    fun `normaliza cada barra contra el maximo de la serie`() {
        val datos = listOf(
            RbChartDato("a", 10f),
            RbChartDato("b", 20f),
            RbChartDato("c", 40f),
        )
        val barras = calcularBarrasDeGrafico(datos, ancho, alto, minAltoBarraPx = minAlto)

        // El máximo (40) ocupa el alto entero disponible.
        assertEquals(alto, barras[2].altoBarraPx, 0.01f)
        // Las demás son la fracción exacta de ese máximo.
        assertEquals(alto * 0.25f, barras[0].altoBarraPx, 0.01f)
        assertEquals(alto * 0.5f, barras[1].altoBarraPx, 0.01f)
    }

    @Test
    fun `una semana con un solo dia de ventas no esconde los otros seis`() {
        val datos = listOf(
            RbChartDato("lu", 0f),
            RbChartDato("ma", 0f),
            RbChartDato("mi", 0f),
            RbChartDato("ju", 0f),
            RbChartDato("vi", 0f),
            RbChartDato("sá", 50_000f),
            RbChartDato("do", 0f),
        )
        val barras = calcularBarrasDeGrafico(datos, ancho, alto, minAltoBarraPx = minAlto)

        assertEquals(alto, barras[5].altoBarraPx, 0.01f)
        // Los seis días en cero siguen siendo barras visibles, no líneas
        // invisibles pegadas al piso.
        barras.filterIndexed { indice, _ -> indice != 5 }.forEach { barra ->
            assertEquals(minAlto, barra.altoBarraPx, 0.01f)
        }
    }

    @Test
    fun `todos los dias en cero no divide por cero`() {
        val datos = List(7) { RbChartDato("d$it", 0f) }
        val barras = calcularBarrasDeGrafico(datos, ancho, alto, minAltoBarraPx = minAlto)

        assertEquals(7, barras.size)
        barras.forEach { barra ->
            assertEquals(minAlto, barra.altoBarraPx, 0.01f)
            assertTrue("un dia en cero no puede dar NaN", barra.altoBarraPx.isFinite())
        }
    }

    @Test
    fun `un valor negativo se recorta a cero antes de normalizar`() {
        val datos = listOf(RbChartDato("a", -10f), RbChartDato("b", 20f))
        val barras = calcularBarrasDeGrafico(datos, ancho, alto, minAltoBarraPx = minAlto)

        assertEquals(minAlto, barras[0].altoBarraPx, 0.01f)
        assertEquals(alto, barras[1].altoBarraPx, 0.01f)
    }

    @Test
    fun `la barra destacada conserva la marca al calcular`() {
        val datos = listOf(
            RbChartDato("lu", 10f),
            RbChartDato("ma", 20f, destacado = true),
        )
        val barras = calcularBarrasDeGrafico(datos, ancho, alto)

        assertTrue(barras[1].destacado)
        assertTrue(!barras[0].destacado)
    }

    @Test
    fun `las barras no se superponen y llenan el ancho disponible`() {
        val datos = List(7) { RbChartDato("d$it", it.toFloat()) }
        val espacio = 6f
        val barras = calcularBarrasDeGrafico(
            datos,
            anchoTotalPx = ancho,
            altoDisponiblePx = alto,
            espacioEntreBarrasPx = espacio,
        )

        for (indice in 0 until barras.lastIndex) {
            val actual = barras[indice]
            val siguiente = barras[indice + 1]
            assertTrue(
                "la barra $indice se mete en la ${indice + 1}",
                actual.x + actual.anchoBarra <= siguiente.x + 0.01f,
            )
        }
        val ultima = barras.last()
        assertTrue(
            "la ultima barra no puede pasarse del ancho disponible",
            ultima.x + ultima.anchoBarra <= ancho + 0.01f,
        )
    }

    @Test
    fun `sin datos no hay barras que dibujar`() {
        assertTrue(calcularBarrasDeGrafico(emptyList(), ancho, alto).isEmpty())
    }

    @Test
    fun `sin espacio disponible no hay barras que dibujar`() {
        val datos = listOf(RbChartDato("a", 10f))
        assertTrue(calcularBarrasDeGrafico(datos, 0f, alto).isEmpty())
        assertTrue(calcularBarrasDeGrafico(datos, ancho, 0f).isEmpty())
    }
}
