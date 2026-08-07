package cl.rutbusiness.core.money

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class DineroTest {

    @Test
    fun `lee un entero sin coma`() {
        val d = assertNotNull(Dinero.deTextoDeServidor("1490"))
        assertEquals(Dinero(1490, 0), d)
        assertEquals("1490", d.aTextoDeServidor())
    }

    @Test
    fun `lee un decimal conservando la escala`() {
        val d = assertNotNull(Dinero.deTextoDeServidor("12.50"))
        assertEquals(Dinero(1250, 2), d)
        assertEquals("12.50", d.aTextoDeServidor())
    }

    @Test
    fun `suma alineando escalas distintas`() {
        val a = assertNotNull(Dinero.deTextoDeServidor("1490"))
        val b = assertNotNull(Dinero.deTextoDeServidor("0.50"))
        assertEquals("1490.50", (a + b).aTextoDeServidor())
    }

    @Test
    fun `multiplica por cantidad sin tocar la escala`() {
        val a = assertNotNull(Dinero.deTextoDeServidor("1490"))
        assertEquals("2980", (a * 2).aTextoDeServidor())
    }

    @Test
    fun `un texto que no es decimal devuelve null en vez de cero`() {
        // Un cero inventado se cobra. Prefiere no saber.
        assertNull(Dinero.deTextoDeServidor("mil"))
        assertNull(Dinero.deTextoDeServidor("1.2.3"))
        assertNull(Dinero.deTextoDeServidor(""))
    }

    @Test
    fun `negativos ida y vuelta`() {
        val d = assertNotNull(Dinero.deTextoDeServidor("-250.75"))
        assertEquals("-250.75", d.aTextoDeServidor())
    }
}

class MonedaTest {

    @Test
    fun `el default arranca sin explotar`() {
        // Regresión: `POR_DEFECTO` estaba declarado antes de las tablas del
        // companion y arrancaba con ellas en null, tumbando la app en el primer
        // frame. El orden de declaración es parte del contrato.
        assertEquals("CLP", Moneda.POR_DEFECTO.codigo)
        assertEquals(0, Moneda.POR_DEFECTO.decimales)
    }

    @Test
    fun `CLP no lleva decimales y agrupa de a tres`() {
        assertEquals("$1.490", Moneda.de("CLP").formatear("1490"))
        assertEquals("$999", Moneda.de("CLP").formatear("999"))
        assertEquals("$1.234.567", Moneda.de("CLP").formatear("1234567"))
    }

    @Test
    fun `USD lleva dos decimales aunque el server mande el entero pelado`() {
        assertEquals("US$12,00", Moneda.de("USD").formatear("12"))
        assertEquals("US$12,50", Moneda.de("USD").formatear("12.5"))
    }

    @Test
    fun `una moneda desconocida cae en dos decimales, nunca en cero`() {
        val rara = Moneda.de("XYZ")
        assertEquals(2, rara.decimales)
        assertEquals("XYZ 12,00", rara.formatear("12"))
    }

    @Test
    fun `nunca recorta un digito significativo`() {
        // CLP declara 0 decimales, pero si el server igual mandó 50 centavos,
        // mostrarlos: cortar sería mostrar un número distinto al que se cobra.
        assertEquals("$1.490,5", Moneda.de("CLP").formatear("1490.50"))
        assertEquals("$1.490", Moneda.de("CLP").formatear("1490.00"))
    }

    @Test
    fun `tres decimales para las monedas que los usan`() {
        assertEquals(3, Moneda.de("KWD").decimales)
        assertEquals("KWD 1,500", Moneda.de("KWD").formatear("1.5"))
    }

    @Test
    fun `un monto ilegible se muestra tal cual y no como cero`() {
        assertEquals("no-es-plata", Moneda.de("CLP").formatear("no-es-plata"))
    }
}
