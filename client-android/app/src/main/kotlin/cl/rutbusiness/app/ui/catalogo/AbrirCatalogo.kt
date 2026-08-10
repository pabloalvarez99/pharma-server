package cl.rutbusiness.app.ui.catalogo

import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Cómo se abre "Lo que vendo" desde cualquier pantalla.
 *
 * **Por qué un `CompositionLocal` y no una pestaña.** La barra de abajo está
 * medida: tres etiquetas completas en 360dp con la letra al 200% ya es el
 * límite: está escrito en `BarraDeNavegacion` y lo prueba
 * `BarraDeNavegacionEscalaTest`, que además dice explícitamente que el catálogo
 * **no** es un destino. Cargar lo que se vende tampoco es un lugar donde uno se
 * queda: se entra, se cargan cinco cosas y se vuelve a cobrar.
 *
 * **Y por qué no un parámetro más.** `ContenedorDeDestinos` le pasa a las
 * pantallas un solo `Destino`, y su prueba (`NavegacionTest`) monta esa misma
 * firma con pantallas de mentira. Hacer bajar un callback a mano por Cobrar, por
 * el agente y por el resumen sería cambiarles la firma a las tres para pasar lo
 * mismo. Es el patrón que la app ya usa para lo que atraviesa pantallas
 * (`LocalOffline`, `LocalRubro`, `LocalCompartirTarjeta`).
 *
 * `null` significa **que todavía no se puede**: sin sesión no hay a qué servidor
 * escribirle. Una pantalla que lee `null` esconde su entrada en vez de ofrecer
 * un botón que no lleva a ninguna parte.
 */
val LocalAbrirCatalogo = staticCompositionLocalOf<(() -> Unit)?> { null }

/** Abre "Lo que vendo", o `null` si desde acá todavía no se puede. */
@Composable
fun abrirCatalogo(): (() -> Unit)? = LocalAbrirCatalogo.current
