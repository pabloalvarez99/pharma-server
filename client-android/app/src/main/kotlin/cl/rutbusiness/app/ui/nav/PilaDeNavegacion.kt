package cl.rutbusiness.app.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue

/**
 * Todo lo que se abre desde "Tu día": el resumen y todo lo que cuelga de él,
 * incluidas las pantallas que antes vivían en el `SeccionDelMenu` propio de
 * `MenuDelPuestoScreen.kt`.
 *
 * Antes había dos `enum` anidados con un `when` cada uno (`RutinaDelDia` en
 * este archivo, `SeccionDelMenu` adentro de `MenuDelPuestoScreen.kt`), sin
 * pila. Eso hacía que [Caja] fuera alcanzable desde dos padres distintos con
 * semánticas de "volver" distintas según por dónde se entraba. Con una sola
 * lista de [Pantalla] hay un solo dueño de la navegación: da lo mismo si
 * [Caja] se empujó desde [Resumen] o desde [Menu], volver siempre saca la
 * última que se empujó.
 *
 * Dónde entra cada pantalla nueva de las olas 29 a 33 (una línea por
 * pantalla, para no tener que releer el encargo cada vez):
 * - Historial de ventas (ola 29): hecho — entrada en el grupo "Lo que usas
 *   seguido" de `ListaDelMenu`, empuja `Pantalla.HistorialVentas`.
 * - Devoluciones (ola 29): entrada nueva en el grupo "todos los días" de
 *   `ListaDelMenu` (pasa en el puesto, no una vez al mes), empuja
 *   `Pantalla.Devoluciones`.
 * - Fiado por persona (ola 30): no es entrada del menú — "quién me debe" ya
 *   tiene camino desde Hoy (ver KDoc de `ListaDelMenu`). Se empuja desde
 *   adentro de [Fiado] con el id de la persona, ej. `Pantalla.FiadoPersona`.
 * - Márgenes (ola 31): entrada nueva en el grupo "una vez al mes" de
 *   `ListaDelMenu`, empuja `Pantalla.Margenes`.
 * - Gastos (ola 30, adelantada de la 32 — sin gastos la ganancia es
 *   ficción): hecho — entrada nueva en el grupo "todos los días" de
 *   `ListaDelMenu` (se anota cada vez que sale plata), empuja
 *   `Pantalla.Gastos`.
 * - Respaldo (ola 33): entrada nueva en el grupo "una vez al mes" de
 *   `ListaDelMenu` — nunca se dice "respaldo" en la etiqueta que ve la
 *   dueña (`CopyMenuTest` lo prohíbe en cualquier rubro), empuja
 *   `Pantalla.Respaldo`.
 *
 * Agregar una pantalla nueva a la pila es: un `object` acá, una línea en
 * [todas], y una rama más en el `when` de `TuDiaRoute` o de `ListaDelMenu` —
 * no cirugía sobre dos `enum` a la vez.
 */
internal sealed class Pantalla(internal val clave: String) {
    internal object Resumen : Pantalla("resumen")
    internal object Caja : Pantalla("caja")
    internal object Fiado : Pantalla("fiado")
    internal object Menu : Pantalla("menu")
    internal object Impresora : Pantalla("impresora")
    internal object Rubro : Pantalla("rubro")
    internal object HistorialVentas : Pantalla("historial-ventas")
    internal object Gastos : Pantalla("gastos")

    internal companion object {
        internal val todas: List<Pantalla>
            get() = listOf(Resumen, Caja, Fiado, Menu, Impresora, Rubro, HistorialVentas, Gastos)

        internal fun porClave(clave: String): Pantalla =
            todas.first { it.clave == clave }
    }
}

/**
 * Una pila de navegación hecha a mano: `push`/`pop` sobre una `List<Pantalla>`.
 *
 * No es `navigation-compose` — el encargo lo prohíbe explícitamente para no
 * sumar una dependencia de Gradle nueva — pero resuelve lo mismo que un
 * `NavHost` resolvería acá: cada pantalla tiene un único camino de vuelta,
 * que es el que la trajo, sin que la pantalla necesite saber quién la abrió.
 *
 * Sin `ViewModelStore` por entrada a propósito: los `ViewModel` de Caja,
 * Fiado, etc. cuelgan del `ViewModelStore` del `Activity` (como ya hacía
 * [ContenedorDeDestinos]), así que salir de una pantalla y volver no reinicia
 * su estado.
 */
@Stable
internal class PilaDeNavegacion(
    internal val pantallas: List<Pantalla>,
    private val onCambiar: (List<Pantalla>) -> Unit,
) {
    /** La pantalla que se está mostrando: el tope de la pila. */
    internal val actual: Pantalla get() = pantallas.last()

    /** `true` cuando no hay a dónde volver — la pila sólo tiene la raíz. */
    internal val enRaiz: Boolean get() = pantallas.size <= 1

    /** Empuja una pantalla nueva encima de la actual. */
    internal fun ir(pantalla: Pantalla) {
        onCambiar(pantallas + pantalla)
    }

    /**
     * Saca la pantalla del tope y vuelve a la anterior. No hace nada si ya
     * está en la raíz — quien llama decide qué pasa entonces (en
     * [TuDiaRoute] eso ya lo maneja el `BackHandler` de más arriba, igual
     * que hace [ContenedorDeDestinos] con sus tres pestañas).
     */
    internal fun volver() {
        if (pantallas.size > 1) onCambiar(pantallas.dropLast(1))
    }
}

/**
 * Recuerda una [PilaDeNavegacion] que sobrevive a rotación y a muerte de
 * proceso.
 *
 * Guarda nombres (`Pantalla.clave`, un `String`), no los `object` en sí: son
 * `internal object` de un `sealed class`, no `Parcelable`, así que el
 * `Saver` explícito convierte a `List<String>` para guardar y reconstruye
 * con [Pantalla.porClave] al restaurar.
 */
@Composable
internal fun rememberPilaDeNavegacion(raiz: Pantalla): PilaDeNavegacion {
    var pantallas: List<Pantalla> by rememberSaveable(
        stateSaver = Saver(
            save = { estado: List<Pantalla> -> estado.map { it.clave } },
            restore = { claves: List<String> -> claves.map { Pantalla.porClave(it) } },
        ),
    ) { mutableStateOf(listOf(raiz)) }

    return PilaDeNavegacion(pantallas) { pantallas = it }
}
