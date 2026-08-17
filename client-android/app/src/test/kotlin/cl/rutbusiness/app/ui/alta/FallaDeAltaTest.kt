package cl.rutbusiness.app.ui.alta

import cl.rutbusiness.app.ui.entrada.FallaDeConexion
import cl.rutbusiness.core.error.AppError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Las cuatro fallas del alta: que existan, que digan cosas distintas, y que
 * cada una salga de lo que de verdad la causa.
 *
 * Es el 80% del valor de esta pantalla. Crear un negocio sale bien una vez por
 * teléfono; sale mal las veces que haga falta, y de esas veces depende que la
 * app se siga usando. Va en la JVM, sin emulador ni server, porque un mapeo de
 * errores que sólo se puede probar desenchufando un router no se prueba nunca.
 */
class FallaDeAltaTest {

    private val donde = "http://192.168.1.10:8080"
    private val email = "rosa@almacen.cl"

    // --- 1. el correo ya no puede crear un negocio acá ----------------------

    @Test
    fun `un 409 dice que ese correo no puede crear un negocio ahi`() {
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(
                status = 409,
                code = "SETUP_ALREADY_DONE",
                serverMessage = "Este servidor ya tiene una cuenta. Pide a tu administrador " +
                    "que cree usuarios nuevos.",
            ),
            donde = donde,
            email = email,
        )

