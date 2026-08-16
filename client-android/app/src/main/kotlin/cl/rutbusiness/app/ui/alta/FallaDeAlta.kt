package cl.rutbusiness.app.ui.alta

import cl.rutbusiness.app.ui.entrada.FallaDeConexion
import cl.rutbusiness.core.error.AppError

/** Lo mínimo que el server acepta (`MIN_PASSWORD_LEN` en `crates/api/src/setup.rs`). */
const val LARGO_MINIMO_DE_CLAVE: Int = 8

/**
 * Por qué no se pudo crear el negocio, en las categorías que se arreglan
 * distinto.
 *
 * Mismo criterio que [cl.rutbusiness.app.ui.entrada.FallaDeConexion] y por la
 * misma razón: un solo "no se pudo crear la cuenta" deja a la dueña parada
 * frente a cuatro problemas que no tienen nada que ver entre sí.
 *
 * | Qué pasó | Dónde se arregla |
 * |---|---|
 * | [SinRed] | en el teléfono: prender el wifi o los datos |
 * | [ServicioNoResponde] | esperar y volver a intentar |
 * | [CorreoYaTieneNegocio] | en la otra puerta: entrar en vez de crear |
 * | [ClaveDebil] | en esta misma pantalla: alargar la clave |
 *
 * Las otras dos ([DatosRechazados], [CreadoPeroNoEntro]) no son categorías de
 * falla sino dos verdades que hay que poder decir: que el server rechazó algo
 * que nosotros no supimos anticipar, y que el negocio **sí** se creó aunque la
 * sesión no se haya abierto.
 *
 * [queHacer] es siempre una instrucción, nunca un diagnóstico: nada de códigos,
 * nada de "servidor no disponible", nada de nombres de endpoint.
 */
