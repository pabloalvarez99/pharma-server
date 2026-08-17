package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.rubro.AttrField
import cl.rutbusiness.core.rubro.RubroPack

/**
 * Cómo se llama todo en esta pantalla, según el pack del rubro.
 *
 * **Nada de esto es una constante de la app.** `GET /api/v1/rubro-pack` ya trae
 * `vocab.item` / `vocab.catalog` y la lista de `attrs` (ADR-0022,
 * `domain::rubro`), así que en feria la misma pantalla dice "Cosa" / "Lo que
 * vendo" / "Se vende por", en farmacia dice "Producto" / "Inventario", y en un
 * restaurant "Plato" / "Carta". Escribir "Producto" a mano acá sería tener dos
 * fuentes de verdad y que una se quedara vieja.
 *
 * Está separado de la pantalla para poder probarlo sin montar Compose: el copy
 * de un rubro es una función pura del pack.
 */
internal data class CopyCatalogo(
    /** Título de la pantalla: "Lo que vendo", "Inventario", "Carta". */
    val titulo: String,
    /**
     * Subtítulo de la lista: mesa del puesto, no panel de ERP.
     * Feria: "Lo que se vende hoy"; formal: "Lo que puedes cobrar".
     */
    val subtitulo: String,
    /** Una cosa vendible: "Cosa", "Producto", "Plato". */
    val item: String,
    /** Botón grande de alta: "Agregar una cosa". */
    val agregar: String,
    /** Título del formulario en alta. */
    val tituloAlta: String,
    /** Título del formulario en edición. */
    val tituloEdicion: String,
    /** Subtítulo del alta: confirma que con esto ya se cobra. */
    val subtituloAlta: String,
    /** Rótulo del buscador: "Buscar cosa" / "Buscar producto". */
    val etiquetaBuscar: String,
    /** Placeholder del buscador. */
    val placeholderBuscar: String,
    /** Rótulo del campo nombre. */
    val etiquetaNombre: String,
    /** Ejemplo adentro del campo nombre. */
    val ejemploNombre: String,
    /** Rótulo del campo precio. */
    val etiquetaPrecio: String,
    /** Rótulo del campo unidad — sale del `AttrField` del pack. */
    val etiquetaUnidad: String,
    /** Clave del atributo de unidad dentro de `product.attrs`. */
    val claveUnidad: String,
    /** `false` cuando el pack no declara un campo de unidad: no se dibuja. */
    val hayUnidad: Boolean,
    /** Título del vacío. */
    val vacioTitulo: String,
    /** Qué se gana al cargarlo, dicho en una frase — el «para qué». */
    val vacioBeneficio: String,
    /** Qué hacer, dicho en una frase. */
    val vacioPista: String,
    /** Pista cuando el buscador no encuentra nada. */
    val vacioBusquedaPista: String,
    /**
     * Con la lista recién empezada (una o dos cosas), el hueco entre la
     * última tarjeta y el botón de abajo se llena con esto en vez de quedar
     * en blanco — ver ADR de la ola 27, "espacio vacío enorme incluso con
     * contenido".
     */
    val sugerenciaPocasCosas: String,
    /** Pie del formulario: con nombre y precio alcanza. */
    val pieFormulario: String,
    /**
     * Mesa / agent-home: cambia subtítulos, buscador y confirmaciones.
     * No reescribe el vocab del pack (`titulo` / `item` siguen saliendo de
     * `vocab.catalog` / `vocab.item`).
     */
    val esFeria: Boolean,
)

/**
 * El atributo que hace de "se vende por".
 *
 * Se busca por **clave**, no por posición: `attrs[0]` funcionaría hoy para feria
 * y se rompería el día que el pack agregue un campo antes. Si el rubro no lo
 * declara -farmacia, minimarket-, el formulario no muestra el campo en vez de
 * inventarle uno.
 */
internal fun campoDeUnidad(pack: RubroPack): AttrField? =
    pack.attrs.firstOrNull { it.key == "unidad" }