        assertTrue("un 409 tiene que ser el choque con lo que ya existe", falla is FallaDeAlta.CorreoYaTieneNegocio)
        // Nombra el correo que acaba de escribir: es lo primero que va a mirar.
        assertTrue("no nombra el correo: ${falla.queHacer}", falla.queHacer.contains(email))
        // Y manda a la puerta que sí funciona, en vez de a cambiar de correo.
        assertTrue(
            "no ofrece entrar en vez de crear: ${falla.queHacer}",
            falla.queHacer.contains("Entrar a mi negocio"),
        )
    }

    // --- 2. la clave es muy corta -------------------------------------------

    @Test
    fun `una clave corta se detecta en el telefono, sin viaje a la red`() {
        assertTrue(revisarLaClave("1234") is FallaDeAlta.ClaveDebil)
        assertTrue(revisarLaClave("secreto") is FallaDeAlta.ClaveDebil)
        assertNull("ocho caracteres es el mínimo del server", revisarLaClave("almacen1"))
    }

    /**
     * Y también si el server la rechaza: el mínimo lo fija él, y el día que lo
     * mueva la app tiene que seguir diciendo "clave corta" y no un genérico.
     */
    @Test
    fun `un 400 que habla de la contrasena es falla de clave`() {
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(
                status = 400,
                code = "INVALID_INPUT",
                serverMessage = "La contraseña debe tener al menos 8 caracteres.",
            ),
            donde = donde,
            email = email,
        )
        assertTrue(falla is FallaDeAlta.ClaveDebil)
    }

    /**
     * Un 400 que habla de otra cosa muestra el mensaje del server tal cual: ya
     * viene en español y escrito para la dueña. Inventarle una segunda versión
     * acá es garantizar que las dos se desincronicen.
     */
    @Test
    fun `un 400 de otra cosa repite el mensaje del server`() {
        val delServer = "El correo no tiene un formato válido (ej: dueno@minegocio.cl)."
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(400, "INVALID_INPUT", delServer),
            donde = donde,
            email = email,
        )
        assertTrue(falla is FallaDeAlta.DatosRechazados)
        assertEquals(delServer, falla.queHacer)
        // Título hablado: no culpa a un «sistema».
        assertEquals("Esos datos no sirvieron", falla.titulo)
        assertFalse(falla.titulo.contains("sistema", ignoreCase = true))
    }

    @Test
    fun `datos rechazados sin mensaje manda a revisar lo escrito`() {
        val falla = FallaDeAlta.DatosRechazados("")
        assertEquals("Esos datos no sirvieron", falla.titulo)
        assertTrue(falla.queHacer.contains("Revisa", ignoreCase = true))
        assertFalse(falla.titulo.contains("sistema", ignoreCase = true))
        assertFalse(falla.queHacer.contains("sistema", ignoreCase = true))
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
    }

    // --- 3. sin internet ----------------------------------------------------

    @Test
    fun `sin red se dice que el telefono no esta conectado`() {
        val falla = deLaConexion(FallaDeConexion.SinRed(), donde)

        assertTrue(falla is FallaDeAlta.SinRed)
        // Manda a prender el wifi del teléfono, no a revisar el computador.
        assertTrue("no manda a prender la red: ${falla.queHacer}", falla.queHacer.contains("wifi"))
    }

    // --- 4. el servicio no responde -----------------------------------------

    @Test
    fun `sin respuesta se dice que el servicio no contesta`() {
        val falla = fallaDeAlta(
            error = AppError.ServidorNoResponde(donde, technical = "timeout"),
            donde = donde,
            email = email,
        )
        assertTrue(falla is FallaDeAlta.ServicioNoResponde)
    }

    @Test
    fun `contestar algo que no es el sistema tambien es que el servicio no sirve`() {
        val falla = deLaConexion(FallaDeConexion.ContestaPeroNoEsElSistema(donde, "200 hotel wifi"), donde)
        assertTrue(falla is FallaDeAlta.ServicioNoResponde)
    }

    /**
     * **La regla que no se puede romper.** Un intento que se corta sin
     * respuesta no prueba que el server no haya alcanzado a guardar: el corte
     * puede haber sido en la vuelta. Decir "no se creó nada" y que después
     * resulte que sí deja a la dueña reintentando contra un "acá ya hay un
     * negocio" y convencida de que la app está rota.
     */
    @Test
    fun `el servicio caido nunca promete que no se creo nada`() {
        val falla = FallaDeAlta.ServicioNoResponde(donde)
        val prohibidas = listOf("no se creó", "no se creo", "nada se creó", "nada se creo")
        val dichas = prohibidas.filter { falla.queHacer.contains(it, ignoreCase = true) }
        assertTrue("promete lo que no puede saber: $dichas", dichas.isEmpty())
        // Y en cambio dice qué va a ver si sí alcanzó a crearse.
        assertTrue(falla.queHacer.contains("ya hay un negocio"))
    }

    // --- lo transversal -----------------------------------------------------

    /**
     * El negocio creado sin sesión abierta es el peor mensaje posible de
     * escribir mal: si dijera "no se pudo crear", la dueña reintentaría,
     * chocaría con el 409 y creería que la app está rota.
     */
    @Test
    fun `creado sin poder entrar dice que el negocio existe y da el nombre corto`() {
        val falla = FallaDeAlta.CreadoPeroNoEntro("almacen-dona-rosa")

        assertTrue(falla.titulo.contains("quedó creado"))
        assertTrue(falla.queHacer.contains("almacen-dona-rosa"))
        assertTrue(falla.queHacer.contains("Entrar a mi negocio"))
        assertTrue(falla.queHacer.contains("negocio es: almacen-dona-rosa"))
    }

    @Test
    fun `creado sin poder entrar en feria o nube habla de puesto`() {
        val falla = FallaDeAlta.CreadoPeroNoEntro("huevos-de-marta", esPuesto = true)

        assertTrue(falla.titulo.contains("puesto"))
        assertTrue(falla.queHacer.contains("puesto es: huevos-de-marta"))
        assertFalse(falla.queHacer.contains("negocio es:"))
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
    }

    // --- slug sugerido en SLUG_TOMADO ---------------------------------------

    @Test
    fun `slugSugeridoEn lee el token despues de Proba con`() {
        val msg = "Ese nombre corto ya lo usa otro puesto. Probá con huevos-de-marta-2."
        assertEquals("huevos-de-marta-2", slugSugeridoEn(msg))
    }

    @Test
    fun `slugSugeridoEn acepta Proba sin tilde`() {
        assertEquals(
            "puesto-rosa-3",
            slugSugeridoEn("Ese nombre corto ya lo usa otro puesto. Proba con puesto-rosa-3."),
        )
    }

    @Test
    fun `slugSugeridoEn en correo tomado devuelve null`() {
        assertNull(
            slugSugeridoEn("Ese correo ya tiene un puesto. Entrá con tu clave, no crees otro."),
        )
    }

    /** Cuatro fallas que dicen lo mismo son una sola falla con cuatro nombres. */
    @Test
    fun `las cuatro dicen cosas distintas`() {
        val fallas = listOf(
            FallaDeAlta.SinRed(),
            FallaDeAlta.ServicioNoResponde(donde),
            FallaDeAlta.CorreoYaTieneNegocio(email, donde),
            FallaDeAlta.ClaveDebil(),
        )
        assertEquals("hay títulos repetidos", 4, fallas.map { it.titulo }.toSet().size)
        assertEquals("hay instrucciones repetidas", 4, fallas.map { it.queHacer }.toSet().size)
    }

    /**
     * Cero jerga, la misma regla del primer uso.
     *
     * "servidor" queda fuera a propósito: la palabra se enseña una vez en
     * [cl.rutbusiness.app.ui.entrada.PrimerUso] y no se repite. Acá el objeto
     * del que se habla es "el computador del negocio".
     */
    @Test
    fun `ninguna falla usa jerga`() {
        val prohibidas = listOf(
            "endpoint", "API", "HTTP", "tenant", "token", "JSON", "status", "404", "409",
            "timeout", "servidor",
        )
        val fallas = listOf(
            FallaDeAlta.SinRed(),
            FallaDeAlta.ServicioNoResponde(donde),
            FallaDeAlta.CorreoYaTieneNegocio(email, donde),
            FallaDeAlta.ClaveDebil(),
            FallaDeAlta.CreadoPeroNoEntro("almacen-dona-rosa"),
        )
        fallas.forEach { falla ->
            val texto = "${falla.titulo} ${falla.queHacer}"
            val encontradas = prohibidas.filter { texto.contains(it, ignoreCase = true) }
            assertTrue("«${falla.titulo}» dice $encontradas", encontradas.isEmpty())
        }
    }

    /** El detalle técnico se guarda, pero jamás en lo que se muestra. */
    @Test
    fun `el detalle tecnico viaja aparte y no se mezcla en el mensaje`() {
        val crudo = "java.net.SocketTimeoutException: failed to connect to /192.168.1.10"
        val falla = fallaDeAlta(
            error = AppError.ServidorNoResponde(donde, technical = crudo),
            donde = donde,
            email = email,
        )
        assertNotNull(falla.tecnico)
        assertTrue(falla.queHacer.contains(crudo).not())
        assertTrue(falla.titulo.contains(crudo).not())
    }

    /** La dirección se muestra como la escribió ella, sin el `http://` nuestro. */
    @Test
    fun `slug tomado no se lee como computador ocupado`() {
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(
                status = 409,
                code = "SLUG_TOMADO",
                serverMessage = "Ese nombre corto ya lo usa otro puesto. Probá con otro nombre.",
            ),
            donde = "https://nube.test.invalid",
            email = email,
            nube = true,
        )
        assertTrue(falla is FallaDeAlta.DatosRechazados)
        assertTrue(falla.queHacer.contains("otro puesto"))
        assertFalse(falla.titulo.contains("sistema", ignoreCase = true))
        assertFalse(falla.titulo.contains("computador", ignoreCase = true))
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
        assertFalse(falla.queHacer.contains("nube.test.invalid"))
        assertFalse(falla.titulo.contains("192.168"))
        assertFalse(falla.titulo.contains("10.0.2.2"))
    }

    /**
     * Día 1 feria/nube: los títulos de falla no culpan al «sistema», no
     * nombran computador ni filtran IP. La dueña lee una instrucción, no un
     * diagnóstico de infra.
     */
    @Test
    fun `titulos feria y nube nunca dicen sistema computador ni IP`() {
        val fallas = listOf(
            FallaDeAlta.SinRed(esFeria = true),
            FallaDeAlta.ServicioNoResponde("http://10.0.2.2:8080", nube = true),
            FallaDeAlta.CorreoYaTieneNegocio(email, "http://10.0.2.2:8080", nube = true),
            FallaDeAlta.ClaveDebil(),
            FallaDeAlta.DatosRechazados("Ese nombre corto ya lo usa otro puesto."),
            FallaDeAlta.CreadoPeroNoEntro("huevos-de-marta", esPuesto = true),
        )
        val prohibidasEnTitulo = listOf(
            "sistema", "computador", "192.168", "10.0.2.2",
        )
        fallas.forEach { falla ->
            val halladas = prohibidasEnTitulo.filter {
                falla.titulo.contains(it, ignoreCase = true)
            }
            assertTrue(
                "título feria/nube filtró $halladas: «${falla.titulo}»",
                halladas.isEmpty(),
            )
        }
    }

    /**
     * CORREO_TOMADO no se arregla reintentando en esta pantalla -a diferencia
     * de SLUG_TOMADO- así que no alcanza con repetir el mensaje pelado del
     * server (que no dice cómo volver a entrar): tiene que caer en la falla
     * que sí sabe decir el camino, con el nombre exacto del botón de
     * [cl.rutbusiness.app.ui.entrada.Puerta] («Entrar a mi puesto»), no un
     * «Entrá» genérico que la dueña no va a encontrar escrito en ningún botón.
     */
    @Test
    fun `correo tomado manda a entrar y no a inventar otro correo`() {
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(
                status = 409,
                code = "CORREO_TOMADO",
                serverMessage = "Ese correo ya tiene un puesto. Entrá con tu clave, no crees otro.",
            ),
            donde = "https://nube.test.invalid",
            email = email,
            nube = true,
        )
        assertTrue(falla is FallaDeAlta.CorreoYaTieneNegocio)
        assertTrue("no dice el botón exacto: ${falla.queHacer}", falla.queHacer.contains("Entrar a mi puesto"))
        assertTrue(falla.queHacer.contains(email))
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
    }

    @Test
    fun `en la nube el servicio caido no nombra una IP`() {
        val falla = FallaDeAlta.ServicioNoResponde("https://nube.test.invalid", nube = true)
        assertFalse(falla.queHacer.contains("nube.test.invalid"))
        assertFalse(falla.queHacer.contains("192.168"))
        assertTrue(falla.queHacer.contains("internet"))
        assertTrue(falla.queHacer.contains("ya están") || falla.queHacer.contains("ya hay"))
    }

    /**
     * SETUP_ALREADY_DONE en la nube: el host compilado (`10.0.2.2`, más adelante
     * un dominio real) no es una dirección que el feriante haya escrito. Decírsela
     * es un leak y suena a "computador".
     */
    @Test
    fun `en la nube correo ya tiene negocio no nombra IP ni computador`() {
        val falla = FallaDeAlta.CorreoYaTieneNegocio(
            email = email,
            donde = "http://10.0.2.2:8080",
            nube = true,
        )
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
        assertFalse(falla.queHacer.contains("10.0.2.2"))
        assertFalse(falla.queHacer.contains("192.168"))
        assertTrue(
            "manda a entrar: ${falla.queHacer}",
            falla.queHacer.contains("Entrar", ignoreCase = true),
        )
    }

    @Test
    fun `un 409 SETUP_ALREADY_DONE de nube no muestra host ni computador`() {
        val falla = fallaDeAlta(
            error = AppError.ErrorDelServidor(
                status = 409,
                code = "SETUP_ALREADY_DONE",
                serverMessage = "Este servidor ya tiene una cuenta.",
            ),
            donde = "http://10.0.2.2:8080",
            email = email,
            nube = true,
        )
        assertTrue(falla is FallaDeAlta.CorreoYaTieneNegocio)
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
        assertFalse(falla.queHacer.contains("10.0.2.2"))
        assertFalse(falla.queHacer.contains("192.168"))
    }

    /** En local el 409 sigue nombrando el host como lo escribió ella. */
    @Test
    fun `en local correo ya tiene negocio si nombra el host sin http`() {
        val falla = FallaDeAlta.CorreoYaTieneNegocio(email, donde)
        assertTrue(falla.queHacer.contains("192.168.1.10:8080"))
        assertFalse(falla.queHacer.contains("http://"))
    }

    @Test
    fun `en feria sin red manda a crear mi puesto`() {
        val falla = FallaDeAlta.SinRed(esFeria = true)
        assertTrue(falla.queHacer.contains("Crear mi puesto"))
        assertFalse(falla.queHacer.contains("Crear mi negocio"))
    }

    @Test
    fun `sin feria sin red manda a crear mi negocio`() {
        val falla = FallaDeAlta.SinRed()
        assertTrue(falla.queHacer.contains("Crear mi negocio"))
    }

    @Test
    fun `los mensajes no muestran el esquema que agrego el normalizador`() {
        val falla = FallaDeAlta.ServicioNoResponde("http://192.168.1.10:8080")
        assertTrue(falla.queHacer.contains("192.168.1.10:8080"))
        assertTrue("muestra maquinaria: ${falla.queHacer}", !falla.queHacer.contains("http://"))
    }

    // --- feria local (sin nube compilada) también habla de puesto -----------

    /**
     * Antes de este fix, [FallaDeAlta.ServicioNoResponde] sólo miraba `nube`
     * para elegir la palabra: una feria en un servidor propio (offline, sin
     * nube compilada) leía "negocio" en la mitad del mensaje. Mismo criterio
     * que `CreadoPeroNoEntro.esPuesto`: nube o feria, cualquiera alcanza.
     */
    @Test
    fun `servicio caido en feria local dice puesto y no negocio`() {
        val falla = deLaConexion(
            problema = FallaDeConexion.ContestaPeroNoEsElSistema(donde, "200 hotel wifi"),
            donde = donde,
            esFeria = true,
        )
        assertTrue(falla is FallaDeAlta.ServicioNoResponde)
        assertTrue("no dice puesto: ${falla.queHacer}", falla.queHacer.contains("puesto"))
        assertFalse("sigue diciendo negocio: ${falla.queHacer}", falla.queHacer.contains("negocio"))
    }

    /** Mismo gap, misma corrección, para [FallaDeAlta.CorreoYaTieneNegocio]. */
    @Test
    fun `correo tomado en feria local dice puesto y el boton exacto`() {
        val falla = FallaDeAlta.CorreoYaTieneNegocio(email, donde, esFeria = true)
        assertEquals("Ese correo no puede crear un puesto acá", falla.titulo)
        assertTrue(falla.queHacer.contains("Entrar a mi puesto"))
        assertFalse(falla.queHacer.contains("negocio"))
    }
}
