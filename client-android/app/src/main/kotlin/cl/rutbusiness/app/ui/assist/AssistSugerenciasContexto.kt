package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.RubroPack

/**
 * Qué sugerir cuando la conversación YA arrancó (a diferencia de
 * [AssistSugerencias], que es solo el primer momento, vacío).
 *
 * El agente entiende más de treinta preguntas y una docena de acciones
 * (ver `crates/assist/src/intent.rs` y `actions.rs`), pero nada de eso sirve
 * si nadie sabe que existe. Acá se decide qué ofrecer después de que la
 * dueña acaba de hacer algo — "vendí", "fié" — en vez de repetir siempre los
 * mismos cinco chips fijos.
 *
 * Cada frase que devuelve esta clase está verificada contra el parser real:
 * ver el comentario de cada constante para el archivo/función exactos.
 */
internal object AssistSugerenciasContexto {

    /** [crates/assist/src/intent.rs] `ultimas_ventas_synonyms`: "¿qué vendí recién?". */
    const val QUE_VENDI_RECIEN = "¿Qué vendí recién?"

    /** [crates/assist/src/intent.rs] `ultimas_ventas_synonyms`: "muéstrame la última venta". */
    const val MUESTRAME_LA_ULTIMA_VENTA = "Muéstrame la última venta"

    /** [crates/assist/src/intent.rs] `por_cobrar_synonyms`: "quien me debe". */
    const val QUIEN_ME_DEBE = "¿Quién me debe plata?"

    /**
     * [crates/assist/src/intent.rs] `costo_producto_captures_term`: cue
     * "cuánto me cuesta X". La FRASE de escritura ("el tomate me cuesta 800")
     * la agregó la ola 29 del lado del parser de acciones — es la que
     * desbloquea el margen y hoy nadie la adivina.
     */
    const val EL_TOMATE_ME_CUESTA_800 = "El tomate me cuesta 800"

    /** Igual a [AssistSugerencias.feria]\[0\]: ya validada por [AssistSugerenciasFeriaTest]. */
    private const val VENDI_KG_DE_TOMATES = "Vendí 2 kg de tomates a 2000"

    /** Igual a [AssistSugerencias.feria]\[1\]: ya validada por [AssistSugerenciasFeriaTest]. */
    private const val ANOTA_FIADO_A_DON_JUAN = "Anota 2 kg de tomates a 2000 fiado a Don Juan"

    /** Igual a [AssistSugerencias.default]\[0\]. */
    private const val CUANTO_VENDI_HOY = "¿Cuánto vendí hoy?"

    /** Igual a [AssistSugerencias.default]\[3\]. */
    private const val REGISTRA_UN_GASTO = "Registra un gasto de 5000 en arriendo"

    private fun esFeria(pack: RubroPack): Boolean =
        pack.features.agentHome || pack.rubro == "feria"

    /**
     * Qué ofrecer justo después de un turno.
     *
     * @param pack rubro activo (decide si se enseña en modo puesto).
     * @param recienEmpezando true cuando todavía no hay ningún mensaje: acá se
     *   enseña a vender y a anotar, porque nadie ha visto el agente hacer nada
     *   todavía.
     * @param ultimaAccionConfirmada la etiqueta (`Action::label()`) de la
     *   última propuesta que la dueña confirmó con éxito, o null si no ha
     *   confirmado ninguna en esta conversación. "vender" ofrece revisar lo
     *   que se vendió; "fiar_venta" ofrece revisar quién debe.
     * @param yaUsadas preguntas que ya se mandaron en esta conversación: no
     *   tiene sentido repetirlas arriba de todo.
     */
    fun para(
        pack: RubroPack,
        recienEmpezando: Boolean,
        ultimaAccionConfirmada: String?,
        yaUsadas: Set<String>,
    ): List<String> {
        val feria = esFeria(pack)
        val orden = linkedSetOf<String>()

        when (ultimaAccionConfirmada) {
            "vender" -> {
                orden += QUE_VENDI_RECIEN
                orden += MUESTRAME_LA_ULTIMA_VENTA
            }
            "fiar_venta" -> orden += QUIEN_ME_DEBE
        }

        if (recienEmpezando) {
            if (feria) {
                orden += VENDI_KG_DE_TOMATES
                orden += ANOTA_FIADO_A_DON_JUAN
            } else {
                orden += CUANTO_VENDI_HOY
                orden += REGISTRA_UN_GASTO
            }
        }

        if (feria) {
            orden += EL_TOMATE_ME_CUESTA_800
        }

        orden += QUIEN_ME_DEBE
        orden += CUANTO_VENDI_HOY

        return orden.filterNot { it in yaUsadas }.take(5)
    }

    /**
     * Sugerencias para el momento en que el agente NO entendió (paso 4 del
     * encargo). Nunca vacío: si no hay nada contextual mejor, cae en la
     * bienvenida de siempre.
     */
    fun cuandoNoEntiende(
        pack: RubroPack,
        ultimaAccionConfirmada: String?,
        yaUsadas: Set<String>,
    ): List<String> {
        val contextual = para(
            pack = pack,
            recienEmpezando = false,
            ultimaAccionConfirmada = ultimaAccionConfirmada,
            yaUsadas = yaUsadas,
        )
        return contextual.ifEmpty { AssistSugerencias.para(pack) }
    }
}
