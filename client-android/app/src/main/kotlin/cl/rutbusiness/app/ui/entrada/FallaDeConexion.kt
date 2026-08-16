package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Por qué no se pudo entrar, en las cuatro categorías que se arreglan distinto.
 *
 * Un solo "error de conexión" no le sirve a nadie. Cuando la app no entra, la
 * dueña está parada frente a cuatro problemas que no tienen nada que ver entre
 * sí y que se arreglan en cuatro lugares distintos:
 *
 * | Qué pasó | Dónde se arregla |
 * |---|---|
 * | [SinRed] | en el teléfono: prender el wifi o los datos |
 * | [NadieContesta] | en el local: prender el computador, o corregir la dirección |
 * | [ContestaPeroNoEsElSistema] | con quien instaló: el número del final está mal |
 * | [DatosQueNoCoinciden] | en esta misma pantalla: el correo o la clave |
 *
 * Mandarla a revisar el wifi cuando lo que estaba mal era la clave es peor que
 * no decirle nada: pierde media hora en el lugar equivocado y termina creyendo
 * que la app está rota.
 *
 * Cada caso trae [titulo] y [queHacer], y [queHacer] siempre es una instrucción
 * en imperativo, nunca un diagnóstico. Nada de códigos, nada de nombres de
 * proceso, nada de "endpoint" ni "servidor no disponible".
 */
sealed class FallaDeConexion(
    val titulo: String,
    val queHacer: String,
    /** Detalle técnico. No se muestra nunca; sirve para diagnosticar. */
    val tecnico: String? = null,
) {
    /**
     * El teléfono no está conectado a ninguna red.
     *
     * Se pregunta **antes** de intentar nada: sin red, cualquier intento
     * termina en un timeout de diez segundos que además no distingue esto de un
     * computador apagado. Preguntarlo primero convierte diez segundos de espera
     * y un mensaje equivocado en una respuesta instantánea y correcta.
     */
    class SinRed(nube: Boolean = false) : FallaDeConexion(
        titulo = if (nube) "No hay internet" else "Tu teléfono no está conectado",
        queHacer = if (nube) {
            "El teléfono no tiene wifi ni datos. Prendelos y reintentá."
        } else {
            "No tiene wifi ni datos prendidos, así que no puede llegar a ninguna parte. " +
                "Prende el wifi del local, o prende los datos del teléfono, y vuelve a intentar."
        },
    )

    /**
     * Se intentó y nadie respondió: dirección mal escrita, computador apagado,
     * o el teléfono en otra red que la del computador.
     *
     * Los tres se ven exactamente igual desde acá y los tres se revisan en el
     * mismo lugar —el local—, así que van juntos y en orden de probabilidad.
     */
    class NadieContesta(
        direccion: String,
        tecnico: String? = null,
        nube: Boolean = false,
    ) : FallaDeConexion(
        titulo = if (nube) "RutAgent no responde" else "En esa dirección no contesta nadie",
        queHacer = if (nube) {
            "No pudimos llegar. Revisá que el teléfono tenga internet y reintentá."
        } else {
            "Escribiste ${comoSeLee(direccion)} y no hubo respuesta. Revisa tres cosas, " +
                "en este orden: que el computador del negocio esté prendido, que este teléfono esté " +
                "en el mismo wifi que él, y que la dirección esté bien escrita."
        },
        tecnico = tecnico,
    )

    /**
     * Contestó algo, pero no el sistema del negocio.
     *
     * Es el caso del número del final equivocado: en la misma máquina puede
     * haber otra cosa escuchando, y esa otra cosa contesta. También cae acá el
     * sistema prendido pero todavía no listo. Los dos se resuelven con la misma
     * persona y con el mismo dato, así que comparten mensaje.
     */
    class ContestaPeroNoEsElSistema(
        direccion: String,
        tecnico: String? = null,
        nube: Boolean = false,
    ) : FallaDeConexion(
        titulo = if (nube) {
            "RutAgent contestó algo raro"
        } else {
            "Ahí contesta algo, pero no se puede trabajar"
        },
        queHacer = if (nube) {
            "Reintentá en un momento. Si sigue igual, no es algo que se arregle desde el teléfono."
        } else {
            "En ${comoSeLee(direccion)} hay un computador prendido, pero lo que contesta " +
                "no es el sistema de tu negocio andando. Casi siempre es el número del final, el que " +
                "va después de los dos puntos, que quedó mal. Si estás segura de la dirección, " +
                "avísale a quien instaló el sistema: esto no se arregla desde el teléfono."
        },
        tecnico = tecnico,
    )

    /**
     * Llegamos y nos dijo que no: correo, clave o nombre corto no calzan.
     *
     * El server contesta lo mismo para los tres campos —nombre corto, correo,
     * clave— así que el mensaje los nombra a los tres. Adivinar cuál de ellos
     * está mal sería inventar precisión que el server no nos dio.
     */
    class DatosQueNoCoinciden(tecnico: String? = null) : FallaDeConexion(
        titulo = "Ese correo o esa clave no calzan",
        queHacer = "La conexión anda: lo que no calza es lo que escribiste. " +
            "Revisá el nombre corto, el correo y la clave. " +
            "La clave distingue mayúsculas de minúsculas.",
        tecnico = tecnico,
    )

    /** Correo en más de un puesto: hay que escribir el nombre corto. */
    class FaltaNombreCorto(tecnico: String? = null) : FallaDeConexion(
        titulo = "Falta el nombre corto",
        queHacer = "Ese correo está en más de un puesto. Escribí el nombre corto del que querés.",
        tecnico = tecnico,
    )
}

