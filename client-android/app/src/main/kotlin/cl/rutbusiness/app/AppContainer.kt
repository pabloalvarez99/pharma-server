package cl.rutbusiness.app

import android.content.Context
import cl.rutbusiness.app.entrada.PreferenciasDeEntradaAndroid
import cl.rutbusiness.app.entrada.RedDelTelefonoAndroid
import cl.rutbusiness.app.gente.CompartirConGenteAndroid
import cl.rutbusiness.app.impresora.ImpresoraBluetoothAndroid
import cl.rutbusiness.app.impresora.PreferenciasDeImpresoraAndroid
import cl.rutbusiness.app.ui.entrada.ServiciosDeEntrada
import cl.rutbusiness.app.ui.impresora.ServiciosDeImpresora
import cl.rutbusiness.app.ui.offline.ServiciosOffline
import cl.rutbusiness.core.net.contestaElServidor
import cl.rutbusiness.core.offline.CacheDeLecturas
import cl.rutbusiness.core.offline.ColaDeVentas
import cl.rutbusiness.core.offline.ConexionConElNegocio
import cl.rutbusiness.core.offline.DespachadorDeVentas
import cl.rutbusiness.core.offline.MonitorDeRed
import cl.rutbusiness.app.ui.rubro.ServiciosDeRubro
import cl.rutbusiness.core.rubro.RubroPackRepository
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.core.session.identidadGoogleDeEsteBuild
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.plus

/**
 * El grafo de dependencias completo, a mano.
 *
 * Sin Hilt ni Koin: son unos pocos objetos. Un framework de inyección acá solo
 * suma clases que cargar en el arranque, y el arranque en frío del teléfono
 * lento es justamente lo que estamos cuidando.
 *
 * El scope vive lo que vive el proceso: no hay nada que cancelar, porque cuando
 * el proceso muere se lleva todo.
 */
class AppContainer(context: Context) {
    private val almacenamiento = AlmacenamientoPlataforma(context.applicationContext)

    /** El reloj de pared. Es el que la dueña lee en la cola: "cobrada 14:32". */
    private val reloj: () -> Long = { System.currentTimeMillis() }

    /** Lo que dice Android del enlace: wifi o datos prendidos. */
    private val monitorDeRed = MonitorDeRed(context.applicationContext)

    /**
     * Si estamos llegando o no al sistema del negocio.
     *
     * Se construye **antes** que la sesión porque la sesión se lo pasa a cada
     * cliente HTTP que arma: así cualquier llamada de cualquier pantalla prende
     * o apaga el cartel, sin que ninguna pantalla tenga que acordarse.
     */
    val conexion = ConexionConElNegocio(monitorDeRed.hayEnlace)

    val sesion = SessionRepository(almacenamiento, conexion)

    /**
     * Pack de rubro del tenant (feria, farmacia, …). Vive a nivel de proceso:
     * se recarga al abrir sesión y se limpia al salir.
     */
    val rubroPack = RubroPackRepository()
    val rubro = ServiciosDeRubro(rubroPack)

    /** Lo último que contestó el server, para cuando no conteste. */
    val cache = CacheDeLecturas(almacenamiento.bloques, reloj)

    /** Las ventas cobradas que todavía no llegaron. Vive en disco. */
    val colaDeVentas = ColaDeVentas(almacenamiento.bloques, reloj)

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
     * Mandar un texto por el chat que la dueña ya usa (fase B).
     *
     * Contexto de aplicación y no de `Activity`: el `Intent` lleva
     * `FLAG_ACTIVITY_NEW_TASK`, así que el selector se monta igual. A diferencia
     * del selector de cuentas de Google, esto no es UI del sistema que necesite
     * una ventana previa.
     *
     * **Todavía no está enchufado a la composición.** Falta la línea de
     * `MainActivity` que lo provee —`LocalCompartirConGente provides
     * container.compartirConGente`—, que este asiento no puede tocar. Mientras
     * no esté, las pantallas leen `null` y esconden el botón, que es
     * exactamente lo que hacen cuando no hay plataforma detrás: no se rompe
     * nada, simplemente la función no aparece.
     */
    val compartirConGente = CompartirConGenteAndroid(context.applicationContext)

    /** La red y las preferencias, que no dependen de qué Activity esté viva. */
    private val redDelTelefono = RedDelTelefonoAndroid(context.applicationContext)
    private val preferenciasDeEntrada = PreferenciasDeEntradaAndroid(context.applicationContext)

