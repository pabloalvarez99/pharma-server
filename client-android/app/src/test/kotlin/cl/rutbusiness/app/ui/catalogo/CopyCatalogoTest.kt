package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.rubro.AttrField
import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
import cl.rutbusiness.core.rubro.PACK_OTRO
import cl.rutbusiness.core.rubro.RubroVocab
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Que "Lo que vendo" hable el idioma del rubro y no el de la app.
 *
 * El pack ya trae `vocab` y `attrs` desde el server (ADR-0022,
 * `domain::rubro`), así que una constante "Producto" escrita a mano en la
 * pantalla sería una segunda fuente de verdad — de las que se quedan viejas sin
 * que nadie se entere hasta que un rubro nuevo muestra la palabra del rubro
 * anterior.
 */
class CopyCatalogoTest {

    // --- feria ----------------------------------------------------------------

    @Test
    fun `en feria la pantalla se llama como la duena la llamaria`() {
        val copy = copyCatalogo(PACK_FERIA)

        assertEquals("Lo que vendo", copy.titulo)
        assertEquals("Cosa", copy.item)
        assertEquals("Se vende por", copy.etiquetaUnidad)
        assertTrue("feria declara el campo de unidad", copy.hayUnidad)
        assertTrue(copy.esFeria)
    }

    /**
     * El chrome de la lista se lee como mesa del puesto, no como inventario.
     *
     * Subtítulo, buscador y chip de confirmación salían hardcodeados en
     * [CatalogoScreen] con voz de farmacia; en feria tienen que hablar del
     * día y de "cosa".
     */
    @Test
    fun `en feria el chrome de la lista es de mesa`() {
        val copy = copyCatalogo(PACK_FERIA)

        assertEquals("Lo que se vende hoy", copy.subtitulo)
        assertEquals("Buscar cosa", copy.etiquetaBuscar)
        assertEquals("Tomate, cilantro, palta…", copy.placeholderBuscar)
        assertEquals("Con el precio ya lo cobras", copy.subtituloAlta)
        assertEquals("Con el nombre y el precio alcanza.", copy.pieFormulario)
        assertFalse(
            "feria no nombra código de barras en el form: era \"${copy.pieFormulario}\"",
            copy.pieFormulario.contains("código de barras"),
        )
        assertEquals("Listo: tomates", copyConfirmacionGuardado("tomates", feria = true))
    }

    /**
     * "Agregar **una** cosa", no "un cosa".
     *
     * Es la clase de detalle que hace que la app suene escrita por alguien y no
     * generada, y sale del género de la palabra del pack, que la app no elige.
     */
    @Test
    fun `el articulo concuerda con la palabra del pack`() {
        assertEquals("Agregar una cosa", copyCatalogo(PACK_FERIA).agregar)
        assertEquals("Cosa nueva", copyCatalogo(PACK_FERIA).tituloAlta)

        assertEquals("Agregar un producto", copyCatalogo(PACK_OTRO).agregar)
        assertEquals("Producto nuevo", copyCatalogo(PACK_OTRO).tituloAlta)
    }

    // --- otros rubros ---------------------------------------------------------

    @Test
    fun `en farmacia dice inventario y no ofrece unidad`() {
        val copy = copyCatalogo(PACK_FARMACIA)

        assertEquals("Inventario", copy.titulo)
        assertEquals("Producto", copy.item)
        assertFalse(
            "farmacia no declara `unidad`: el formulario no puede inventarle un campo",
            copy.hayUnidad,
        )
        assertFalse(copy.esFeria)
    }

    /** Retail formal conserva la voz de cobro / inventario, no la de mesa. */
    @Test
    fun `en farmacia el chrome sigue siendo de inventario`() {
        val copy = copyCatalogo(PACK_FARMACIA)

        assertEquals("Lo que puedes cobrar", copy.subtitulo)
        assertEquals("Buscar producto", copy.etiquetaBuscar)
        assertEquals("Nombre", copy.placeholderBuscar)
        assertEquals("Con esto ya lo puedes cobrar", copy.subtituloAlta)
        assertTrue(copy.pieFormulario.contains("código de barras"))
        assertEquals(
            "Guardado: Paracetamol",
            copyConfirmacionGuardado("Paracetamol", feria = false),
        )
    }

