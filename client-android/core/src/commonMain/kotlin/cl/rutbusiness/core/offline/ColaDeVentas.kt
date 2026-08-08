package cl.rutbusiness.core.offline

import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Una venta cobrada que todavía no llegó al sistema del negocio.
 *
 * [clave] es la clave de idempotencia, y es **la misma** que usa el cobro
 * normal: no se inventa una para la cola. Es lo que hace que reintentar sea
 * seguro — el server cachea la respuesta contra esa clave, así que el segundo
 * POST contesta 200 con la orden que ya creó en vez de cobrar de nuevo. Por eso
 * también es el identificador de la fila: dos filas con la misma clave serían
 * la misma venta contada dos veces.
 */
@Serializable
data class VentaEnCola(
    val clave: String,
    val solicitud: SolicitudDeVenta,
    /** Hora del teléfono cuando la dueña la cobró. Se muestra tal cual. */
    val cobradaEn: Long,
    /** Cuántos productos distintos lleva. Para mostrar sin abrir la venta. */
    val lineas: Int,
    val intentos: Int = 0,
    /** Cuándo se puede volver a intentar. Es el freno de los datos móviles. */
    val proximoIntentoEn: Long = 0L,
    /**
     * El server la rechazó y no va a cambiar de opinión (stock que no alcanza,
     * cliente borrado). Deja de reintentarse pero **no se borra sola**: la
     * dueña tiene que verla y decidir. Una venta que desaparece sin que nadie
     * la mire es plata que nadie sabe que se perdió.
     */
    val rechazada: Boolean = false,
    /** Lo que dijo el server, ya escrito para mostrar. */
    val motivo: String? = null,
) {
    val esperando: Boolean get() = !rechazada
}

/**
 * Las ventas que se cobraron sin red, guardadas en disco.
 *
 * **Sobrevive a que la app se cierre y a que el teléfono se apague**, y ése es
 * el punto entero: la venta se escribe a disco **antes** del primer intento de
 * mandarla, no después de que falle. Si se escribiera después, el hueco entre
 * el POST y la falla sería justo el momento en que se pierde una venta — y es
 * el momento en que más probable es que el teléfono se quede sin batería,
 * porque es cuando la radio está a full buscando señal.
 *
 * Todas las escrituras pasan por [candado]: es un solo archivo y dos corrutinas
 * escribiéndolo a la vez se pisarían. El despachador reintenta mientras la
 * pantalla encola, así que ese cruce no es teórico.
 */
