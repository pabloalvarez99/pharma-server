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

/**
 * Cómo se le dice al agente que se vendió algo.
 *
 * Va en los vacíos como ejemplo y no como instrucción: la vara es el cuaderno, y
 * en el cuaderno nadie escribe "registrar una transacción" — escribe lo que
 * pasó. El ejemplo es una venta de feria (kilos, precio redondo) porque una
 * dueña copia el ejemplo que se parece a lo suyo.
 *
 * Está acá y no adentro de cada pantalla porque la frase la enseñan dos vacíos
 * distintos: si se separan, cada pantalla le enseña otra manera de hablar al
 * mismo agente.
 */
const val ASI_SE_ANOTA_UNA_VENTA = "vendí 2 kg de tomates a 2000"

/** Ídem para fiar: quién y cuánto, en el orden en que se dice. */
const val ASI_SE_FIA = "Don Juan me debe 5000"
