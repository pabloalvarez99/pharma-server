package cl.rutbusiness.core.session

import android.content.Context
import androidx.credentials.CredentialManager
import androidx.credentials.CustomCredential
import androidx.credentials.GetCredentialRequest
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.NoCredentialException
import com.google.android.libraries.identity.googleid.GetGoogleIdOption
import com.google.android.libraries.identity.googleid.GoogleIdTokenCredential

/**
 * Pide una cuenta de Google de verdad, con el selector del sistema (ADR-0022).
 *
 * Vive en `androidMain` y no en la pantalla porque acá se importa `android.*` y
 * `ui/` no puede (ADR-0021). La pantalla sigue viendo sólo la interfaz
 * [IdentidadGoogle]; de este archivo no sabe nada.
 *
 * ## El client id no está en este archivo, y no puede estarlo
 *
 * Llega por constructor desde `BuildConfig`, que a su vez lo saca de
 * `local.properties` (gitignorado) o del entorno — ver `app/build.gradle.kts`.
 * En el repo no hay ningún valor. Un client id de Android **no es un secreto**
 * (viaja dentro del APK y cualquiera lo lee descomprimiéndolo), pero la Regla 3
 * no hace excepciones por grado: el día que al lado haya que poner algo que sí
 * es secreto, el camino ya existe y nadie tiene que inventarlo con apuro.
 *
 * ## `pedirCuenta` devuelve el `id_token` y se olvida de él
 *
 * El token no se guarda, no se loguea y no se escribe a disco. Viaja una vez
 * hacia `POST /api/v1/auth/google`, el server lo verifica contra las llaves
 * públicas de Google y a cambio emite **nuestro** JWT, que es el único que se
 * persiste. Un `id_token` en un log es una sesión ajena regalada.
 *
 * @param contexto **tiene que ser el de una Activity**. El selector de cuentas
 *   es UI del sistema y necesita una ventana donde montarse; con el contexto de
 *   la aplicación, Credential Manager falla en tiempo de ejecución. Por eso se
 *   arma en `MainActivity` y no en `AppContainer`, igual que
 *   `CompartirTarjetaAndroid`.
 */
class IdentidadGoogleAndroid(
    private val contexto: Context,
    private val clientId: String,
) : IdentidadGoogle {

    private val credenciales by lazy { CredentialManager.create(contexto) }

    /**
     * Este constructor sólo se usa con un client id no vacío — el gate está en
     * [identidadGoogleDeEsteBuild], que es el único lugar que decide.
     */
    override fun disponible(): Boolean = clientId.isNotBlank()

    override fun copyPromocion(rubroEsFeria: Boolean): String =
        if (rubroEsFeria) {
            "Entrá con tu cuenta de Google y no te acordás de ninguna clave " +
                "más. Es la misma cuenta del teléfono."
        } else {
            "Entrá con tu cuenta de Google, sin teclear la clave cada vez."
        }

    override suspend fun pedirCuenta(): ResultadoGoogle {
        val opcion = GetGoogleIdOption.Builder()
            // El client id **Web**, no el de Android: es el que el server espera
            // encontrar en el `aud` del token. Poner el de Android acá hace que
            // el server rechace todos los tokens con 401 y el error no dice por
            // qué. Ver el comentario de `rb.googleClientId` en el gradle.
            .setServerClientId(clientId)
            // `false` = mostrar todas las cuentas del teléfono, no sólo las que
            // ya usaron esta app. Filtrar por autorizadas es lo correcto para
            // volver a entrar, pero acá casi siempre es la primera vez: con el
            // filtro puesto, a quien nunca entró le aparece "no hay cuentas" y
            // se queda mirando un botón que no hace nada.
            .setFilterByAuthorizedAccounts(false)
            // Nada de entrar solo. Con una sola cuenta en el teléfono, el
            // auto-select mete a la persona al negocio sin que haya elegido.
            .setAutoSelectEnabled(false)
            .build()

        val pedido = GetCredentialRequest.Builder()
            .addCredentialOption(opcion)
            .build()

        return try {
            val respuesta = credenciales.getCredential(contexto, pedido)
            val credencial = respuesta.credential

            if (credencial !is CustomCredential ||
                credencial.type != GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_CREDENTIAL
            ) {
                return ResultadoGoogle.Error(
                    "El teléfono devolvió algo que no es una cuenta de Google. " +
                        "Usá correo y clave.",
                )
            }

            val google = GoogleIdTokenCredential.createFrom(credencial.data)
            if (google.idToken.isBlank()) {
                return ResultadoGoogle.Error(
                    "Google no devolvió la credencial. Probá de nuevo o usá " +
                        "correo y clave.",
                )
            }

            ResultadoGoogle.Listo(
                email = google.id.takeIf { it.isNotBlank() },
                displayName = google.displayName,
                idToken = google.idToken,
            )
        } catch (_: GetCredentialCancellationException) {
            // Tocó atrás o cerró el selector. No es un error y no lleva cartel:
            // avisarle "cancelaste" a quien acaba de cancelar a propósito es
            // ruido.
            ResultadoGoogle.Cancelado
        } catch (e: NoCredentialException) {
            // No hay ninguna cuenta de Google en el teléfono. Es el caso normal
            // de un aparato de feria recién comprado, no una falla.
            ResultadoGoogle.NoDisponible(
                mensajeUsuario = "Este teléfono no tiene ninguna cuenta de " +
                    "Google agregada. Usá el correo y la clave del negocio.",
                razonInterna = "no_credential: ${e.type}",
            )
        } catch (e: GetCredentialException) {
            // Sin Play Services, sin red, o el proveedor falló. El mensaje
            // técnico no se le muestra a nadie: no dice nada que la dueña del
            // puesto pueda hacer.
            ResultadoGoogle.Error(
                "No se pudo abrir la lista de cuentas de Google. " +
                    "Usá el correo y la clave de abajo.",
            ).also { registrarFalla(e) }
        }
    }

    /**
     * Diagnóstico. Loguea el **tipo** de la falla, nunca el contenido de la
     * credencial: `GetCredentialException.errorMessage` puede traer datos de la
     * cuenta, y esto sale por logcat, que en Android lee cualquier app con
     * permisos de desarrollo.
     */
    private fun registrarFalla(e: GetCredentialException) {
        android.util.Log.w("IdentidadGoogle", "picker falló: ${e.type}")
    }
}

/**
 * El único lugar donde se decide si este build tiene Google.
 *
 * **Sin client id devuelve exactamente [IdentidadGoogleNoCableada]** — el mismo
 * objeto que la app usa hoy, no una implementación parecida que devuelve
 * `false`. Es la diferencia entre "se comporta igual" y "es lo mismo", y es lo
 * que hace que este carril se pueda mergear antes de que existan las
 * credenciales en la consola de Google: un APK sin `rb.googleClientId` arma el
 * mismo grafo de objetos que armaba antes de este commit.
 *
 * El contexto entra como lambda y no como valor por la misma razón: en un build
 * sin client id **no se evalúa nunca**. Así el camino de hoy no depende de que
 * exista una Activity, y el test puede probarlo sin Robolectric — con una
 * lambda que revienta si alguien la toca.
 *
 * Lo verifica `IdentidadGoogleDeEsteBuildTest`.
 */
fun identidadGoogleDeEsteBuild(clientId: String?, contexto: () -> Context): IdentidadGoogle =
    if (clientId.isNullOrBlank()) {
        IdentidadGoogleNoCableada
    } else {
        IdentidadGoogleAndroid(contexto(), clientId)
    }