class ColaDeVentas(
    private val almacen: AlmacenDeBloques,
    private val reloj: () -> Long,
) {

    private val _ventas = MutableStateFlow<List<VentaEnCola>>(emptyList())

    /** Lo que hay en la cola, para que la pantalla lo muestre. */
    val ventas: StateFlow<List<VentaEnCola>> = _ventas.asStateFlow()

    private val candado = Mutex()
    private var cargada = false

    /** Lee el archivo. Se llama al arrancar el proceso, antes de la UI. */
    suspend fun cargar() {
        candado.withLock {
            if (cargada) return
            cargada = true
            _ventas.value = leerDeDisco()
        }
    }

    /**
     * Anota una venta y la deja lista para mandarse.
     *
     * Vuelve `false` si la cola está llena. Llegar a [MAXIMO] con la red caída
     * es un negocio que lleva días sin sistema, y ahí lo honesto es cortar y
     * decirlo: seguir aceptando ventas que nadie va a poder mandar es acumular
     * un problema más grande en silencio.
     *
     * Si la clave ya está en la cola, no se duplica la fila — un doble toque en
     * "Guardar venta" es una sola venta.
     */
    suspend fun encolar(venta: VentaEnCola): Boolean = candado.withLock {
        val actuales = _ventas.value
        if (actuales.any { it.clave == venta.clave }) return@withLock true
        if (actuales.size >= MAXIMO) return@withLock false
        guardar(actuales + venta)
        true
    }

    /** La venta llegó al server: se va de la cola. */
    suspend fun quitar(clave: String) = candado.withLock {
        guardar(_ventas.value.filterNot { it.clave == clave })
    }

    /**
     * El intento falló por red. Suma un intento y corre el próximo.
     *
     * La espera se duplica hasta [ESPERA_MAXIMA_MS]. Reintentar en loop apretado
     * contra un teléfono sin señal quema batería y megas de datos que en Chile
     * se pagan caros, y no llega antes: si no hay señal, no hay señal.
     */
    suspend fun postergar(clave: String) = candado.withLock {
        guardar(
            _ventas.value.map { venta ->
                if (venta.clave != clave) {
                    venta
                } else {
                    val intentos = venta.intentos + 1
                    venta.copy(
                        intentos = intentos,
                        proximoIntentoEn = reloj() + esperaTras(intentos),
                    )
                }
            },
        )
    }

    /** El server la rechazó. Deja de intentarse y queda a la vista. */
    suspend fun rechazar(clave: String, motivo: String) = candado.withLock {
        guardar(
            _ventas.value.map { venta ->
                if (venta.clave == clave) {
                    venta.copy(rechazada = true, motivo = motivo, intentos = venta.intentos + 1)
                } else {
                    venta
                }
            },
        )
    }

    /**
     * Borra el backoff de todas: la próxima vuelta las intenta ya.
     *
     * Lo usa el botón "Intentar ahora". El reintento automático puede estar
     * durmiendo cinco minutos justo cuando la dueña ve volver el wifi, y
     * hacerla esperar mirando el cartel es la desconfianza que la cola existe
     * para evitar.
     */
    suspend fun apurar() = candado.withLock {
        guardar(_ventas.value.map { if (it.esperando) it.copy(proximoIntentoEn = 0L) else it })
    }

    /**
     * Saca una venta rechazada.
     *
     * Sólo rechazadas, y sólo a mano: una venta que todavía se puede mandar no
     * se descarta desde ninguna pantalla, porque el único motivo para borrarla
     * sería un error de la app y ahí lo que se pierde es plata cobrada.
     */
    suspend fun descartar(clave: String) = candado.withLock {
        guardar(_ventas.value.filterNot { it.clave == clave && it.rechazada })
    }

    /**
     * Fusiona ventas de un snapshot de respaldo (ADR-0022 restore).
     *
     * - No duplica por [VentaEnCola.clave] (la que ya está gana).
     * - Respeta [MAXIMO]: no mete más de lo que cabe.
     * - Las que entran salen con intentos en 0 y listas para mandarse ya.
     *
     * Devuelve cuántas filas **nuevas** se agregaron.
     */
    suspend fun fusionarDesdeRespaldo(entrantes: List<VentaEnCola>): Int = candado.withLock {
        if (entrantes.isEmpty()) return@withLock 0
        val actuales = _ventas.value
        val claves = actuales.map { it.clave }.toHashSet()
        val nuevas = mutableListOf<VentaEnCola>()
        for (v in entrantes) {
            if (v.clave.isBlank()) continue
            if (v.clave in claves) continue
            if (actuales.size + nuevas.size >= MAXIMO) break
            nuevas.add(
                v.copy(
                    intentos = 0,
                    proximoIntentoEn = 0L,
                    rechazada = false,
                    motivo = null,
                ),
            )
            claves.add(v.clave)
        }
        if (nuevas.isEmpty()) return@withLock 0
        guardar(actuales + nuevas)
        nuevas.size
    }

    /** La primera venta a la que le toca salir, o `null` si no hay ninguna lista. */
    fun proxima(): VentaEnCola? {
        val ahora = reloj()
        return _ventas.value.firstOrNull { it.esperando && it.proximoIntentoEn <= ahora }
    }

    /** Cuántas están esperando salir. Es el número que ve la dueña. */
    val cuantasEsperan: Int get() = _ventas.value.count { it.esperando }

    // --- disco ---------------------------------------------------------------

    private suspend fun guardar(nuevas: List<VentaEnCola>) {
        _ventas.value = nuevas
        val texto = runCatching {
            JSON.encodeToString(ColaGuardada.serializer(), ColaGuardada(ventas = nuevas))
        }.getOrNull() ?: return
        almacen.escribir(ARCHIVO, texto)
    }

    private suspend fun leerDeDisco(): List<VentaEnCola> {
        val texto = almacen.leer(ARCHIVO) ?: return emptyList()
        val guardada = runCatching {
            JSON.decodeFromString(ColaGuardada.serializer(), texto)
        }.getOrNull() ?: return emptyList()
        // Un archivo de una versión futura no se toca: mejor no mandar nada que
        // mandar algo que no se entiende. Se deja donde está para que una app
        // más nueva lo levante.
        if (guardada.version > VERSION) return emptyList()
        return guardada.ventas
    }

    @Serializable
    private data class ColaGuardada(
        val version: Int = VERSION,
        val ventas: List<VentaEnCola> = emptyList(),
    )

    companion object {
        /**
         * Cuántas ventas aguanta la cola.
         *
         * Un almacén de barrio hace del orden de cien boletas al día, así que
         * 200 cubre un día entero sin sistema con margen. En disco son unas
         * decenas de KB, y en memoria sólo lo que se está mostrando.
         */
        const val MAXIMO = 200

        /** Primera espera tras un intento fallido. */
        const val ESPERA_INICIAL_MS = 5_000L

        /** Techo de la espera: cinco minutos. */
        const val ESPERA_MAXIMA_MS = 5 * 60_000L

        /** 5 s, 10 s, 20 s, 40 s, 80 s, 160 s, y de ahí 5 min fijos. */
        fun esperaTras(intentos: Int): Long {
            var espera = ESPERA_INICIAL_MS
            repeat((intentos - 1).coerceAtLeast(0)) {
                if (espera >= ESPERA_MAXIMA_MS) return ESPERA_MAXIMA_MS
                espera *= 2
            }
            return espera.coerceAtMost(ESPERA_MAXIMA_MS)
        }

        private const val ARCHIVO = "ventas-pendientes.json"
        private const val VERSION = 1

        private val JSON = Json {
            ignoreUnknownKeys = true
            explicitNulls = false
        }
    }
}