sealed class FallaDeAlta(
    val titulo: String,
    val queHacer: String,
    /** Detalle técnico. No se muestra nunca; sirve para diagnosticar. */
    val tecnico: String? = null,
) {
    /**
     * El teléfono no está conectado a ninguna red.
     *
     * Se pregunta **antes** de mandar nada, igual que en la entrada: sin red
     * cualquier intento termina en un timeout largo que además no distingue
     * esto de un servicio caído.
     */
    class SinRed(esFeria: Boolean = false) : FallaDeAlta(
        titulo = "Tu teléfono no está conectado",
        queHacer = "No tiene wifi ni datos prendidos, así que no puede llegar a ninguna parte. " +
            "Prende el wifi del local, o prende los datos del teléfono, y vuelve a tocar " +
            if (esFeria) "«Crear mi puesto»." else "«Crear mi negocio».",
    )

    /**
     * Se intentó y no hubo respuesta.
     *
     * **No dice "no se creó nada".** Un intento que se corta sin respuesta no
     * prueba que el server no haya alcanzado a guardar: el corte puede haber
     * sido en la vuelta. Prometer que no pasó nada y que después resulte que sí
     * es peor que no prometer nada, así que se dice qué va a ver si se creó —
     * es el mismo cuidado que con la cola de ventas y con `/assist/act`.
     */
    class ServicioNoResponde(
        donde: String,
        tecnico: String? = null,
        nube: Boolean = false,
    ) : FallaDeAlta(
        titulo = "El servicio no está contestando",
        queHacer = if (nube) {
            "Intentamos crear tu puesto y no hubo respuesta. Revisá que el teléfono tenga " +
                "internet, espera un momento y vuelve a intentar. Si al reintentar te dice que " +
                "ese nombre o correo ya están, es que sí alcanzó a crearse: entra con tu correo " +
                "y tu clave."
        } else {
            "Intentamos crear tu negocio en ${comoSeLee(donde)} y no hubo respuesta. " +
                "Revisa que tu teléfono tenga wifi o datos, espera un momento y vuelve a intentar. " +
                "Si al reintentar te dice que ahí ya hay un negocio, es que sí alcanzó a crearse: " +
                "entra con tu correo y tu clave."
        },
        tecnico = tecnico,
    )

    /**
     * Ese correo no puede crear un negocio en ese lugar, porque el lugar ya
     * tiene uno.
     *
     * El título nombra el correo porque es lo que la dueña acaba de escribir y
     * lo primero que va a revisar; el cuerpo dice exactamente lo que pasó y no
     * más. El server contesta `SETUP_ALREADY_DONE`, que significa "esta
     * instalación ya tiene una cuenta". **No** significa "ese correo está
     * tomado por otra persona", y decírselo así la mandaría a inventarse un
     * correo nuevo, que no arregla nada. Lo que arregla es entrar en vez de
     * crear, o crear en otro computador.
     */
    class CorreoYaTieneNegocio(
        email: String,
        donde: String,
        tecnico: String? = null,
        nube: Boolean = false,
    ) : FallaDeAlta(
        titulo = if (nube) {
            "Ese correo no puede crear un puesto acá"
        } else {
            "Ese correo no puede crear un negocio acá"
        },
        queHacer = if (nube) {
            // El host de la nube es compilado (emulador, dominio…): no se lo
            // mostramos al feriante ni lo llamamos "computador".
            "Ahí ya hay un puesto creado, así que $email no puede crear otro encima. " +
                "Si ese puesto es tuyo, vuelve atrás y toca «Entrar»: entras con ese " +
                "mismo correo y tu clave."
        } else {
            "En ${comoSeLee(donde)} ya hay un negocio creado, así que $email no puede " +
                "crear otro encima. Si ese negocio es tuyo, vuelve atrás y toca «Entrar a mi " +
                "negocio»: entras con ese mismo correo y tu clave."
        },
        tecnico = tecnico,
    )

    /**
     * La clave no llega al mínimo.
     *
     * Se detecta en el teléfono antes de mandar nada —es una cuenta de letras—
     * y además se mapea la respuesta del server, por si algún día el mínimo
     * cambia allá y no acá.
     */
    class ClaveDebil(tecnico: String? = null) : FallaDeAlta(
        titulo = "Esa clave es muy corta",
        queHacer = "Tiene que tener al menos $LARGO_MINIMO_DE_CLAVE letras o números. No hace " +
            "falta que sea complicada: sirve algo que puedas recordar, como el nombre del local " +
            "junto con el año.",
        tecnico = tecnico,
    )

    /**
     * Algo de lo escrito no sirvió.
     *
     * Título honesto, sin culpar a un «sistema» ni a un computador: la dueña
     * tiene que revisar lo que puso. El cuerpo es el mensaje del server tal
     * cual —ya viene en español y escrito para ella (`ApiError::invalid` en
     * `crates/api/src/setup.rs`)— o, si viniera vacío, la instrucción corta.
     * Traducirlo de nuevo acá sería inventar una segunda versión que se
     * desincroniza con la primera.
     */
    class DatosRechazados(mensajeDelServidor: String, tecnico: String? = null) : FallaDeAlta(
        titulo = "Esos datos no sirvieron",
        queHacer = mensajeDelServidor.ifBlank { "Revisá lo que escribiste." },
        tecnico = tecnico,
    )

    /**
     * El negocio quedó creado y la sesión no se abrió.
     *
     * Es el peor mensaje posible de escribir mal: si acá dijéramos "no se pudo
     * crear tu negocio", la dueña lo intentaría de nuevo, chocaría con "ahí ya
     * hay un negocio" y creería que la app está rota. Se dice lo que pasó y se
     * la manda a la puerta que sí va a funcionar, con los datos ya listos.
     *
     * [esPuesto] = feria o nube: se habla de «puesto», no de «negocio».
     */
    class CreadoPeroNoEntro(
        nombreCorto: String,
        tecnico: String? = null,
        esPuesto: Boolean = false,
    ) : FallaDeAlta(
        titulo = if (esPuesto) {
            "Tu puesto quedó creado, pero no pudimos entrar"
        } else {
            "Tu negocio quedó creado, pero no pudimos entrar"
        },
        queHacer = if (esPuesto) {
            "El puesto existe y tus datos quedaron guardados. Vuelve atrás, toca " +
                "«Entrar» y entra con tu correo y tu clave. El nombre corto de tu " +
                "puesto es: $nombreCorto"
        } else {
            "El negocio existe y tus datos quedaron guardados. Vuelve atrás, toca " +
                "«Entrar a mi negocio» y entra con tu correo y tu clave. El nombre corto de tu " +
                "negocio es: $nombreCorto"
        },
        tecnico = tecnico,
    )
}

/**
 * La dirección como la escribió la dueña, sin la maquinaria que le pusimos.
 *
 * Mismo criterio -y misma razón- que en la pantalla de entrada: `http://` lo
 * agrega el normalizador, no ella, y un mensaje de error no es el lugar para
 * enseñar qué es un esquema de URL.
 */
internal fun comoSeLee(direccion: String): String =
    direccion.substringAfter("://", direccion)

/**
 * Lo que se revisa en el teléfono antes de gastar un viaje a la red.
 *
 * Devuelve `null` cuando lo escrito da para intentar. Sólo mira la clave: el
 * nombre y el correo los revisa el paso donde se escriben, con el campo en rojo
 * al lado del error, que es donde se corrigen. La clave necesita además este
 * chequeo porque su regla —el largo— la fija el server y hay que poder decirla
 * con el mismo número.
 */
