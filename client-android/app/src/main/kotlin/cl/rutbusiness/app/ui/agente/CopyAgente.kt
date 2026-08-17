package cl.rutbusiness.app.ui.agente

/**
 * Las frases de ejemplo con las que se enseña a hablarle al agente.
 *
 * Separado de `HablarleAlAgente.kt` (que es sólo el cableado de navegación)
 * por el mismo criterio que separa cada pantalla de su `Copy*.kt`: acá vive
 * el texto, allá quién puede llegar. Puro JVM, sin Compose.
 *
 * **Ojo con la duplicación.** [AssistSugerencias][cl.rutbusiness.app.ui.assist.AssistSugerencias]
 * (`ui/assist`, otro dueño) trae sus propios chips de bienvenida con las
 * mismas dos frases escritas de nuevo, en vez de importar estas constantes.
 * No se unifica acá porque tocar ese archivo es de otra ola; en cambio,
 * [CopyAgenteTest] cruza ambas listas para que una frase que cambie de un
 * lado y no del otro no pase en silencio.
 */

/**
 * Cómo se le dice al agente que se vendió algo.
 *
 * Va en los vacíos como ejemplo y no como instrucción: la vara es el cuaderno, y
 * en el cuaderno nadie escribe "registrar una transacción" — escribe lo que
 * pasó. El ejemplo es una venta de feria (kilos, precio redondo) porque una
 * dueña copia el ejemplo que se parece a lo suyo.
 *
 * Está acá y no adentro de cada pantalla porque la frase la enseñan dos vacíos
 * distintos («Hoy» y «Quién me debe»): si se separan, cada pantalla le enseña
 * otra manera de hablar al mismo agente.
 *
 * Frase validada contra el parser real en `crates/assist/tests/feria_chips.rs`
 * (`precio_dicho`): si esto deja de parsear, ese test lo agarra antes que
 * cualquier persona en feria.
 */
const val ASI_SE_ANOTA_UNA_VENTA = "vendí 2 kg de tomates a 2000"

/**
 * Ídem para fiar: una venta fiada completa (qué, a cuánto, a quién).
 *
 * "Don Juan me debe 5000" suena natural pero el parser lo deja Incomplete
 * (pide el producto). Esta frase sí cierra el loop como Venta fiado. También
 * cubierta por `crates/assist/tests/feria_chips.rs`.
 */
const val ASI_SE_FIA = "anota 2 kg de tomates a 2000 fiado a Don Juan"

/**
 * Todas las cadenas de usuario de este archivo, para el gate de tono.
 *
 * Mismo criterio que `todoCopyGenteUsuario` en `CopyGente.kt`.
 */
internal fun todoCopyAgenteUsuario(): List<String> = listOf(
    ASI_SE_ANOTA_UNA_VENTA,
    ASI_SE_FIA,
)
