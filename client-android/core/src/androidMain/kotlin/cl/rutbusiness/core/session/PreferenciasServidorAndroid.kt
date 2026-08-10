package cl.rutbusiness.core.session

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "rutbusiness_config")

/**
 * Lo que el operador no debería tener que tipear dos veces: dirección del
 * server, sucursal y correo.
 *
 * Acá **nunca** entra la contraseña ni el token: DataStore guarda en texto
 * plano. El token va cifrado en [TokenStoreAndroid].
 */
internal class PreferenciasServidorAndroid(context: Context) : PreferenciasServidor {

    private val store = context.applicationContext.dataStore

    override suspend fun leerBaseUrl(): String? = leer(BASE_URL)

    override suspend fun guardarBaseUrl(url: String) {
        store.edit { it[BASE_URL] = url }
    }

    override suspend fun leerUltimoTenant(): String? = leer(TENANT)

    override suspend fun leerUltimoEmail(): String? = leer(EMAIL)

    override suspend fun guardarUltimoAcceso(tenant: String, email: String) {
        store.edit {
            it[TENANT] = tenant
            it[EMAIL] = email
        }
    }

    private suspend fun leer(clave: Preferences.Key<String>): String? =
        store.data.first()[clave]?.takeIf { it.isNotBlank() }

    private companion object {
        val BASE_URL = stringPreferencesKey("base_url")
        val TENANT = stringPreferencesKey("ultimo_tenant")
        val EMAIL = stringPreferencesKey("ultimo_email")
    }
}
