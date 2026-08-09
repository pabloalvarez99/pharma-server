package cl.rutbusiness.app.ui.alta

/**
 * Los nueve rubros del catálogo, copiados del server.
 *
 * **Fuente de verdad: `crates/domain/src/rubro.rs`, la tabla `PACKS`.** De ahí
 * salen la clave, la etiqueta y la frase de cada uno, en ese orden. La clave es
 * lo único que viaja: es la que `domain::provisioning::validate_vertical`
 * acepta, y la que después decide qué módulos prende el pack del rubro
 * (`GET /api/v1/rubro-pack`). Mandar una clave que no esté en esta lista es un
 * 400 del server, no un rubro nuevo.
 *
 * Ojo con `docs/strategy/rubro-catalog.md`: la tabla de ese documento dice que
 * las claves internas son en inglés (`pharmacy`, `restaurant`…). Eso quedó
 * viejo — el código dice `farmacia`, y el código es el que valida. La prueba
 * `validate_vertical("farmacia")` del crate lo fija.
 *
 * **Por qué está duplicado acá.** El alta ocurre *antes* de tener sesión, y el
 * único endpoint que sirve el catálogo —`GET /api/v1/rubro-pack`— pide JWT y
 * además devuelve un solo pack, el del tenant. No hay forma de pedirle la lista
 * al server sin haber entrado, así que la lista viaja en la app. Es una copia y
 * como toda copia se puede desincronizar: `RubrosTest` compara esta lista
 * contra `rubro.rs` leyéndolo del repo, así que si alguien agrega un rubro en
 * el server y no acá, el build de la app se cae.
 */
data class Rubro(
    /** Lo que se manda como `vertical`. Igual a `RubroPack.rubro` en el server. */
    val clave: String,
    /** Lo que lee la dueña. Igual a `RubroPack.label`. */
    val etiqueta: String,
    /** Una línea que ayuda a reconocerse. Igual a `RubroPack.tagline`. */
    val frase: String,
)

/**
 * En el mismo orden que `PACKS`. `feria` va primero porque es el foco activo
 * del producto (ADR-0022) y porque el primer rubro de la lista es el que se lee
 * sin desplazar en un teléfono chico. `otro` va al final y no es el elegido por
 * defecto: el alta obliga a tocar uno, porque un rubro elegido de verdad
 * configura la app y un rubro puesto por descarte no configura nada.
 */
val RUBROS: List<Rubro> = listOf(
    Rubro("feria", "Feria / Calle", "Tu puesto, sin cuaderno."),
    Rubro("farmacia", "Farmacia", "Tu farmacia, en regla."),
    Rubro("minimarket", "Minimarket / Almacén", "Tu almacén, al día."),
    Rubro("restaurant", "Restaurant / Comida", "Tu cocina, bajo control."),
    Rubro("cafe", "Café / Pastelería", "Tu café, listo cada mañana."),
    Rubro("tienda", "Tienda / Retail", "Tu tienda, ordenada."),
    Rubro("belleza", "Belleza / Estética", "Tu salón, agendado."),
    Rubro("servicios", "Servicios / Oficios", "Tu oficio, facturado."),
    Rubro("otro", "Otro", "Tu negocio, a tu manera."),
)
