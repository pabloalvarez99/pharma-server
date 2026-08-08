package cl.rutbusiness.app.impresion

import cl.rutbusiness.app.ui.impresora.AnchoDePapel
import cl.rutbusiness.app.ui.impresora.EnlaceDeImpresora
import cl.rutbusiness.app.ui.impresora.EscPos
import cl.rutbusiness.app.ui.impresora.EstadoDePapel
import cl.rutbusiness.app.ui.impresora.FallaDeImpresion
import cl.rutbusiness.app.ui.impresora.ImpresoraConocida
import cl.rutbusiness.app.ui.impresora.ImpresoraElegida
import cl.rutbusiness.app.ui.impresora.Intento
import cl.rutbusiness.app.ui.impresora.PreferenciasDeImpresora
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import cl.rutbusiness.app.impresora.ImpresoraBluetoothAndroid
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * La capa de Android de verdad, corriendo en un aparato de verdad.
 *
 * Lo que las pruebas de la JVM **no** pueden cubrir: que
 * [ImpresoraBluetoothAndroid] hable con el framework sin reventar. Ahí viven
 * los errores que un doble nunca reproduce — un `SecurityException` que no se
 * atrapó, un `getSystemService` que devuelve `null`, un permiso mal escrito en
 * el manifiesto.
 *
 * No prueba que imprima: para eso hace falta una impresora física emparejada, y
 * el emulador no tiene radio Bluetooth. Lo que sí prueba es que **cuando no se
 * puede, se degrada bien**: cada llamada contesta un [Intento.Fallo] con un
 * mensaje en castellano, y ninguna se lleva puesto el proceso. Ése es
 * exactamente el camino que va a recorrer el teléfono de la dueña el día que la
 * impresora esté apagada.
 *
 * Se corre con `./gradlew :app:connectedDebugAndroidTest` y un aparato o
 * emulador enchufado.
 */
@RunWith(AndroidJUnit4::class)
class ImpresoraBluetoothAndroidTest {

    private val enlace = ImpresoraBluetoothAndroid(
        InstrumentationRegistry.getInstrumentation().targetContext,
    )

    /**
     * Los permisos que se piden son los de **este** Android, no los de todos.
     *
     * Hasta Android 11 la lista va vacía porque `BLUETOOTH` se concede al
     * instalar; desde Android 12 aparece `BLUETOOTH_CONNECT`. Si esta lógica se
     * invierte, la app le pide a un Android 8 un permiso que no existe y el
     * diálogo nunca aparece: la dueña queda mirando un botón que no hace nada.
     */
    @Test
    fun losPermisosCorrespondenALaVersionDeAndroid() {
        val esperados = if (android.os.Build.VERSION.SDK_INT >= 31) {
            listOf("android.permission.BLUETOOTH_CONNECT")
        } else {
            emptyList()
        }
        assertTrue(
            "en SDK ${android.os.Build.VERSION.SDK_INT} se pidieron ${enlace.permisosQuePedir}",
            enlace.permisosQuePedir == esperados,
        )
    }

    /** BLUETOOTH_SCAN no se pide nunca: la app no descubre, sólo lee emparejadas. */
    @Test
    fun nuncaSePideElPermisoDeEscaneo() {
        assertTrue(
            "pedir BLUETOOTH_SCAN sería pedir «buscar dispositivos cercanos» sin usarlo",
            enlace.permisosQuePedir.none { it.contains("SCAN") },
        )
    }

    /**
     * Listar no explota pase lo que pase.
     *
     * Sin radio, sin permiso, con el Bluetooth apagado o sin nada emparejado:
     * las cuatro contestan un fallo con texto para la dueña. Ninguna sube una
     * excepción a la UI.
     */
    @Test
    fun listarNuncaExplotaYSiempreExplicaQueHacer() {
        when (val r = enlace.emparejadas()) {
            is Intento.Ok -> assertNotNull("la lista no puede ser nula", r.valor)
            is Intento.Fallo -> {
                assertTrue("un título vacío no le dice nada a nadie", r.falla.titulo.isNotBlank())
                assertTrue(
                    "«${r.falla.titulo}» no dice qué hacer",
                    r.falla.queHacer.isNotBlank(),
                )
            }
        }
    }

    /**
     * Imprimir contra una MAC que no existe falla ordenado y **rápido**.
     *
     * Es el caso de la impresora que alguien desemparejó: tiene que caer en
     * "ya no está emparejada" o "no contesta", nunca en una excepción ni en un
     * cuelgue. Si esto se colgara, la cajera vería la pantalla congelada con el
     * cliente esperando.
     */
    @Test
    fun imprimirAUnaImpresoraInexistenteFallaOrdenado() {
        val inexistente = ImpresoraElegida(
            direccion = "00:00:00:00:00:00",
            nombre = "Impresora fantasma",
            ancho = AnchoDePapel.Mm58,
        )
        val bytes = ConstructorDeBoleta(AnchoDePapel.Mm58).izquierda("prueba").cortar().construir()

        val resultado = runBlocking { enlace.imprimir(inexistente, bytes) }

        assertTrue("tenía que fallar: no existe esa impresora", resultado is Intento.Fallo)
        val falla = (resultado as Intento.Fallo).falla
        assertTrue(
            "«${falla.titulo}» tiene que decir qué hacer",
            falla.queHacer.isNotBlank(),
        )
    }

    /** El huso del aparato, que es con el que se fecha la boleta. */
    @Test
    fun elDesfaseHorarioEsElDelAparato() {
        val desfase = enlace.desfaseHorarioMinutos()
        assertTrue("desfase fuera de rango: $desfase", desfase in -12 * 60..14 * 60)
    }
}
