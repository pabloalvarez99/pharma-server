package cl.rutbusiness.core.session

/**
 * Guarda el JWT. La implementación de cada plataforma decide *cómo*, pero el
 * contrato es que **nunca queda en texto plano**.
 */
interface TokenStore {
    suspend fun leer(): String?
    suspend fun guardar(token: String)
    suspend fun borrar()
}

/**
 * Lo que el operador tipeó y no queremos que vuelva a tipear: la dirección del
 * server, la sucursal y el correo. Acá **no** va la contraseña.
 */
interface PreferenciasServidor {
    suspend fun leerBaseUrl(): String?
    suspend fun guardarBaseUrl(url: String)
    suspend fun leerUltimoTenant(): String?
    suspend fun leerUltimoEmail(): String?
    suspend fun guardarUltimoAcceso(tenant: String, email: String)
}

/**
 * Segundo y último punto `expect/actual` del proyecto: dónde vive el
 * almacenamiento persistente. En Android es Keystore + DataStore; en iOS será
 * Keychain + UserDefaults. El resto del código no se entera.
 */
expect class AlmacenamientoPlataforma {
    val tokens: TokenStore
    val preferencias: PreferenciasServidor
}
