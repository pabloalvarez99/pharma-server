package cl.rutbusiness.app.ui.nav

/**
 * Copy de la barra de abajo, según el pack (ADR-0022).
 *
 * Puro JVM: [BarraDeNavegacion] arma la pestaña, este archivo fija las tres
 * palabras. Mismo patrón que [copyTituloEscaner][cl.rutbusiness.app.ui.cobrar]
 * y el resto de la feria — el texto vive en un objeto aparte para que
 * [CopyNavegacionTest] lo pueda fijar sin levantar Compose ni Android.
 *
 * Feria habla del puesto, nunca del software: "Vender" y no "Cobrar", "Hoy" y
 * no "Tu día". "Hablar" reemplaza a "Agente" porque en el puesto no hay un
 * bot con nombre propio — hay alguien con quien conversar mientras se
 * atiende. Retail formal conserva las palabras de siempre.
 */

/**
 * Ir a hablarle al negocio. Feria: "Hablar" — la directiva del founder es que
 * la dueña no navega un menú, conversa; "Agente" es la palabra de quien arma
 * el software, no de quien vende.
 */
internal fun etiquetaAgente(feria: Boolean): String = if (feria) "Hablar" else "Agente"

/**
 * La pantalla que se toca doscientas veces al día. Feria: "Vender" — es lo
 * que la dueña vino a hacer al puesto, no un verbo de POS ("Cobrar" suena a
 * caja registradora).
 */
internal fun etiquetaCobrar(feria: Boolean): String = if (feria) "Vender" else "Cobrar"

/**
 * "¿Cuánto vendí hoy?" y, adentro, quién le debe. Feria: "Hoy" — la pregunta
 * de todos los días dicha con una palabra, más corta que "Tu día" y más fácil
 * de leer con el pulgar sosteniendo una bolsa.
 */
internal fun etiquetaResumen(feria: Boolean): String = if (feria) "Hoy" else "Tu día"