    @Test
    fun `helpers de lista no inventan plata ni jerga`() {
        assertEquals("Por kilo", copyComoSeVende("kilo"))
        assertEquals("Nada con \"tomate\"", copyVacioBusqueda("  tomate  "))
        assertEquals("Cargando lo que vendo", copyCargandoLista("Lo que vendo"))
        assertEquals("Cargando inventario", copyCargandoLista("Inventario"))
    }

    /**
     * El campo de unidad se busca por **clave**, nunca por posición.
     *
     * `attrs[0]` daría lo mismo hoy para feria y devolvería "Precio de
     * referencia" el día que el server agregue un atributo antes — y la dueña
     * vería el precio pedido dos veces, una de ellas como texto libre.
     */
    @Test
    fun `la unidad se busca por clave y no por posicion`() {
        val alReves = PACK_FERIA.copy(attrs = PACK_FERIA.attrs.reversed())
        assertEquals("Se vende por", copyCatalogo(alReves).etiquetaUnidad)
        assertEquals("unidad", copyCatalogo(alReves).claveUnidad)

        val sinUnidad = PACK_FERIA.copy(
            attrs = listOf(AttrField(key = "precio_ref", label = "Precio de referencia")),
        )
        assertNull(campoDeUnidad(sinUnidad))
        assertFalse(copyCatalogo(sinUnidad).hayUnidad)
    }

    /**
     * Un pack sin vocabulario no deja la pantalla sin título.
     *
     * El server manda `vocab` siempre, pero un server viejo o un pack a medio
     * escribir llegarían con las cadenas vacías, y "" arriba de la pantalla es
     * peor que una palabra genérica.
     */
    @Test
    fun `un pack sin vocabulario cae en palabras genericas`() {
        val mudo = PACK_FERIA.copy(vocab = RubroVocab(item = "  ", catalog = ""))
        val copy = copyCatalogo(mudo)

        assertEquals("Inventario", copy.titulo)
        assertEquals("Producto", copy.item)
    }

    /**
     * En feria el vacío enseña dos caminos day-1: cargar con precio, o venderle
     * al agente. No le habla de código de barras (en el puesto no hay).
     */
    @Test
    fun `en feria el vacio ensena venderle al agente`() {
        val copy = copyCatalogo(PACK_FERIA)

        assertTrue(
            "pista feria debe enseñar la frase al agente: era \"${copy.vacioPista}\"",
            copy.vacioPista.contains("vendí tomates a 2000"),
        )
        assertFalse(
            "feria no pide código de barras en el vacío: era \"${copy.vacioPista}\"",
            copy.vacioPista.contains("código de barras"),
        )
    }

    /** agentHome sin rubro=feria también usa el vacío de feria. */
    @Test
    fun `agentHome usa el vacio de feria aunque el rubro no diga feria`() {
        val pack = PACK_OTRO.copy(features = PACK_OTRO.features.copy(agentHome = true))
        val copy = copyCatalogo(pack)

        assertTrue(copy.vacioPista.contains("vendí tomates a 2000"))
        assertFalse(copy.vacioPista.contains("código de barras"))
    }

    /** Farmacia y formal siguen con la pista genérica del form. */
    @Test
    fun `en farmacia el vacio sigue con codigo de barras`() {
        val pista =
            "Agrega lo que vendes con su precio y ya puedes cobrarlo. " +
                "No hace falta código de barras ni nada más."
        assertEquals(pista, copyCatalogo(PACK_FARMACIA).vacioPista)
        assertEquals(pista, copyCatalogo(PACK_OTRO).vacioPista)
    }
}
