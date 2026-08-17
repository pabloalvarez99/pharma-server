package cl.rutbusiness.app.ui.agente

import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Cómo se llega al agente desde una pantalla que no es la del agente.
 *
 * En feria el agente **es** la casa (ADR-0022): anotar una venta o un fiado se
 * hace hablándole, no llenando un formulario. Por eso los vacíos de "Hoy" y de
 * "Quién me debe" tienen que poder mandar ahí — es literalmente el siguiente
 * paso que enseñan.
 *
 * Mismo patrón y misma razón que
 * [cl.rutbusiness.app.ui.catalogo.LocalAbrirCatalogo]: quien sabe cambiar de
 * pestaña es el contenedor de la navegación, y bajarle un callback a mano a cada
 * pantalla obligaría a cambiarle la firma a todas para pasar siempre lo mismo.
 *
 * `null` significa **que desde acá todavía no se puede**: un contenedor que no
 * lo provee, o una prueba de pantalla suelta. La pantalla que lee `null` esconde
 * el botón en vez de ofrecer uno que no lleva a ninguna parte, y el texto que
 * enseña el paso queda igual — en feria la pestaña del agente está a un toque en
 * la barra de abajo.
 *
 * Quien lo provee es `ContenedorDeDestinos`, que es el único que sabe cambiar de
 * pestaña, con el mismo `remember` de identidad estable que usa para
 * `LocalAbrirCatalogo`:
 *
 * ```
 * CompositionLocalProvider(LocalIrAlAgente provides irAlAgente) { ... }
 * ```
 *
 * Mientras no lo provea, el botón simplemente no aparece y nada se rompe.
 */
val LocalIrAlAgente = staticCompositionLocalOf<(() -> Unit)?> { null }

/** Abre el agente, o `null` si desde acá todavía no se puede. */
@Composable
fun irAlAgente(): (() -> Unit)? = LocalIrAlAgente.current

// Las frases de ejemplo ("vendí 2 kg...", "anota... fiado a...") viven en
// CopyAgente.kt: este archivo es sólo la puerta (el cableado de navegación),
// no el copy. Mismo criterio que separa FiadoScreen.kt de CopyFiado.kt.
