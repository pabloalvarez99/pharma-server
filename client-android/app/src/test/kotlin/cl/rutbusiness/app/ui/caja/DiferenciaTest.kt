package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.money.Moneda
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La diferencia del cierre: de qué lado quedó y qué se le dice a la dueña.
 *
 * Estas frases están fijadas a propósito. Es la pantalla más delicada del día —
 * si se siente como una auditoría, la dueña deja de cerrar y el producto pierde
 * lo único que hace que el día sirva. Cambiar el texto tiene que romper el build
 * y obligar a pensarlo de nuevo, no pasar en un refactor.
 *
 * Nada de esto monta Compose: es lógica pura, así que corre en milisegundos y en
 * cada commit.
 */
class DiferenciaTest {

    private val pesos = Moneda.de("CLP")
    private val dolares = Moneda.de("USD")

    /** Jerga de cajero formal / auditoría que el feriante no debe ver. */
    private val jergaFeriaProhibida = listOf(
        "sistema",
        "cajón",
        "cajon",
        "arqueo",
        "sesión",
        "sesion",
        "transacción",
        "transaccion",
    )

    // --- de qué lado quedó ---------------------------------------------------

    @Test
    fun `una discrepancia negativa es plata que falta`() {
        val lectura = leerDiferencia("-2500")
        assertEquals(Cuadre.Falta, lectura.cuadre)
        // El signo se saca de la cadena, no restando: los dígitos son los mismos
        // que grabó el cierre.
        assertEquals("2500", lectura.magnitud)
    }

    @Test
    fun `una discrepancia positiva es plata que sobra`() {
        val lectura = leerDiferencia("1000")
        assertEquals(Cuadre.Sobra, lectura.cuadre)
        assertEquals("1000", lectura.magnitud)
    }

    @Test
    fun `cero es cuadrar`() {
        assertEquals(Cuadre.Justo, leerDiferencia("0").cuadre)
    }

    /** `0.00` y `0` son la misma nada: comparar cadenas fallaría en uno de los dos. */
    @Test
    fun `cero con decimales tambien es cuadrar`() {
        assertEquals(Cuadre.Justo, leerDiferencia("0.00").cuadre)
    }

    @Test
    fun `la escala del server se respeta tal cual`() {
        assertEquals("2500.75", leerDiferencia("-2500.75").magnitud)
    }

    /**
     * Sin dato no se dice "cuadró".
     *
     * Es el caso que importa: decirle a la dueña que cuadró cuando en
     * realidad no se pudo leer la diferencia es la única respuesta que hace que
     * deje de contar.
     */
    @Test
    fun `sin diferencia del server no se inventa que cuadro`() {
        assertEquals(Cuadre.Desconocido, leerDiferencia(null).cuadre)
        assertEquals(Cuadre.Desconocido, leerDiferencia("").cuadre)
        assertEquals(Cuadre.Desconocido, leerDiferencia("no es un numero").cuadre)
    }

    // --- lo que lee la dueña -------------------------------------------------

