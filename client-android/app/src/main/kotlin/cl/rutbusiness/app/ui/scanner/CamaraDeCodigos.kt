package cl.rutbusiness.app.ui.scanner

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier

/**
 * Los formatos que el lector habilita, y ninguno más.
 *
 * Cada formato extra es otro decodificador corriendo sobre cada frame, y el
 * aparato objetivo tiene 1-2 GB y una CPU de hace ocho años: habilitar "todos"
 * se paga en frames perdidos, o sea en segundos de la cajera con el cliente
 * esperando.
 *
 * - [Ean13] y [Ean8] son lo que traen impreso los productos del retail chileno.
 * - [Code128] es lo que imprimen las balanzas de fiambrería y las etiquetas
 *   internas; sale casi gratis porque comparte el detector 1D con los EAN.
 *
 * Los 2D (QR, Data Matrix, PDF417) quedan fuera **a propósito**: usan otro
 * detector completo y en un punto de venta de abarrotes no aparecen.
 */
enum class FormatoDeCodigo { Ean13, Ean8, Code128 }

/** Lo que se lee en un mostrador chileno. */
val FORMATOS_DE_RETAIL: Set<FormatoDeCodigo> =
    setOf(FormatoDeCodigo.Ean13, FormatoDeCodigo.Ean8, FormatoDeCodigo.Code128)

/**
 * En qué quedó el permiso de cámara.
 *
 * [NegadoParaSiempre] existe separado de [Negado] porque son dos pantallas
 * distintas: en el primero Android ya no va a volver a preguntar y el único
 * camino es Ajustes; en el segundo alcanza con volver a pedirlo.
 */
enum class EstadoDelPermiso { SinPedir, Concedido, Negado, NegadoParaSiempre }

@Stable
interface PermisoDeCamara {
    val estado: EstadoDelPermiso

    /** Dispara el diálogo del sistema. Sólo después de haber explicado para qué. */
    fun pedir()

    /** Abre la ficha de la app en Ajustes, en la sección de permisos. */
    fun abrirAjustes()
}

/**
 * La cámara, vista desde la UI.
 *
 * Existe para que `ui/` no importe `android.*` (regla del `README.md` de
 * `client-android`): acá está el contrato, y la implementación con CameraX y
 * ML Kit vive en `cl.rutbusiness.app.camara`, del otro lado de
 * [LocalCamaraDeCodigos].
 *
 * El [Visor] mantiene la cámara viva **mientras está en composición y no más**.
 * Sacarlo de la composición la suelta: es la única forma de garantizar que al
 * salir de la pantalla no queda una sesión de cámara comiéndose la RAM y la
 * batería.
 */
interface CamaraDeCodigos {

    @Composable
    fun recordarPermiso(): PermisoDeCamara

    /**
     * Pinta lo que ve la cámara y avisa cada código que reconoce.
     *
     * [onCodigo] se llama en el hilo principal y puede llegar muchas veces con
     * el mismo código: filtrar repeticiones es de quien lo recibe (ver
     * [AntiRebote]), porque el criterio de "es el mismo producto otra vez" es
     * de negocio, no de cámara.
     */
    @Composable
    fun Visor(
        modifier: Modifier,
        formatos: Set<FormatoDeCodigo>,
        onCodigo: (String) -> Unit,
    )
}

/**
 * La cámara del aparato, o `null` si este build no tiene ninguna.
 *
 * `null` no es un caso raro que haya que blindar: es lo que ven los tests de
 * Robolectric y lo que vería una tablet sin cámara. Con `null`, la pantalla de
 * cobrar simplemente no ofrece el botón de escanear y se cobra tipeando.
 */
val LocalCamaraDeCodigos = staticCompositionLocalOf<CamaraDeCodigos?> { null }
