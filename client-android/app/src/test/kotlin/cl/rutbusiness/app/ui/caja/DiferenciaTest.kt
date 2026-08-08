package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.money.Moneda
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * La diferencia del arqueo: de qué lado quedó y qué se le dice a la dueña.
 *
 * Estas frases están fijadas a propósito. Es la pantalla más delicada del día —
 * si se siente como una auditoría, la dueña deja de cerrar caja y el producto
 * pierde lo único que hace que la caja sirva. Cambiar el texto tiene que romper
 * el build y obligar a pensarlo de nuevo, no pasar en un refactor.
 *
 * Nada de esto monta Compose: es lógica pura, así que corre en milisegundos y en
 * cada commit.
 */
class DiferenciaTest {

    private val pesos = Moneda.de("CLP")
    private val dolares = Moneda.de("USD")

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
     * Es el caso que importa: decirle a la dueña que la caja cuadró cuando en
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
            val copy = copyDeDiferencia(pesos, "15000", "17500", discrepancia)
            val texto = "${copy.titular} ${copy.explicacion} ${copy.calma}".lowercase()
            prohibidas.forEach { palabra ->
                assert(!texto.contains(palabra)) {
                    "con discrepancia «$discrepancia» el texto dice «$palabra»: $texto"
                }
            }
        }
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

    /**
     * La moneda es por tenant y esta pantalla no la puede asumir.
     *
     * Un negocio en dólares tiene que leer `US$25,00` y no `$2.500`. Si el
     * teléfono formatea como si fueran pesos, la pantalla de arqueo muestra un
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
}
