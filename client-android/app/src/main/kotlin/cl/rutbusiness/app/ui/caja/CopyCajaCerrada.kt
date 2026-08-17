package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.app.ui.resumen.DiaUtc

/**
 * Copy propio de [PasoCajaCerrada] que no vive en [copyDeDiferencia] ni en
 * [labelListoCierre] (`Diferencia.kt`, de otro asiento de esta ola), aunque
 * las frases hablen del mismo momento. Archivo nuevo para no tocar
 * `Diferencia.kt` ni `CopyCaja.kt`.
 *
 * Dos cosas que la pantalla de cierre no decía y sí tiene que decir:
 *
 * - **Que este cierre puede no ser el de hoy.** El `ViewModel` de la caja vive
 *   mientras el proceso siga vivo (`viewModel(key = "caja:...")` en
 *   `CajaScreen.kt`), y en feria el teléfono rara vez se fuerza a cerrar: vuelve
 *   al bolsillo del delantal con la app abierta. Si nadie tocó "Listo por hoy"
 *   anoche, al otro día la pantalla sigue mostrando el cierre de ayer con la
 *   cara de un cierre de hoy. [cerroOtroDia] detecta el caso comparando el día
 *   UTC de `closed_at` contra el día UTC de ahora -misma convención que
 *   [DiaUtc]- y [tituloCierreAnterior] / [explicacionCierreAnterior] /
 *   [labelEmpezarHoy] cambian el aviso y el botón para que la dueña entienda
 *   que tiene que arrancar el día de hoy, no que ya lo cerró.
 * - **Que se pueda contar cómo quedó el día.** El resto de la app ya tiene esa
 *   costumbre: `ResumenScreen` manda cuánto se vendió con [RecadoParaGente]
 *   (`ui/gente`), y esta pantalla -el cierre, el momento más completo de
 *   información del día- no ofrecía nada parecido. [resumenDeCierreParaCompartir]
 *   arma la misma frase que ya lee la dueña en la tarjeta, lista para
 *   [cl.rutbusiness.app.ui.gente.mensajeHoy].
 */

/**
 * `true` cuando el cierre que se está mostrando no es el de hoy.
 *
 * Compara el día UTC de `closed_at` contra el día UTC de [ahoraEnMilis] -misma
 * convención que [DiaUtc]: no importa si coincide con la medianoche de Chile,
 * sólo que sea consistente con el día que ya usa el resto de la app-. Sin
 * `closed_at` (no debería pasar en una caja ya cerrada, pero el server no lo
 * garantiza) no se acusa nada: se asume que es de hoy antes que mostrar un
 * aviso que podría estar mal.
 */
internal fun cerroOtroDia(cerradaEnDelServidor: String?, ahoraEnMilis: Long): Boolean {
    val diaDelCierre = cerradaEnDelServidor?.trim().orEmpty().take(10)
    if (!DIA_ISO.matches(diaDelCierre)) return false

    val diaDeHoy = DiaUtc.rfc3339(ahoraEnMilis).take(10)
    return diaDelCierre != diaDeHoy
}

/**
 * `YYYY-MM-DD`, el prefijo de un `closed_at` RFC 3339.
 *
 * Se verifica la forma y no sólo el largo: cualquier texto de 10 caracteres
 * que no sea una fecha difiere del día de hoy, y sin este chequeo el aviso de
 * "este cierre es de otro día" aparecía por un dato roto del server.
 */
private val DIA_ISO = Regex("""\d{4}-\d{2}-\d{2}""")

/**
 * Título del aviso cuando el cierre mostrado no es el de hoy.
 *
 * Nunca "ayer" a secas como si fuera parte de la tarjeta de la diferencia:
 * es un aviso aparte, arriba de todo, porque cambia lo que significa el resto
 * de la pantalla.
 */
internal fun tituloCierreAnterior(feria: Boolean): String =
    if (feria) "Este cierre es de otro día" else "Esta caja se cerró otro día"

/**
 * Cuerpo del aviso: qué pasó y qué hacer. Dice "toca el botón" y no nombra el
 * botón, porque [labelEmpezarHoy] es la etiqueta y repetirla en dos lugares
 * las desincroniza el día que una cambie sin la otra.
 */
internal fun explicacionCierreAnterior(feria: Boolean): String =
    if (feria) {
        "Ya quedó guardado. Toca el botón para empezar el día de hoy."
    } else {
        "Ya quedó guardado. Toca el botón para abrir la caja de hoy."
    }

/**
 * Etiqueta del botón cuando el cierre mostrado no es el de hoy.
 *
 * Reemplaza a [labelListoCierre] sólo en este caso: "Listo por hoy" da a
 * entender que hoy ya se cerró, y hoy todavía no empezó. El botón sigue
 * llamando al mismo `onListo` de siempre -no cambia la navegación, sólo lo que
 * dice-.
 */
internal fun labelEmpezarHoy(feria: Boolean): String =
    if (feria) "Empezar el día de hoy" else "Abrir la caja de hoy"

/**
 * El resumen del cierre, listo para meter en el mensaje del chat.
 *
 * Junta [CopyDeDiferencia.titular] y [CopyDeDiferencia.explicacion] tal cual
 * los lee la dueña en la tarjeta. Sin [CopyDeDiferencia.calma]: esa frase
 * tranquiliza a quien está mirando la plata en el momento, no es un dato para
 * mandarle a otra persona.
 */
internal fun resumenDeCierreParaCompartir(copy: CopyDeDiferencia): String =
    "${copy.titular}. ${copy.explicacion}"

/**
 * Todas las cadenas de usuario de este archivo, para el gate de tono.
 *
 * Mismo criterio que `todoCopyGenteUsuario` en `CopyGente.kt`.
 */
internal fun todoCopyCajaCerradaUsuario(): List<String> = listOf(
    tituloCierreAnterior(feria = true),
    tituloCierreAnterior(feria = false),
    explicacionCierreAnterior(feria = true),
    explicacionCierreAnterior(feria = false),
    labelEmpezarHoy(feria = true),
    labelEmpezarHoy(feria = false),
)