    @Test
    fun `cuando falta plata se dice sin acusar a nadie`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "15000",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "-2500",
        )

        assertEquals("Faltan $2.500", copy.titular)
        assertEquals("Contaste $15.000 y el sistema tenía anotados $17.500.", copy.explicacion)
        assertEquals(
            "Casi siempre es un vuelto de más o una compra chica que no se alcanzó a anotar. " +
                "Queda guardado así y mañana empiezas de nuevo.",
            copy.calma,
        )
    }

    /**
     * La regla de redacción, escrita como prueba.
     *
     * "te faltan" pone a la dueña de sujeto de la falta; "faltante" y "error"
     * son jerga contable que suena a auditoría. Ninguna de las tres puede
     * aparecer, y ésta es la prueba que lo impide en el próximo refactor.
     */
    @Test
    fun `el texto de la diferencia no acusa ni usa jerga`() {
        val prohibidas = listOf("te falta", "faltante", "error", "descuadre", "!", "responsab")

        listOf("-2500", "1000", "0").forEach { discrepancia ->
            listOf(false, true).forEach { feria ->
                val copy = copyDeDiferencia(pesos, "15000", "17500", discrepancia, feria = feria)
                val texto = "${copy.titular} ${copy.explicacion} ${copy.calma}".lowercase()
                prohibidas.forEach { palabra ->
                    assertFalse(
                        "con discrepancia «$discrepancia» feria=$feria el texto dice «$palabra»: $texto",
                        texto.contains(palabra),
                    )
                }
            }
        }
    }

    /**
     * "Queda guardado así y mañana empiezas de nuevo." no nombra ningún objeto
     * del negocio (a diferencia de "lo anotado" vs "sistema", o "caja" vs
     * "día"), así que no hay jerga de rubro que cambiar y la frase es la misma
     * en feria y en retail. Antes esto vivía como un `if (feria) ... else ...`
     * con las dos ramas idénticas -- código muerto que un refactor futuro
     * podía "completar" con una diferencia que nunca existió. Esta prueba fija
     * que la frase es una sola para que quien la toque lo haga a propósito.
     */
    @Test
    fun `la frase de manana es la misma en feria y retail porque no nombra nada del negocio`() {
        val feria = copyDeDiferencia(pesos, "15000", "17500", "-2500", feria = true)
        val retail = copyDeDiferencia(pesos, "15000", "17500", "-2500", feria = false)
        val frase = "Queda guardado así y mañana empiezas de nuevo."

        assertTrue(feria.calma.endsWith(frase))
        assertTrue(retail.calma.endsWith(frase))
    }

    @Test
    fun `cuando sobra plata tambien se explica solo`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "18500",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "1000",
        )

        assertEquals("Sobran $1.000", copy.titular)
        assertEquals("Contaste $18.500 y el sistema tenía anotados $17.500.", copy.explicacion)
    }

    @Test
    fun `cuando cuadra se dice que cuadro y no se muestra un cero`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "17500",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "0",
        )

        assertEquals("La caja cuadró", copy.titular)
        assertEquals("Contaste $17.500 y es justo lo que el sistema tenía anotado.", copy.explicacion)
        assertEquals("Listo por hoy.", copy.calma)
    }

    @Test
    fun `feria cuando cuadra habla del dia no de la caja ni del sistema`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "17500",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "0",
            feria = true,
        )
        assertEquals("El día cuadró", copy.titular)
        assertEquals("Contaste $17.500 y es justo lo que se había anotado.", copy.explicacion)
        assertEquals("Listo por hoy.", copy.calma)
        assertSinJergaFeria(copy)
    }

    @Test
    fun `feria cuando falta plata habla de lo anotado del dia`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "15000",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "-2500",
            feria = true,
        )

        assertEquals("Faltan $2.500", copy.titular)
        assertEquals("Contaste $15.000 y lo anotado del día era $17.500.", copy.explicacion)
        assertTrue(copy.calma.contains("empiezas"))
        assertSinJergaFeria(copy)
    }

    @Test
    fun `feria cuando sobra plata tambien habla del dia`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "18500",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "1000",
            feria = true,
        )

        assertEquals("Sobran $1.000", copy.titular)
        assertEquals("Contaste $18.500 y lo anotado del día era $17.500.", copy.explicacion)
        assertSinJergaFeria(copy)
    }

    @Test
    fun `sin comparacion se dice que falta el dato`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "15000",
            esperadoDelServidor = null,
            discrepanciaDelServidor = null,
        )

        assertEquals("Caja cerrada", copy.titular)
        assertEquals("La caja quedó cerrada con los $15.000 que contaste.", copy.explicacion)
        assert(copy.calma.contains("No pudimos traer la comparación"))
    }

    @Test
    fun `feria sin comparacion habla del puesto`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "15000",
            esperadoDelServidor = null,
            discrepanciaDelServidor = null,
            feria = true,
        )
        assertEquals("Día cerrado", copy.titular)
        assertEquals("El día quedó cerrado con los $15.000 que contaste.", copy.explicacion)
        assertTrue(copy.calma.lowercase().contains("puesto"))
        assertFalse(copy.calma.lowercase().contains("caja"))
        assertSinJergaFeria(copy)
    }

    /**
     * Gate de regresión: si alguien vuelve a meter "sistema" / "cajón" / "arqueo"
     * en el camino feria, el build se cae acá — en falta, sobra, justo y sin dato.
     */
    @Test
    fun `feria nunca dice sistema cajon arqueo sesion ni transaccion`() {
        val casos = listOf("-2500", "1000", "0", null, "")
        casos.forEach { discrepancia ->
            val copy = copyDeDiferencia(
                moneda = pesos,
                contadoDelServidor = "15000",
                esperadoDelServidor = "17500",
                discrepanciaDelServidor = discrepancia,
                feria = true,
            )
            assertSinJergaFeria(copy)
        }
        // Sin montos del server tampoco se cuela jerga.
        assertSinJergaFeria(
            copyDeDiferencia(
                moneda = pesos,
                contadoDelServidor = null,
                esperadoDelServidor = null,
                discrepanciaDelServidor = null,
                feria = true,
            ),
        )
    }

    @Test
    fun `el boton de cierre en feria cierra el cuaderno y no dice solo Listo`() {
        assertEquals("Listo por hoy", labelListoCierre(feria = true))
        assertEquals("Listo", labelListoCierre(feria = false))
        assertFalse(labelListoCierre(true).equals("Listo", ignoreCase = true))
    }

    /**
     * La moneda es por tenant y esta pantalla no la puede asumir.
     *
     * Un negocio en dólares tiene que leer `US$25,00` y no `$2.500`. Si el
     * teléfono formatea como si fueran pesos, la pantalla de cierre muestra un
     * número distinto del que tiene el server — y acá eso no es un bug de
     * formato, es una acusación falsa.
     */
    @Test
    fun `una moneda con decimales se escribe con sus decimales`() {
        val copy = copyDeDiferencia(
            moneda = dolares,
            contadoDelServidor = "150.00",
            esperadoDelServidor = "175.00",
            discrepanciaDelServidor = "-25.00",
        )

        assertEquals("Faltan US$25,00", copy.titular)
        assertEquals("Contaste US$150,00 y el sistema tenía anotados US$175,00.", copy.explicacion)
    }

    @Test
    fun `feria con moneda de decimales sigue sin jerga formal`() {
        val copy = copyDeDiferencia(
            moneda = dolares,
            contadoDelServidor = "150.00",
            esperadoDelServidor = "175.00",
            discrepanciaDelServidor = "-25.00",
            feria = true,
        )
        assertEquals("Faltan US$25,00", copy.titular)
        assertEquals("Contaste US$150,00 y lo anotado del día era US$175,00.", copy.explicacion)
        assertSinJergaFeria(copy)
    }

    private fun assertSinJergaFeria(copy: CopyDeDiferencia) {
        val texto = "${copy.titular} ${copy.explicacion} ${copy.calma}".lowercase()
        jergaFeriaProhibida.forEach { palabra ->
            assertFalse(
                "copy feria no debe decir «$palabra»: $texto",
                texto.contains(palabra.lowercase()),
            )
        }
    }
}
