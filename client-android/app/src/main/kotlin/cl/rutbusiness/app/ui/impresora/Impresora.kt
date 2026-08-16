package cl.rutbusiness.app.ui.impresora

/**
 * Qué hacer cuando el teléfono no tiene Bluetooth, según rubro.
 *
 * Feria no tiene computador de día 1 (ADR-0022): la venta quedó anotada y no se
 * promete reimprimir en un PC. Farmacia / retail formal conserva la salida por
 * el computador del negocio.
 */
internal fun copyQueHacerSinBluetooth(feria: Boolean): String =
    if (feria) {
        "No se puede imprimir desde este teléfono. La venta igual quedó anotada."
    } else {
        "No se puede imprimir desde acá. La venta igual quedó registrada y la " +
            "boleta se puede imprimir después desde el computador del negocio."
    }

/**
 * Una impresora que el teléfono ya conoce.
 *
 * @param direccion la MAC. Es lo que identifica de verdad a la impresora: el
 *   nombre lo puede cambiar cualquiera desde los ajustes del teléfono.
 */
data class ImpresoraConocida(val direccion: String, val nombre: String)

/**
 * La impresora del negocio, ya elegida y recordada.
 *
 * Se guarda una vez y no se vuelve a preguntar. La dueña configura esto el día
 * que compra la impresora, no todos los días: cada pregunta repetida es una
 * oportunidad de contestar mal.
 */
data class ImpresoraElegida(
    val direccion: String,
    val nombre: String,
    val ancho: AnchoDePapel,
)

/**
 * Todo lo que puede salir mal entre el teléfono y el papel, en categorías que
 * la dueña entiende.
 *
 * La regla, la misma que en `AppError`: cada caso dice **qué hacer**, no qué
 * falló por dentro. "Error de impresión" no le sirve a nadie parado frente a un
 * cliente; "puede estar apagada o lejos, enciéndela y toca Reintentar" sí.
 *
 * @param sePuedeReintentar `false` cuando volver a tocar el botón no puede
 *   cambiar nada y hay que ir a arreglar algo afuera de la app.
 */
sealed class FallaDeImpresion(
    val titulo: String,
    val queHacer: String,
    val sePuedeReintentar: Boolean,
    /** Detalle técnico. Nunca se muestra; sirve para diagnosticar. */
    val tecnico: String? = null,
) {
    /**
     * Android 12 y posteriores piden permiso para hablar con aparatos
     * Bluetooth, y hay que pedírselo a la dueña en el momento.
     */
    class FaltaPermiso : FallaDeImpresion(
        titulo = "Falta darle permiso al Bluetooth",
        // El nombre acá no es decorativo: es la instrucción de dónde tocar. En
        // Ajustes › Aplicaciones el teléfono lista la app por su `app_name`, así
        // que este texto tiene que decir exactamente eso — si nombra otro
        // producto, la dueña recorre la lista buscando algo que no existe. Por
        // eso queda atado a `app_name` en `NombreDelProductoTest`.
        queHacer = "Toca «Dar permiso» y acepta. Es una sola vez. Si no te aparece el aviso, " +
            "Android ya lo tiene negado: entra a Ajustes › Aplicaciones › RutAgent › " +
            "Permisos y activa «Dispositivos cercanos».",
        sePuedeReintentar = true,
    )

    /**
     * Tablet o teléfono sin radio Bluetooth. Raro, pero existe.
     *
     * @param feria pack feria (ADR-0022): no manda a un computador ni habla de
     *   boleta; la venta se anotó y listo. Default `false` para los call sites
     *   del enlace Bluetooth (sin CompositionLocal); la UI re-escribe con
     *   [copyQueHacerSinBluetooth] si el rubro es feria.
     */
    class SinBluetooth(feria: Boolean = false) : FallaDeImpresion(
        titulo = "Este teléfono no tiene Bluetooth",
        queHacer = copyQueHacerSinBluetooth(feria),
        sePuedeReintentar = false,
    )

    class BluetoothApagado : FallaDeImpresion(
        titulo = "El Bluetooth está apagado",
        queHacer = "Enciéndelo deslizando desde arriba de la pantalla, y toca «Reintentar».",
        sePuedeReintentar = true,
    )

    /** Nunca se emparejó ninguna impresora con este teléfono. */
    class NingunaEmparejada : FallaDeImpresion(
        titulo = "Todavía no hay ninguna impresora",
        queHacer = "Enciende la impresora y emparéjala una vez desde Ajustes › Bluetooth del " +
            "teléfono (la clave suele ser 0000 o 1234). Después vuelve acá y elígela de la lista.",
        sePuedeReintentar = true,
    )

    /** La que estaba guardada ya no figura entre las emparejadas. */
    class YaNoEstaEmparejada(nombre: String) : FallaDeImpresion(
        titulo = "«$nombre» ya no está emparejada",
        queHacer = "Alguien la desvinculó del teléfono. Emparéjala de nuevo desde Ajustes › " +
            "Bluetooth, o toca «Cambiar impresora» y elige otra.",
        sePuedeReintentar = true,
    )

    /**
     * Está emparejada pero no contesta.
     *
     * Apagada y fuera de alcance se ven **idénticas** desde el teléfono, así
     * que se dicen las dos en orden de probabilidad en vez de prometer una
     * precisión que no tenemos y mandar a revisar lo que no era.
     */
    class NoContesta(nombre: String, tecnico: String? = null) : FallaDeImpresion(
        titulo = "«$nombre» no contesta",
        queHacer = "Puede estar apagada o quedar muy lejos. Revisa que tenga luz encendida, " +
            "acércate un poco y toca «Reintentar».",
        sePuedeReintentar = true,
        tecnico = tecnico,
    )

    class SinPapel(nombre: String) : FallaDeImpresion(
        titulo = "«$nombre» se quedó sin papel",
        queHacer = "Cambia el rollo, cierra la tapa y toca «Reintentar».",
        sePuedeReintentar = true,
    )

    /** Se cayó la conexión con la boleta a medio salir. */
    class SeCortoAMitad(nombre: String, tecnico: String? = null) : FallaDeImpresion(
        titulo = "Se cortó la impresión",
        queHacer = "Puede que la boleta haya salido a medias. Revisa el papel y toca " +
            "«Reintentar»: reimprimir no vuelve a cobrar.",
        sePuedeReintentar = true,
        tecnico = tecnico,
    )

    /** No se pudo traer la boleta del server para imprimirla. */
    class SinDatosDeLaBoleta(tecnico: String? = null) : FallaDeImpresion(
        titulo = "No pudimos traer la boleta",
        queHacer = "La venta está cobrada y guardada. Revisa la señal y toca «Reintentar».",
        sePuedeReintentar = true,
        tecnico = tecnico,
    )

    class Desconocida(tecnico: String? = null) : FallaDeImpresion(
        titulo = "No se pudo imprimir",
        queHacer = "Revisa que la impresora esté encendida, con papel y cerca, y toca " +
            "«Reintentar».",
        sePuedeReintentar = true,
        tecnico = tecnico,
    )
}

