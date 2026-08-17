package cl.rutbusiness.app.ui.alta

/**
 * Copy de los carteles del alta, extraído para tests JVM sin Compose.
 *
 * Feria habla de puesto y voseo; retail conserva «negocio» y el ejemplo
 * minegocio. [PasoDonde] es solo on-prem (APK sin URL horneada) y sí puede
 * nombrar computador + IP de LAN — no se reutiliza en Cuenta/Negocio/Rubro.
 */

internal data class CopyPasoCuenta(
    val titulo: String,
    val labelCorreo: String,
    val placeholderCorreo: String,
    val ayudaCorreo: String,
    val labelClave: String,
    val ayudaClave: String,
)

internal fun copyPasoCuenta(esFeria: Boolean): CopyPasoCuenta =
    if (esFeria) {
        CopyPasoCuenta(
            titulo = "Con esto vas a entrar",
            labelCorreo = "Tu correo",
            placeholderCorreo = "marta@correo.cl",
            ayudaCorreo = "Con esto entras al puesto.",
            labelClave = "Tu clave",
            ayudaClave = "Al menos $LARGO_MINIMO_DE_CLAVE letras o números. Anótala: nadie " +
                "puede recuperártela por vos.",
        )
    } else {
        CopyPasoCuenta(
            titulo = "Con esto vas a entrar",
            labelCorreo = "Tu correo",
            placeholderCorreo = "dueno@minegocio.cl",
            ayudaCorreo = "Es tu nombre de usuario para entrar.",
            labelClave = "Tu clave",
            ayudaClave = "Al menos $LARGO_MINIMO_DE_CLAVE letras o números. Anótala: nadie " +
                "puede recuperártela por ti.",
        )
    }

internal data class CopyPasoDonde(
    val titulo: String,
    val cuerpo: String,
    val labelDireccion: String,
    val placeholder: String,
    val ayudaSinDireccion: String,
)

/** Solo camino on-prem / LAN. No usar en pasos visibles de nube. */
internal fun copyPasoDonde(): CopyPasoDonde =
    CopyPasoDonde(
        titulo = "El computador del negocio",
        cuerpo = "Tus ventas no se guardan en el teléfono: se guardan en un computador tuyo. " +
            "Puede estar en el local o arrendado en internet.",
        labelDireccion = "Dirección del computador",
        placeholder = "192.168.1.10:8080",
        ayudaSinDireccion = "¿No la tienes? Pídesela a quien lo instaló.",
    )

internal data class CopyPasoNegocio(
    val titulo: String,
    val label: String,
    val placeholder: String,
    val ayuda: String,
)

internal fun copyPasoNegocio(esFeria: Boolean): CopyPasoNegocio =
    if (esFeria) {
        CopyPasoNegocio(
            titulo = "El nombre de tu puesto",
            label = "Nombre del puesto",
            placeholder = "Huevos de Marta",
            ayuda = "Como te gritan en la feria. Después se puede cambiar.",
        )
    } else {
        CopyPasoNegocio(
            titulo = "El nombre de tu negocio",
            label = "Nombre del negocio",
            placeholder = "Almacén Doña Rosa",
            ayuda = "El que le dice la gente del barrio. Después se puede cambiar.",
        )
    }
