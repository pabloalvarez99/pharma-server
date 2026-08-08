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
 * Dónde vive el almacenamiento persistente. En Android es Keystore + DataStore
 * + archivos; en iOS será Keychain + UserDefaults + archivos. El resto del
 * código no se entera.
 *
 * Tres cosas distintas con tres necesidades distintas: el token va cifrado
 * porque es una credencial, las preferencias van en texto plano porque son la
 * dirección del server, y los bloques van en archivos sueltos porque el caché
 * y la cola de ventas no pueden quedar residentes en memoria.
 */
expect class AlmacenamientoPlataforma {
    val tokens: TokenStore
    val preferencias: PreferenciasServidor

    /** Disco para el caché de lecturas y la cola de ventas sin mandar. */
    val bloques: cl.rutbusiness.core.offline.AlmacenDeBloques
}