    /**
     * Lo que necesita la pantalla de entrada.
     *
     * Igual de barato de construir que la impresora: guarda un `Context` y abre
     * un `SharedPreferences`. Nada toca la radio ni la red hasta que alguien
     * pregunta.
     *
     * Es una función y no un `val` por una sola razón: el selector de cuentas de
     * Google necesita el contexto de una **Activity** —es UI del sistema y tiene
     * que montarse sobre una ventana—, y este contenedor vive en la aplicación,
     * que no tiene ninguna. Con el contexto de la aplicación, Credential Manager
     * compila igual y falla recién cuando alguien toca el botón, o sea en el
     * teléfono de la dueña y no acá.
     *
     * @param contextoDeActivity el de `MainActivity`, no el de la aplicación.
     */
    fun entradaPara(contextoDeActivity: Context) = ServiciosDeEntrada(
        red = redDelTelefono,
        preferencias = preferenciasDeEntrada,
        // Sin `rb.googleClientId` al compilar esto es exactamente
        // `IdentidadGoogleNoCableada` —el mismo objeto de siempre— y el botón de
        // Google no aparece. Ver `app/build.gradle.kts` y el test
        // `IdentidadGoogleDeEsteBuildTest`.
        identidadGoogle = identidadGoogleDeEsteBuild(
            clientId = BuildConfig.GOOGLE_CLIENT_ID,
            contexto = { contextoDeActivity },
        ),
        // A dónde apunta "Crear mi negocio". Se decide al compilar
        // (`-Prb.urlNube=...`), no acá: ver el comentario en
        // `app/build.gradle.kts`. Vacío significa que este APK no trae nube, y
        // entonces el alta pregunta dónde guardar en vez de inventar un
        // destino; se traduce a `null` acá, una sola vez, para que ninguna
        // pantalla tenga que acordarse de que "" y null son lo mismo.
        nube = BuildConfig.URL_NUBE.takeIf { it.isNotBlank() },
    )

    private val scope = CoroutineScope(SupervisorJob())

    /** `true` mientras haya sesión. Es lo único que el despachador necesita saber. */
    private val haySesion = MutableStateFlow(false)

    val despachador = DespachadorDeVentas(
        cola = colaDeVentas,
        conexion = conexion,
        hayEnlace = monitorDeRed.hayEnlace,
        haySesion = haySesion,
        apiActiva = { sesion.apiActiva() },
        reloj = reloj,
    )

    /** El paquete que recibe la UI. Ver `ui/offline/ServiciosOffline.kt`. */
    val offline = ServiciosOffline(
        conexion = conexion,
        cola = colaDeVentas,
        cache = cache,
        despachador = despachador,
        reloj = reloj,
    )

    init {
        // Leer la sesión de disco arranca acá, en `Application.onCreate`, y no
        // colgado del primer `LaunchedEffect` de la UI. En el aparato lento esa
        // lectura tarda ~40 ms; disparada desde la UI corría después de que
        // Compose ya hubiera compuesto y dibujado el spinner de "Cargando", un
        // frame entero que se descartaba enseguida. Disparada acá se solapa con
        // la inicialización de Compose y el primer frame ya es la pantalla real.
        scope.launch { sesion.restaurar() }

        // La cola también se lee antes del primer frame, y por un motivo más
        // fuerte: si la app se cerró con ventas adentro, tienen que empezar a
        // salir apenas haya señal, no cuando alguien abra una pantalla.
        scope.launch { colaDeVentas.cargar() }

        scope.launch {
            sesion.estado.collect { haySesion.value = it is EstadoSesion.Activa }
        }

        // Pack de rubro: al entrar se pide al server; al salir se limpia.
        // Offline → fallback local (PacksLocales). No bloquea la UI.
        scope.launch {
            sesion.estado.collectLatest { estado ->
                when (estado) {
                    is EstadoSesion.Activa -> {
                        val api = sesion.apiActiva() ?: return@collectLatest
                        rubroPack.cargar(api)
                    }
                    is EstadoSesion.SinSesion -> rubroPack.limpiar()
                    EstadoSesion.Cargando -> Unit
                }
            }
        }

        // Cuando vuelve el enlace se pregunta si además volvió el server. Una
        // llamada chica por evento de red, nunca un sondeo periódico: los datos
        // móviles se pagan por mega.
        conexion.escuchar(scope) { sesion.apiActiva()?.contestaElServidor() ?: false }

        despachador.arrancar(scope)
    }
}
