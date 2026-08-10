package cl.rutbusiness.core.rubro

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Pack de rubro que manda el server (`GET /api/v1/rubro-pack`).
 *
 * Espejo de `domain::rubro::RubroPack` (ADR-0022). La UI gatea con
 * [features]; el fallback local vive en [PacksLocales] para cuando no hay
 * LAN o el server es viejo.
 *
 * `ignoreUnknownKeys` en el Json de [cl.rutbusiness.core.net.ApiFactory]
 * cubre campos nuevos del server sin tumbar apps viejas.
 */
@Serializable
data class RubroPack(
    val rubro: String,
    val label: String = "",
    val tagline: String = "",
    val accent: String = "#8b97ad",
    val features: RubroFeatures = RubroFeatures(),
    val vocab: RubroVocab = RubroVocab(),
    val attrs: List<AttrField> = emptyList(),
    @SerialName("seed_vertical") val seedVertical: String? = null,
    @SerialName("coming_soon") val comingSoon: List<String> = emptyList(),
)

@Serializable
data class RubroFeatures(
    val recetas: Boolean = false,
    val lotes: Boolean = false,
    @SerialName("physical_stock") val physicalStock: Boolean = true,
    val clinical: Boolean = false,
    /** Agente = home (feria). */
    @SerialName("agent_home") val agentHome: Boolean = false,
    /** Cámara / barcode en Cobrar. Off en feria. */
    val barcode: Boolean = true,
    /** Impresora térmica. Off en feria. */
    val printer: Boolean = true,
    /** DTE / SII. Off en feria informal. */
    val dte: Boolean = true,
    /** Alta sin RUT empresa day-1. */
    @SerialName("informal_ok") val informalOk: Boolean = false,
)

@Serializable
data class RubroVocab(
    val item: String = "Producto",
    val catalog: String = "Inventario",
)

@Serializable
data class AttrField(
    val key: String,
    val label: String,
    val kind: String = "text",
)

/** Pack genérico cuando no hay vertical ni red. Nunca default a farmacia. */
val PACK_OTRO: RubroPack = RubroPack(
    rubro = "otro",
    label = "Otro",
    tagline = "Tu negocio, a tu manera.",
    accent = "#8b97ad",
    features = RubroFeatures(
        agentHome = false,
        barcode = true,
        printer = true,
        dte = true,
        informalOk = false,
        physicalStock = true,
    ),
)

/** Pack feria embebido: offline-first y tests sin server. */
val PACK_FERIA: RubroPack = RubroPack(
    rubro = "feria",
    label = "Feria / Calle",
    tagline = "Tu puesto, sin cuaderno.",
    accent = "#f59e0b",
    features = RubroFeatures(
        recetas = false,
        lotes = false,
        physicalStock = true,
        clinical = false,
        agentHome = true,
        barcode = false,
        printer = false,
        dte = false,
        informalOk = true,
    ),
    vocab = RubroVocab(item = "Cosa", catalog = "Lo que vendo"),
    attrs = listOf(
        AttrField(key = "unidad", label = "Se vende por", kind = "text"),
        AttrField(key = "precio_ref", label = "Precio de referencia", kind = "money"),
    ),
    comingSoon = listOf(
        "Venta por peso en el celular",
        "Recordatorios de quién te debe",
    ),
)

/** Pack farmacia embebido (contraste + fallback si vertical=farmacia offline). */
val PACK_FARMACIA: RubroPack = RubroPack(
    rubro = "farmacia",
    label = "Farmacia",
    tagline = "Tu farmacia, en regla.",
    accent = "#1fd3a3",
    features = RubroFeatures(
        recetas = true,
        lotes = true,
        physicalStock = true,
        clinical = true,
        agentHome = false,
        barcode = true,
        printer = true,
        dte = true,
        informalOk = false,
    ),
)

/**
 * Catálogo local mínimo para offline. No pretende duplicar los 9 packs del
 * server: si el vertical no está acá, se usa [PACK_OTRO] (nunca farmacia).
 */
object PacksLocales {
    fun de(rubro: String?): RubroPack {
        val key = rubro?.trim()?.lowercase().orEmpty()
        return when (key) {
            "feria" -> PACK_FERIA
            "farmacia" -> PACK_FARMACIA
            else -> PACK_OTRO
        }
    }
}