internal fun revisarLaClave(clave: String): FallaDeAlta? =
    if (clave.length < LARGO_MINIMO_DE_CLAVE) FallaDeAlta.ClaveDebil() else null

/**
 * Qué significa que el alta falle **después** de que el sondeo salió bien.
 *
 * Haber llegado al servicio hace un segundo cambia la lectura de todo lo que
 * venga: un rechazo ya no puede ser "revisa el wifi". Por eso este mapeo es más
 * filoso que el de [cl.rutbusiness.app.ui.common.aCopy].
 */
internal fun fallaDeAlta(
    error: AppError,
    donde: String,
    email: String,
    nube: Boolean = false,
): FallaDeAlta = when {
    // 409 de la nube compartida: el nombre corto o el correo ya son de otro
    // puesto. Traducir eso a "este computador está ocupado" es el recuadro
    // rojo que vio el feriante.
    error is AppError.ErrorDelServidor &&
        error.status == 409 &&
        (error.code == "SLUG_TOMADO" || error.code == "CORREO_TOMADO") ->
        FallaDeAlta.DatosRechazados(error.userMessage, error.technical)

    // 409: la instalación ya tiene cuenta (`SETUP_ALREADY_DONE`).
    error is AppError.ErrorDelServidor && error.status == 409 ->
        FallaDeAlta.CorreoYaTieneNegocio(email, donde, error.technical, nube)

    // 400 hablando de la clave: el mínimo del server se movió respecto del
    // nuestro. Se cree el del server y se dice como falla de clave, no como un
    // rechazo genérico.
    error is AppError.ErrorDelServidor && error.status == 400 && hablaDeLaClave(error) ->
        FallaDeAlta.ClaveDebil(error.technical)

    error is AppError.ErrorDelServidor && error.status == 400 ->
        FallaDeAlta.DatosRechazados(error.userMessage, error.technical)

    // Todo lo demás -sin respuesta, 5xx, 503, lo inesperado- es la misma cosa
    // para la dueña: el servicio no está en condiciones ahora.
    else -> FallaDeAlta.ServicioNoResponde(donde, error.technical, nube)
}

/**
 * Traduce una falla de la entrada a una del alta.
 *
 * Las categorías de la entrada son cuatro porque ahí hay credenciales que
 * pueden no coincidir; acá todavía no hay ninguna, así que las tres que quedan
 * se reparten en dos: la del teléfono y la del servicio. Van juntas porque para
 * la dueña "no contesta nadie" y "contesta algo que no sirve" se arreglan igual
 * cuando lo que estaba haciendo era crear su negocio: esperar y reintentar.
 */
internal fun deLaConexion(
    problema: FallaDeConexion,
    donde: String,
    nube: Boolean = false,
    esFeria: Boolean = false,
): FallaDeAlta =
    when (problema) {
        is FallaDeConexion.SinRed -> FallaDeAlta.SinRed(esFeria)
        else -> FallaDeAlta.ServicioNoResponde(donde, problema.tecnico, nube)
    }

/**
 * Si el rechazo del server es por la clave.
 *
 * Se mira el mensaje y no un código, porque `/setup` contesta `INVALID_INPUT`
 * para las cuatro validaciones que hace y el texto es lo único que las
 * distingue. Es frágil por naturaleza, así que **la rama frágil es la que
 * mejora el mensaje**, no la que decide si falla: si el texto cambia, se cae a
 * [FallaDeAlta.DatosRechazados], que muestra el mensaje del server igual.
 */
private fun hablaDeLaClave(error: AppError.ErrorDelServidor): Boolean =
    error.userMessage.contains("contraseña", ignoreCase = true) ||
        error.userMessage.contains("clave", ignoreCase = true)

/**
 * El slug que el server propuso en un 409 `SLUG_TOMADO`.
 *
 * No se parsea el envelope (`details.suggested_slug`): [AppError.ErrorDelServidor]
 * no lo trae. El mensaje de alta ya lo nombra («…Probá con huevos-de-marta-2.»);
 * se toma el token que sigue a «Probá con » / «Proba con » y se le saca el
 * punto final si lo trae.
 */
internal fun slugSugeridoEn(mensaje: String): String? {
    val marcas = listOf("Probá con ", "Proba con ")
    for (marca in marcas) {
        val i = mensaje.indexOf(marca)
        if (i < 0) continue
        val token = mensaje
            .substring(i + marca.length)
            .trimStart()
            .takeWhile { !it.isWhitespace() }
            .trimEnd('.')
        if (token.isNotEmpty()) return token
    }
    return null
}