/**
 * La dirección como la escribió la dueña, sin la parte que le pusimos nosotros.
 *
 * `ServerUrl.normalizar` agrega `http://` cuando falta, porque el cliente HTTP
 * lo necesita. Devolvérselo en un mensaje es mostrarle maquinaria: ella escribió
 * `192.168.1.10:8080` y eso es lo que tiene que reconocer cuando le decimos que
 * ahí no contesta nadie. Un mensaje de error no es el lugar para enseñar qué es
 * un esquema de URL.
 */
internal fun comoSeLee(direccion: String): String =
    direccion.substringAfter("://", direccion)

/**
 * Los dos chequeos que se hacen **antes** de mandar la contraseña. `null` si
 * hay a dónde mandarla.
 *
 * Es una función suelta y no un método del `ViewModel` por lo mismo que
 * `copyDeDiferencia` en la caja: es la lógica que decide qué le vamos a decir a
 * la dueña cuando algo falla, o sea justo lo que hay que poder probar sin
 * levantar un servidor ni desenchufar un router.
 *
 * **El orden importa y no es cosmético.** Sin red no se sondea: no tiene
 * sentido esperar el timeout de un viaje que no puede salir, y además cualquier
 * cosa que mandemos con el teléfono desconectado es una espera pura.
 *
 * @param hayRed `null` cuando nadie proveyó el servicio de plataforma. Se salta
 *   el chequeo en vez de inventar una respuesta: contestar "sin red" porque el
 *   doble no existe dejaría la app sin poder entrar.
 */
internal suspend fun diagnosticarLaEntrada(
    direccion: String,
    hayRed: (() -> Boolean)?,
    sondear: suspend (String) -> Sondeo,
    nube: Boolean = false,
): FallaDeConexion? {
    if (hayRed?.invoke() == false) return FallaDeConexion.SinRed(nube)

    return when (val sondeo = sondear(direccion)) {
        Sondeo.EsElSistema -> null
        is Sondeo.NadieContesta -> FallaDeConexion.NadieContesta(direccion, sondeo.tecnico, nube)
        is Sondeo.ContestaOtraCosa ->
            FallaDeConexion.ContestaPeroNoEsElSistema(direccion, sondeo.tecnico, nube)
    }
}

/**
 * Qué significa que el login falle **después** de que el sondeo salió bien.
 *
 * Haber llegado al sistema hace un segundo cambia la lectura de todo lo que
 * venga: un rechazo ya no puede ser "revisa el wifi". Por eso este mapeo es más
 * filoso que el de [cl.rutbusiness.app.ui.common.aCopy], que traduce fallas de
 * pantallas que ya están adentro y no tienen ese dato.
 */
internal fun fallaDeLogin(
    error: AppError,
    direccion: String,
    nube: Boolean = false,
): FallaDeConexion = when (error) {
    is AppError.ErrorDelServidor ->
        if (error.code == "NECESITA_NEGOCIO") {
            FallaDeConexion.FaltaNombreCorto(error.technical)
        } else {
            FallaDeConexion.ContestaPeroNoEsElSistema(direccion, error.technical, nube)
        }

    // El server contesta lo mismo para negocio, correo y clave equivocados, así
    // que el mensaje nombra los tres en vez de adivinar cuál fue.
    is AppError.CredencialesInvalidas -> FallaDeConexion.DatosQueNoCoinciden(error.technical)

    // La dirección pasó el sondeo, así que si acá se cae la conexión es que se
    // cortó en el medio: el wifi del local, el computador que se durmió.
    is AppError.ServidorNoResponde ->
        FallaDeConexion.NadieContesta(direccion, error.technical, nube)

    // Nos contestó y nos dijo que no, con algo que no son credenciales malas: el
    // sistema está ahí pero no está en condiciones de trabajar.
    else -> FallaDeConexion.ContestaPeroNoEsElSistema(direccion, error.technical, nube)
}

/**
 * Cómo se **muestra** una falla de entrada: tarjeta legible, no muro rojo.
 *
 * Mismo criterio que el cartel del alta: borde grueso y superficie elevada
 * para verse al sol sin pintar la pantalla de `dangerContainer` (que al 200%
 * se lee como alarma, no como ayuda). El copy sigue siendo [FallaDeConexion]
 * — acá no se inventa IP ni se nombra el computador.
 */
@Composable
internal fun TarjetaDeFallaEntrada(
    titulo: String,
    queHacer: String,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surfaceRaised)
            .border(dimens.focusRing, colors.outlineStrong, shape)
            .padding(dimens.space3)
            .rbAssertive(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = queHacer,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )
    }
}