/** Salió bien, o salió mal con una explicación. */
sealed interface Intento<out T> {
    data class Ok<T>(val valor: T) : Intento<T>
    data class Fallo(val falla: FallaDeImpresion) : Intento<Nothing>
}

/**
 * Lo que el teléfono sabe hacer con una impresora térmica.
 *
 * Ésta es la frontera de plataforma del módulo, el equivalente al
 * `expect`/`actual` de `AlmacenamientoPlataforma`: de acá para arriba no hay un
 * solo `android.*`, así que las pantallas compilan igual el día que exista el
 * target de iOS y sólo haya que escribir la otra implementación (CoreBluetooth
 * o el AirPrint del caso).
 *
 * **Nunca escanea.** Sólo lee las impresoras que el teléfono ya tiene
 * emparejadas. Un descubrimiento Bluetooth activo mantiene la radio despierta y
 * se come la batería del aparato viejo, y no hace falta: emparejar es algo que
 * se hace una vez desde los ajustes del sistema, que además es donde la dueña
 * ya sabe hacerlo porque es donde emparejó sus audífonos.
 */
interface EnlaceDeImpresora {

    /**
     * Los permisos que hay que pedirle a la dueña en **este** Android, o vacío
     * si no hace falta ninguno.
     *
     * Son cadenas opacas para quien las recibe. Android 12 los cambió y el
     * aparato objetivo puede estar de cualquiera de los dos lados; quién
     * responde eso es la implementación, no la pantalla.
     */
    val permisosQuePedir: List<String>

    /** `true` si falta alguno de [permisosQuePedir]. */
    fun faltaPermiso(): Boolean

    /** Si el aparato tiene radio Bluetooth. */
    fun hayBluetooth(): Boolean

    /** Si está encendida ahora. */
    fun bluetoothEncendido(): Boolean

    /** Las impresoras ya emparejadas con el teléfono. */
    fun emparejadas(): Intento<List<ImpresoraConocida>>

    /**
     * Manda los bytes y espera a que salgan.
     *
     * Suspende: abre el socket, escribe y cierra. Nada queda conectado después,
     * que es la otra mitad de no gastar batería.
     */
    suspend fun imprimir(impresora: ImpresoraElegida, bytes: ByteArray): Intento<Unit>

    /**
     * Minutos entre UTC y la hora del teléfono, para fechar la boleta en la
     * hora del negocio.
     */
    fun desfaseHorarioMinutos(): Int
}

/**
 * Dónde queda anotada la impresora del negocio.
 *
 * Es configuración de **este teléfono**, no del tenant: la impresora está
 * enchufada acá. Por eso no viaja al server, igual que en el escritorio, donde
 * el nombre de la impresora vive en el `localStorage` de esa máquina.
 */
interface PreferenciasDeImpresora {
    fun leer(): ImpresoraElegida?
    fun guardar(impresora: ImpresoraElegida)
    fun olvidar()

    /** La última venta impresa o intentada, para el botón de reimprimir. */
    fun leerUltimaBoleta(): String?
    fun guardarUltimaBoleta(ordenId: String)
}
