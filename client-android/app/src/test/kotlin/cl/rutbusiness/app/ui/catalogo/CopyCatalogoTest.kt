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

    /** El vacío enseña el próximo paso en vez de informar que está vacío. */
    @Test
    fun `el vacio dice que hacer y no que no hay nada`() {
        val copy = copyCatalogo(PACK_FERIA)

        assertTrue(
            "la pista tiene que decir qué hacer: era \"${copy.vacioPista}\"",
            copy.vacioPista.contains("Agrega"),
        )
        assertTrue(
            "y tiene que decir que el código de barras no hace falta, que es lo que " +
                "la señora del puesto está por preguntar",
            copy.vacioPista.contains("código de barras"),
        )
    }
}
