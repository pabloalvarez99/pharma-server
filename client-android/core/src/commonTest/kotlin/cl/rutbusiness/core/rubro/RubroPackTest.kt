package cl.rutbusiness.core.rubro

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RubroPackTest {

    @Test
    fun `feria es agent-first sin barcode ni printer ni dte`() {
        val f = PACK_FERIA.features
        assertTrue(f.agentHome)
        assertTrue(f.informalOk)
        assertFalse(f.barcode)
        assertFalse(f.printer)
        assertFalse(f.dte)
        assertEquals("Cosa", PACK_FERIA.vocab.item)
        assertEquals("Lo que vendo", PACK_FERIA.vocab.catalog)
    }

    @Test
    fun `farmacia conserva superficie formal`() {
        val f = PACK_FARMACIA.features
        assertFalse(f.agentHome)
        assertTrue(f.barcode)
        assertTrue(f.printer)
        assertTrue(f.dte)
        assertTrue(f.recetas)
        assertTrue(f.clinical)
    }

    @Test
    fun `pack local desconocido cae a otro nunca farmacia`() {
        assertEquals("otro", PacksLocales.de(null).rubro)
        assertEquals("otro", PacksLocales.de("").rubro)
        assertEquals("otro", PacksLocales.de("ferreteria").rubro)
        assertEquals("feria", PacksLocales.de("FERIA").rubro)
        assertEquals("farmacia", PacksLocales.de("farmacia").rubro)
    }

    @Test
    fun `repositorio arranca en otro y acepta forzar feria`() {
        val repo = RubroPackRepository()
        assertEquals("otro", repo.actual.rubro)
        repo.usarLocal("feria")
        assertTrue(repo.actual.features.agentHome)
        assertFalse(repo.actual.features.barcode)
        repo.limpiar()
        assertEquals("otro", repo.actual.rubro)
    }
}
