package cl.rutbusiness.app

import android.content.Context
import cl.rutbusiness.app.entrada.PreferenciasDeEntradaAndroid
import cl.rutbusiness.app.entrada.RedDelTelefonoAndroid
import cl.rutbusiness.app.impresora.ImpresoraBluetoothAndroid
import cl.rutbusiness.app.impresora.PreferenciasDeImpresoraAndroid
import cl.rutbusiness.app.ui.entrada.ServiciosDeEntrada
import cl.rutbusiness.app.ui.impresora.ServiciosDeImpresora
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.plus

/**
 * El grafo de dependencias completo, a mano.
 *
 * Sin Hilt ni Koin: son tres objetos. Un framework de inyección acá solo suma
 * clases que cargar en el arranque, y el arranque en frío del teléfono lento es
 * justamente lo que estamos cuidando.
 *
 * El scope vive lo que vive el proceso: no hay nada que cancelar, porque cuando
 * el proceso muere se lleva todo. Existe para una sola cosa, y es la de abajo.
 */
class AppContainer(context: Context) {
    private val almacenamiento = AlmacenamientoPlataforma(context.applicationContext)
    val sesion = SessionRepository(almacenamiento)

    /**
     * La impresora térmica.
     *
     * Se arma acá, pero **no se toca nada del Bluetooth**: el adaptador se pide
     * recién cuando alguien va a imprimir. Construir esto es guardar un
     * `Context` y abrir un `SharedPreferences`, así que no le suma nada al
     * arranque en frío que este archivo justamente cuida.
     */
    val impresora = ServiciosDeImpresora(
        sesion = sesion,
        enlace = ImpresoraBluetoothAndroid(context.applicationContext),
        preferencias = PreferenciasDeImpresoraAndroid(context.applicationContext),
    )

    /**
     * Lo que necesita la pantalla de entrada.
     *
     * Igual de barato de construir que la impresora: guarda un `Context` y abre
     * un `SharedPreferences`. Nada toca la radio ni la red hasta que alguien
     * pregunta.
     */
    val entrada = ServiciosDeEntrada(
        red = RedDelTelefonoAndroid(context.applicationContext),
        preferencias = PreferenciasDeEntradaAndroid(context.applicationContext),
    )

    private val scope = CoroutineScope(SupervisorJob())

    init {
        // Leer la sesión de disco arranca acá, en `Application.onCreate`, y no
        // colgado del primer `LaunchedEffect` de la UI. En el aparato lento esa
        // lectura tarda ~40 ms; disparada desde la UI corría después de que
        // Compose ya hubiera compuesto y dibujado el spinner de "Cargando", un
        // frame entero que se descartaba enseguida. Disparada acá se solapa con
        // la inicialización de Compose y el primer frame ya es la pantalla real.
        scope.launch { sesion.restaurar() }
    }
}
