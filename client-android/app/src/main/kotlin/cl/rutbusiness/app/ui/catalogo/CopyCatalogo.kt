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
    /** Una cosa vendible: "Cosa", "Producto", "Plato". */
    val item: String,
    /** Botón grande de alta: "Agregar una cosa". */
    val agregar: String,
    /** Título del formulario en alta. */
    val tituloAlta: String,
    /** Título del formulario en edición. */
    val tituloEdicion: String,
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
    /** Qué hacer, dicho en una frase. */
    val vacioPista: String,
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

    // "una cosa" / "un producto": el artículo cambia con la palabra del pack y
    // no se puede derivar de la gramática sin adivinar. Las tres formas que hoy
    // devuelven los packs son "Cosa" (f), "Producto" (m), "Plato" (m) y
    // "Servicio" (m); cualquier palabra nueva cae en el masculino, que es lo que
    // hace el castellano con lo desconocido.
    val articulo = if (item.endsWith("a", ignoreCase = true)) "una" else "un"
    val enMinuscula = item.replaceFirstChar { it.lowercase() }

    return CopyCatalogo(
        titulo = catalogo,
        item = item,
        agregar = "Agregar $articulo $enMinuscula",
        tituloAlta = "$item nuevo".let {
            if (articulo == "una") "$item nueva" else it
        },
        tituloEdicion = "Editar $enMinuscula",
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
        // El vacío enseña el próximo paso, no informa que está vacío.
        vacioPista = "Agrega lo que vendes con su precio y ya puedes cobrarlo. " +
            "No hace falta código de barras ni nada más.",
    )
}

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