internal fun copyCatalogo(pack: RubroPack): CopyCatalogo {
    val item = pack.vocab.item.trim().ifEmpty { "Producto" }
    val catalogo = pack.vocab.catalog.trim().ifEmpty { "Inventario" }
    val unidad = campoDeUnidad(pack)
    val feria = pack.features.agentHome || pack.rubro.equals("feria", ignoreCase = true)

    // "una cosa" / "un producto": el artículo cambia con la palabra del pack y
    // no se puede derivar de la gramática sin adivinar. Las tres formas que hoy
    // devuelven los packs son "Cosa" (f), "Producto" (m), "Plato" (m) y
    // "Servicio" (m); cualquier palabra nueva cae en el masculino, que es lo que
    // hace el castellano con lo desconocido.
    val articulo = if (item.endsWith("a", ignoreCase = true)) "una" else "un"
    val enMinuscula = item.replaceFirstChar { it.lowercase() }

    return CopyCatalogo(
        titulo = catalogo,
        // Mesa del puesto vs inventario formal: misma lista, otra voz.
        subtitulo = if (feria) "Lo que se vende hoy" else "Lo que puedes cobrar",
        item = item,
        agregar = "Agregar $articulo $enMinuscula",
        tituloAlta = "$item nuevo".let {
            if (articulo == "una") "$item nueva" else it
        },
        tituloEdicion = "Editar $enMinuscula",
        subtituloAlta = if (feria) {
            "Con el precio ya lo cobras"
        } else {
            "Con esto ya lo puedes cobrar"
        },
        // Feria: "Buscar cosa". Farmacia: "Buscar producto". Sale del vocab.
        etiquetaBuscar = "Buscar $enMinuscula",
        // El placeholder del buscador reusa el ejemplo del nombre: en feria
        // "Tomate, cilantro…"; en formal sin unidad, "Nombre".
        placeholderBuscar = if (unidad != null) "Tomate, cilantro, palta…" else "Nombre",
        etiquetaNombre = "¿Cómo se llama?",
        // El ejemplo habla del rubro sin nombrarlo: en feria "Tomate" es lo
        // primero que se carga, y en un rubro que no conocemos un ejemplo
        // inventado confunde más que ayudar.
        ejemploNombre = if (unidad != null) "Tomate, cilantro, palta…" else "Nombre",
        etiquetaPrecio = "¿A cuánto lo vendes?",
        etiquetaUnidad = unidad?.label?.trim().orEmpty().ifEmpty { "Se vende por" },
        claveUnidad = unidad?.key ?: CLAVE_UNIDAD,
        hayUnidad = unidad != null,
        vacioTitulo = "Todavía no cargaste nada",
        // El «para qué»: sin esto el vacío sólo explica un paso técnico
        // (cómo cargar), nunca por qué vale la pena hacerlo primero.
        vacioBeneficio = if (feria) {
            "Así, cuando vendas, sólo tocas el precio en vez de escribirlo de nuevo."
        } else {
            "Así lo cobras directo desde la lista, sin escribir el precio cada vez."
        },
        // Feria: el vacío enseña el form y el day-1 por el agente.
        // Farmacia/otro: form + "no hace falta código de barras".
        vacioPista = if (feria) {
            "Agrega tomates, cilantro, lo que sea, con el precio. " +
                "O dile al agente: «vendí tomates a 2000»."
        } else {
            "Agrega lo que vendes con su precio y ya puedes cobrarlo. " +
                "No hace falta código de barras ni nada más."
        },
        vacioBusquedaPista =
            "Fíjate cómo se escribe, o agrégalo si todavía no está.",
        sugerenciaPocasCosas = if (feria) {
            "Sigue agregando lo que vendes hoy: mientras más cosas tengas cargadas, " +
                "menos tienes que escribir a mano cuando llega la clienta."
        } else {
            "Sigue agregando lo que vendes: con todo cargado, cobrar es tocar y listo."
        },
        // Feria no nombra código de barras: en el puesto no hay.
        pieFormulario = if (feria) {
            "Con el nombre y el precio alcanza."
        } else {
            "Con el nombre y el precio alcanza. No hace falta código de barras."
        },
        esFeria = feria,
    )
}

/**
 * Chip de confirmación al volver de guardar.
 *
 * Feria: "Listo: tomates" (mesa, sin jerga de sistema). Formal conserva
 * "Guardado:" porque es el lenguaje que ya usa el resto del ERP.
 */
internal fun copyConfirmacionGuardado(nombre: String, feria: Boolean): String {
    val limpio = nombre.trim()
    return if (feria) "Listo: $limpio" else "Guardado: $limpio"
}

/** Cómo se vende, al lado del nombre: "Por kilo". */
internal fun copyComoSeVende(unidad: String): String = "Por ${unidad.trim()}"

/** Título del vacío cuando el buscador no encontró nada. */
internal fun copyVacioBusqueda(consulta: String): String =
    "Nada con \"${consulta.trim()}\""

/** Loading de la lista: "Cargando lo que vendo" / "Cargando inventario". */
internal fun copyCargandoLista(titulo: String): String =
    "Cargando ${titulo.replaceFirstChar { it.lowercase() }}"

/**
 * Las unidades que se ofrecen con un toque.
 *
 * Son **sugerencias**, no un enum: el pack declara que existe un campo "Se vende
 * por" y de qué tipo es (`text`), pero no enumera sus valores — y hace bien,
 * porque la señora que vende huevos escribe "bandeja" y ninguna lista cerrada la
 * habría tenido. Por eso el formulario deja además escribir la que sea.
 *
 * El orden es el de la feria: lo que más se usa primero, para que el dedo no
 * tenga que buscar.
 */
internal val UNIDADES_SUGERIDAS: List<String> = listOf(
    "kilo",
    "unidad",
    "atado",
    "bolsa",
    "malla",
    "bandeja",
    "docena",
    "caja",
)

/** Placeholder del campo unidad libre. */
internal const val PLACEHOLDER_UNIDAD: String = "kilo, atado, bolsa…"

/** Si el pack pide unidad y la dueña no la llena, no es error. */
internal const val AYUDA_UNIDAD_OPCIONAL: String = "Opcional"

/** CTA del formulario. */
internal const val ETIQUETA_GUARDAR: String = "Guardar"

/** CTA mientras el server responde. */
internal const val ETIQUETA_GUARDANDO: String = "Guardando…"
