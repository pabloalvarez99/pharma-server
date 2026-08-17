package cl.rutbusiness.app.ui.menu

import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Cómo se llega a "las ventas que faltan subir" desde cualquier pantalla.
 *
 * Esa pantalla es [cl.rutbusiness.app.ui.offline.PantallaDeCola], y hoy el
 * único gatillo que la muestra es `FranjaDeConexion.onVerCola` dentro de
 * `ContenedorDeDestinos` — y esa franja no se dibuja nada cuando está
 * conectado y no hay nada pendiente (`ContenedorDeDestinos.kt`). Con la
 * conexión andando bien, que es el caso común, no existe ningún camino hacia
 * el respaldo ni hacia la cola.
 *
 * Este local reusa el mismo estado `viendoCola` que ya maneja
 * `ContenedorDeDestinos` — no duplica nada de la lógica de cifrado ni de
 * subida, sólo ofrece una segunda puerta hacia la misma pantalla. Falta una
 * línea en `ContenedorDeDestinos` para proveerlo (agregarla no es parte de
 * este cambio: ese archivo está fuera del alcance de esta ola). Hasta
 * entonces el menú simplemente esconde esta entrada, como corresponde cuando
 * [LocalAbrirVentasPendientes] no está provisto.
 */
val LocalAbrirVentasPendientes = staticCompositionLocalOf<(() -> Unit)?> { null }

/** Abre "las ventas que faltan subir", o `null` si desde acá todavía no se puede. */
@Composable
fun abrirVentasPendientes(): (() -> Unit)? = LocalAbrirVentasPendientes.current
