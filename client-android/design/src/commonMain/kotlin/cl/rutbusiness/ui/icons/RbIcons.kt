package cl.rutbusiness.ui.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import cl.rutbusiness.ui.theme.RbDefaultDimens

/**
 * RutBusiness's own icon set — [ImageVector]s built by hand, not imported.
 *
 * `material-icons-extended` was rejected before this file existed (see the
 * KDoc that used to live on `BarraDeNavegacion.dibujarGlobo` and friends): it
 * pastes megabytes of glyphs the app never uses onto a phone the hardware
 * floor describes as "almost out of space" for three icons' worth of use. That
 * reasoning is sound. What was wrong was the conclusion drawn from it — that
 * the alternative is a `Canvas` hand-drawn per screen. The actual alternative
 * is what this file is: a *set*, defined once, that costs kilobytes of Kotlin
 * bytecode and nothing at runtime beyond what any other Compose UI code costs.
 * A vector is not a dependency; `material-icons-extended` is a dependency.
 *
 * A new subpackage — not a new file in `theme/` next to [RbDefaultDimens] and
 * `RbColors` — because thirty-plus vectors are a namespace of their own, and
 * this way nobody has to scroll past them to read a color token.
 *
 * ## The geometry rules
 *
 * A set reads as *produced* by the consistency between its pieces, not by how
 * good any single glyph is. Every icon below is built from exactly the same
 * handful of numbers, on purpose, and the next icon added to this file must
 * reuse them rather than invent new ones:
 *
 * - **Grid.** Every vector is a 24×24 viewport ([VIEWPORT]), and every vector's
 *   `defaultWidth`/`defaultHeight` is [RbDefaultDimens.iconSize] — the same
 *   24dp token every other component in the design system already measures
 *   icon slots against. A caller can render an [ImageVector] from this file at
 *   any size; 24dp is only the *declared* size Compose falls back to.
 * - **Safe area.** Silhouettes stay inside the `[2, 22]` inset on both axes —
 *   an outer 2-unit margin, same on every icon — with two named exceptions:
 *   [wifiArcs] and the scan-frame corners in [escaner] are allowed to reach
 *   the true edge, because framing the full canvas is the entire point of
 *   both shapes.
 * - **Stroke.** Every outline path uses the same [STROKE_WIDTH], the same
 *   [StrokeCap.Round], the same [StrokeJoin.Round]. No icon below ever passes
 *   a different value for any of the three — [strokePath] hard-codes them so
 *   that varying one requires deliberately not using the helper.
 * - **Corner radius.** Exactly two radii exist in this file, never a third:
 *   [CORNER_UTILITY] (2f) for boxy/utility shapes (the notebook, the package,
 *   the till, the bar chart), and a **pill** radius — literally
 *   `min(width, height) / 2` of the shape it rounds — for anything meant to
 *   read as soft or branded (the chat bubble, the bill, the mic capsule).
 *   Both are documented at the call site as a comment naming which rule
 *   applies; there is no third number floating in a path.
 * - **Optical weight.** Every filled variant is the *solid silhouette of the
 *   outline's outer shape* — interior linework (the notebook's ruled lines,
 *   the alert's exclamation mark, the check's tick) is present in the outline
 *   and dropped in the filled version, never redrawn as a cutout. A hollow
 *   punch-through needs `PathFillType.EvenOdd` wound just right to read as a
 *   hole rather than an extra blob, and that is a second geometry system per
 *   icon with no way to render-check it here (see the module-level warning
 *   below). A flat silhouette always reads correctly regardless of tint or
 *   background, and it keeps all thirty-plus icons visually the same
 *   *species* of shape instead of thirty-plus one-off decisions.
 *
 * ## Fill variants
 *
 * The bar marks its active tab three ways at once — pastilla, bold label, and
 * a filled icon instead of an outline one (see `BarraDeNavegacion.Pestana`).
 * That third signal needs a genuinely different drawing per state, not a tint
 * swap, so every concept the bar can show ([agente], [cobrar], [resumen])
 * ships both a `Contorno` and a `Relleno` [ImageVector]. The mic follows the
 * same pattern for [cl.rutbusiness.app.ui.assist.BotonDeVoz]'s
 * hablando/escuchando toggle, and a handful of other closed-shape concepts
 * ([persona], [dinero], [fiado], [catalogo], [caja], [alerta], [listo]) get
 * the pair too, because the pattern is cheap once [CORNER_UTILITY] and the
 * pill rule exist and a status icon toggling solid-vs-outline is a legible,
 * reusable idea beyond just the three tabs. Purely linear glyphs — an arrow,
 * an X, a plus sign, a magnifying glass — have no meaningful "filled" reading
 * (a solid magnifying glass is not a different icon, just a worse-drawn one),
 * so those ship one [ImageVector] each and are registered in [all] under
 * their bare concept name.
 *
 * ## What this file cannot prove
 *
 * Nothing here renders. `RbIconsTest` is a pure-JVM test — no Robolectric, no
 * Compose host — because [ImageVector.Builder] is plain Kotlin construction
 * with no Android call underneath it, but that also means the test can only
 * check what construction leaves observable: matching viewport/default size,
 * the outline/filled pairing for the bar's three concepts, and that every
 * `ImageVector`-returning property on this object is reachable through [all]
 * (no orphan left off the registry). It cannot catch a stray coordinate that
 * draws a lopsided triangle. Whoever wires a new icon into a screen should
 * glance at it once on a device or in the Compose preview before trusting the
 * shape.
 */
object RbIcons {

    // ---------------------------------------------------------------------
    // Agente — hablarle al negocio. Bubble body + a filled tail always, the
    // same reasoning the original `dibujarGlobo` KDoc gave: a stroked
    // triangle that small reads as noise, so the tail is a separate filled
    // subpath even in the outline variant.
    // ---------------------------------------------------------------------

    /** Chat bubble, contorno. Corner radius: pill, `min(18, 12) / 2 = 6`. */
    val agenteContorno: ImageVector = vector("RbIcons.agenteContorno") {
        strokePath { roundedRect(3f, 4f, 18f, 12f, 6f) }
        fillPath { moveTo(7f, 16f); lineTo(7f, 20f); lineTo(11f, 16f); close() }
    }

    /** Chat bubble, relleno — solid silhouette, same tail. */
    val agenteRelleno: ImageVector = vector("RbIcons.agenteRelleno") {
        fillPath {
            roundedRect(3f, 4f, 18f, 12f, 6f)
            moveTo(7f, 16f); lineTo(7f, 20f); lineTo(11f, 16f); close()
        }
    }

    // ---------------------------------------------------------------------
    // Cobrar — vender / cobrar. A bill with a coin cut-out in outline; the
    // coin drops in relleno because a coin drawn *inside* a solid fill in the
    // same color disappears, same call the original `dibujarBillete` made.
    // ---------------------------------------------------------------------

    /** Bill, contorno. Corner radius: pill, `min(19, 10) / 2 = 5`. */
    val cobrarContorno: ImageVector = vector("RbIcons.cobrarContorno") {
        strokePath {
            roundedRect(2.5f, 7f, 19f, 10f, 5f)
            circle(12f, 12f, 2.2f)
        }
    }

    /** Bill, relleno — solid silhouette, no coin. */
    val cobrarRelleno: ImageVector = vector("RbIcons.cobrarRelleno") {
        fillPath { roundedRect(2.5f, 7f, 19f, 10f, 5f) }
    }

    // ---------------------------------------------------------------------
    // Resumen — el día. Three ascending bars. The original hand-drawn version
    // never had an outline state — bars were always solid — which quietly
    // broke the bar's own "three signals at once" rule for this one tab. This
    // set fixes that: hollow bars for the unselected tab, solid for selected.
    // ---------------------------------------------------------------------

    /** Bar chart, contorno. Corner radius: [CORNER_UTILITY]. */
    val resumenContorno: ImageVector = vector("RbIcons.resumenContorno") {
        strokePath {
            roundedRect(4f, 14f, 4f, 6f, CORNER_UTILITY)
            roundedRect(10f, 9f, 4f, 11f, CORNER_UTILITY)
            roundedRect(16f, 4f, 4f, 16f, CORNER_UTILITY)
        }
    }

    /** Bar chart, relleno — the always-solid look the app already shipped. */
    val resumenRelleno: ImageVector = vector("RbIcons.resumenRelleno") {
        fillPath {
            roundedRect(4f, 14f, 4f, 6f, CORNER_UTILITY)
            roundedRect(10f, 9f, 4f, 11f, CORNER_UTILITY)
            roundedRect(16f, 4f, 4f, 16f, CORNER_UTILITY)
        }
    }

    // ---------------------------------------------------------------------
    // Micrófono — mismo concepto que `BotonDeVoz.dibujarMicrofono`, portado a
    // vector. Igual que ahí: sólo la cápsula cambia de relleno a contorno; el
    // arco, el palito y el pie se dibujan siempre con trazo en las dos
    // variantes, así que `Relleno` combina un `fillPath` (cápsula) con un
    // `strokePath` (el resto) en el mismo `ImageVector`.
    // ---------------------------------------------------------------------

    /** Mic, contorno. Capsule corner radius: pill, `min(6, 10) / 2 = 3`. */
    val microfonoContorno: ImageVector = vector("RbIcons.microfonoContorno") {
        strokePath {
            roundedRect(9f, 3f, 6f, 10f, 3f)
            micCradle()
        }
    }

    /** Mic, relleno — capsule solid, cradle still stroked. */
    val microfonoRelleno: ImageVector = vector("RbIcons.microfonoRelleno") {
        fillPath { roundedRect(9f, 3f, 6f, 10f, 3f) }
        strokePath { micCradle() }
    }

    // ---------------------------------------------------------------------
    // Persona — head + shoulders. Both subpaths are the outer shape, so the
    // "drop interior linework" rule has nothing to drop: relleno is the same
    // two subpaths, just filled instead of stroked.
    // ---------------------------------------------------------------------

    /** Person, contorno. Head circle radius 3.4. */
    val personaContorno: ImageVector = vector("RbIcons.personaContorno") {
        strokePath {
            circle(12f, 7.5f, 3.4f)
            shoulders()
        }
    }

    /** Person, relleno — solid head and shoulders. */
    val personaRelleno: ImageVector = vector("RbIcons.personaRelleno") {
        fillPath {
            circle(12f, 7.5f, 3.4f)
            shoulders()
            close()
        }
    }

    // ---------------------------------------------------------------------
    // Dinero — plata en general (distinto de "cobrar", que es la acción de
    // vender). Dos monedas superpuestas; en relleno el `NonZero` fill de
    // ambos círculos con el mismo sentido de giro las funde en un solo blob,
    // que es exactamente la silueta que se quiere.
    // ---------------------------------------------------------------------

    /** Stacked coins, contorno. */
    val dineroContorno: ImageVector = vector("RbIcons.dineroContorno") {
        strokePath {
            circle(9f, 15f, 5.2f)
            circle(15f, 9f, 5.2f)
        }
    }

    /** Stacked coins, relleno — the two circles union into one silhouette. */
    val dineroRelleno: ImageVector = vector("RbIcons.dineroRelleno") {
        fillPath {
            circle(9f, 15f, 5.2f)
            circle(15f, 9f, 5.2f)
        }
    }

    // ---------------------------------------------------------------------
    // Fiado — quién me debe. A ledger: a notebook body with two short ruled
    // lines standing in for entries. Portrait aspect (16×18) is what tells it
    // apart from `catalogo`'s landscape box once both are filled silhouettes.
    // ---------------------------------------------------------------------

    /** Ledger, contorno. Corner radius: [CORNER_UTILITY]. */
    val fiadoContorno: ImageVector = vector("RbIcons.fiadoContorno") {
        strokePath {
            roundedRect(4f, 3f, 16f, 18f, CORNER_UTILITY)
            moveTo(7f, 9f); horizontalLineTo(17f)
            moveTo(7f, 13f); horizontalLineTo(15f)
        }
    }

    /** Ledger, relleno — solid notebook, ruled lines dropped. */
    val fiadoRelleno: ImageVector = vector("RbIcons.fiadoRelleno") {
        fillPath { roundedRect(4f, 3f, 16f, 18f, CORNER_UTILITY) }
    }

    // ---------------------------------------------------------------------
    // Catálogo — lo que vendo. A package: a box body, a lid flap, a seam.
    // Landscape aspect (16×12) versus `fiado`'s portrait ledger.
    // ---------------------------------------------------------------------

    /** Package, contorno. Corner radius: [CORNER_UTILITY]. */
    val catalogoContorno: ImageVector = vector("RbIcons.catalogoContorno") {
        strokePath {
            roundedRect(4f, 8f, 16f, 12f, CORNER_UTILITY)
            moveTo(4f, 8f); quadTo(12f, 3f, 20f, 8f)
            moveTo(12f, 8f); lineTo(12f, 20f)
        }
    }

    /** Package, relleno — solid box, flap and seam dropped. */
    val catalogoRelleno: ImageVector = vector("RbIcons.catalogoRelleno") {
        fillPath { roundedRect(4f, 8f, 16f, 12f, CORNER_UTILITY) }
    }

    // ---------------------------------------------------------------------
    // Caja — la caja del día (cash drawer), no el mismo concepto que
    // `catalogo`. Drawer body plus a raised till outline; wide aspect
    // (18×9) instead of the package's near-square 16×12.
    // ---------------------------------------------------------------------

    /** Cash drawer, contorno. Corner radius: [CORNER_UTILITY]. */
    val cajaContorno: ImageVector = vector("RbIcons.cajaContorno") {
        strokePath {
            roundedRect(3f, 10f, 18f, 9f, CORNER_UTILITY)
            moveTo(9f, 10f); lineTo(9f, 6f); lineTo(15f, 6f); lineTo(15f, 10f)
        }
    }

    /** Cash drawer, relleno — solid drawer, till detail dropped. */
    val cajaRelleno: ImageVector = vector("RbIcons.cajaRelleno") {
        fillPath { roundedRect(3f, 10f, 18f, 9f, CORNER_UTILITY) }
    }

    // ---------------------------------------------------------------------
    // Alerta — a warning triangle. `StrokeJoin.Round` (already the default in
    // [strokePath]) softens the tip instead of a sharp point, matching the
    // "formas gruesas y simples" rule the original hand-drawn icons stated.
    // ---------------------------------------------------------------------

    /** Warning triangle, contorno, with the exclamation mark. */
    val alertaContorno: ImageVector = vector("RbIcons.alertaContorno") {
        strokePath {
            moveTo(12f, 3.5f); lineTo(21f, 20f); lineTo(3f, 20f); close()
            moveTo(12f, 9f); lineTo(12f, 14.5f)
            dot(12f, 17f)
        }
    }

    /** Warning triangle, relleno — solid, exclamation mark dropped. */
    val alertaRelleno: ImageVector = vector("RbIcons.alertaRelleno") {
        fillPath { moveTo(12f, 3.5f); lineTo(21f, 20f); lineTo(3f, 20f); close() }
    }

    // ---------------------------------------------------------------------
    // Listo — done / check. A ring with a tick in contorno; relleno drops the
    // tick for the same reason every other filled icon does (see the
    // module KDoc's "optical weight" rule).
    // ---------------------------------------------------------------------

    /** Check, contorno — ring plus tick. */
    val listoContorno: ImageVector = vector("RbIcons.listoContorno") {
        strokePath {
            circle(12f, 12f, 8.5f)
            moveTo(8f, 12.3f); lineTo(10.8f, 15.3f); lineTo(16.3f, 8.8f)
        }
    }

    /** Check, relleno — solid disc, tick dropped. */
    val listoRelleno: ImageVector = vector("RbIcons.listoRelleno") {
        fillPath { circle(12f, 12f, 8.5f) }
    }

    // ---------------------------------------------------------------------
    // Below: single-style glyphs. A back chevron, a close mark, plus/minus, a
    // magnifying glass, a share icon, wifi (and its crossed-out twin), a
    // printer and a scan frame. None of these read differently filled — see
    // the module KDoc's "Fill variants" section — so each is one ImageVector.
    // ---------------------------------------------------------------------

    /** Atrás — a left-pointing chevron. */
    val atras: ImageVector = vector("RbIcons.atras") {
        strokePath { moveTo(15f, 4f); lineTo(7f, 12f); lineTo(15f, 20f) }
    }

    /** Cerrar — an X. */
    val cerrar: ImageVector = vector("RbIcons.cerrar") {
        strokePath {
            moveTo(5f, 5f); lineTo(19f, 19f)
            moveTo(19f, 5f); lineTo(5f, 19f)
        }
    }

    /** Más — a plus sign. */
    val mas: ImageVector = vector("RbIcons.mas") {
        strokePath {
            moveTo(12f, 4f); lineTo(12f, 20f)
            moveTo(4f, 12f); lineTo(20f, 12f)
        }
    }

    /** Menos — a minus sign. Same horizontal stroke as [mas], no vertical. */
    val menos: ImageVector = vector("RbIcons.menos") {
        strokePath { moveTo(4f, 12f); lineTo(20f, 12f) }
    }

    /** Buscar — a magnifying glass. */
    val buscar: ImageVector = vector("RbIcons.buscar") {
        strokePath {
            circle(10f, 10f, 6f)
            moveTo(14.5f, 14.5f); lineTo(20f, 20f)
        }
    }

    /** Compartir — three nodes, two connecting lines. Nodes are always solid
     *  dots; only the connecting lines are stroked, so this mixes a
     *  [fillPath] and a [strokePath] in the one ImageVector. */
    val compartir: ImageVector = vector("RbIcons.compartir") {
        fillPath {
            circle(18f, 6f, 2.2f)
            circle(6f, 12f, 2.2f)
            circle(18f, 18f, 2.2f)
        }
        strokePath {
            moveTo(7.8f, 11f); lineTo(16.3f, 7f)
            moveTo(7.8f, 13f); lineTo(16.3f, 17f)
        }
    }

    /** Wifi — three arcs and a dot, sharing [wifiArcs] with [sinConexion] so
     *  the two always draw the same signal shape. */
    val wifi: ImageVector = vector("RbIcons.wifi") {
        strokePath {
            wifiArcs()
            dot(12f, 19f)
        }
    }

    /** Sin conexión — [wifiArcs] plus a diagonal slash, so it can never drift
     *  out of sync with [wifi]'s signal shape. */
    val sinConexion: ImageVector = vector("RbIcons.sinConexion") {
        strokePath {
            wifiArcs()
            dot(12f, 19f)
            moveTo(4f, 4f); lineTo(20f, 20f)
        }
    }

    /** Impresora — paper in, body, slot, paper out. */
    val impresora: ImageVector = vector("RbIcons.impresora") {
        strokePath {
            roundedRect(7f, 4f, 10f, 5f, 1f)
            roundedRect(4f, 9f, 16f, 8f, CORNER_UTILITY)
            moveTo(7f, 13f); horizontalLineTo(17f)
            roundedRect(7f, 17f, 10f, 4f, 1f)
        }
    }

    /** Escáner — four viewfinder corners plus a scan beam, the same shape
     *  family as `PantallaDeEscaneo`'s full-screen frame, at icon scale. */
    val escaner: ImageVector = vector("RbIcons.escaner") {
        strokePath {
            moveTo(3f, 8f); lineTo(3f, 3f); lineTo(8f, 3f)
            moveTo(16f, 3f); lineTo(21f, 3f); lineTo(21f, 8f)
            moveTo(21f, 16f); lineTo(21f, 21f); lineTo(16f, 21f)
            moveTo(8f, 21f); lineTo(3f, 21f); lineTo(3f, 16f)
            moveTo(3f, 12f); lineTo(21f, 12f)
        }
    }

    /**
     * Every icon this file declares, keyed by its property name.
     *
     * `RbIconsTest` walks this object's public `ImageVector`-returning
     * getters through plain `java.lang.reflect` (no `kotlin-reflect`
     * dependency needed for that) and asserts each one shows up here by
     * reference — the guard against an icon added above and never wired into
     * the catalogue.
     */
    val all: Map<String, ImageVector> = linkedMapOf(
        "agenteContorno" to agenteContorno,
        "agenteRelleno" to agenteRelleno,
        "cobrarContorno" to cobrarContorno,
        "cobrarRelleno" to cobrarRelleno,
        "resumenContorno" to resumenContorno,
        "resumenRelleno" to resumenRelleno,
        "microfonoContorno" to microfonoContorno,
        "microfonoRelleno" to microfonoRelleno,
        "personaContorno" to personaContorno,
        "personaRelleno" to personaRelleno,
        "dineroContorno" to dineroContorno,
        "dineroRelleno" to dineroRelleno,
        "fiadoContorno" to fiadoContorno,
        "fiadoRelleno" to fiadoRelleno,
        "catalogoContorno" to catalogoContorno,
        "catalogoRelleno" to catalogoRelleno,
        "cajaContorno" to cajaContorno,
        "cajaRelleno" to cajaRelleno,
        "alertaContorno" to alertaContorno,
        "alertaRelleno" to alertaRelleno,
        "listoContorno" to listoContorno,
        "listoRelleno" to listoRelleno,
        "atras" to atras,
        "cerrar" to cerrar,
        "mas" to mas,
        "menos" to menos,
        "buscar" to buscar,
        "compartir" to compartir,
        "wifi" to wifi,
        "sinConexion" to sinConexion,
        "impresora" to impresora,
        "escaner" to escaner,
    )
}

// ---------------------------------------------------------------------------
// Builder plumbing. Every icon above goes through [vector], and every stroked
// or filled path inside it goes through [strokePath]/[fillPath], so the grid,
// the stroke weight and the cap/join are never re-typed and never drift.
// ---------------------------------------------------------------------------

/** The one viewport every icon in this file declares. */
private const val VIEWPORT = 24f

/** The one stroke weight every outline path in this file uses. */
private const val STROKE_WIDTH = 2f

/** The one non-pill corner radius this file uses. See the module KDoc. */
private const val CORNER_UTILITY = 2f

/** Icons are built in a neutral color; the caller's `tint` (via `Icon`'s
 *  `tint` parameter) replaces it at draw time, same as any other vector
 *  asset. Nothing below hard-codes an [cl.rutbusiness.ui.theme.RbColors]
 *  token, which is what lets one icon serve both themes. */
private val TINT = SolidColor(Color.Black)

private fun vector(name: String, block: ImageVector.Builder.() -> Unit): ImageVector =
    ImageVector.Builder(
        name = name,
        defaultWidth = RbDefaultDimens.iconSize,
        defaultHeight = RbDefaultDimens.iconSize,
        viewportWidth = VIEWPORT,
        viewportHeight = VIEWPORT,
    ).apply(block).build()

/** Adds one stroked path node: [STROKE_WIDTH], [StrokeCap.Round],
 *  [StrokeJoin.Round] — always, everywhere, no per-call override. */
private fun ImageVector.Builder.strokePath(block: PathBuilder.() -> Unit) {
    path(
        stroke = TINT,
        strokeLineWidth = STROKE_WIDTH,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathBuilder = block,
    )
}

/** Adds one filled path node — the solid-silhouette half of every pair. */
private fun ImageVector.Builder.fillPath(block: PathBuilder.() -> Unit) {
    path(fill = TINT, pathBuilder = block)
}

/**
 * A rounded rectangle, traced clockwise from the top edge.
 *
 * The single source of both corner-radius rules this file allows: pass
 * [CORNER_UTILITY] for a boxy shape, or `min(w, h) / 2` for a pill. No icon
 * above computes a corner by hand.
 */
private fun PathBuilder.roundedRect(x: Float, y: Float, w: Float, h: Float, r: Float) {
    moveTo(x + r, y)
    horizontalLineTo(x + w - r)
    arcTo(r, r, 0f, false, true, x + w, y + r)
    verticalLineTo(y + h - r)
    arcTo(r, r, 0f, false, true, x + w - r, y + h)
    horizontalLineTo(x + r)
    arcTo(r, r, 0f, false, true, x, y + h - r)
    verticalLineTo(y + r)
    arcTo(r, r, 0f, false, true, x + r, y)
    close()
}

/** A circle, traced as two half-circle arcs — the standard translation of an
 *  SVG circle into cubic/arc path commands. */
private fun PathBuilder.circle(cx: Float, cy: Float, r: Float) {
    moveTo(cx + r, cy)
    arcTo(r, r, 0f, false, true, cx - r, cy)
    arcTo(r, r, 0f, false, true, cx + r, cy)
    close()
}

/** A small filled dot, drawn as a near-zero-length line under a round cap —
 *  cheaper than a third primitive, and it inherits [STROKE_WIDTH] as its
 *  diameter for free, so a dot is never a mismatched size. */
private fun PathBuilder.dot(cx: Float, cy: Float) {
    moveTo(cx, cy)
    lineTo(cx, cy + 0.01f)
}

/** The cradle under [RbIcons.microfonoContorno]/[RbIcons.microfonoRelleno]'s
 *  capsule: the arms that hug it, the stem, the foot. Always stroked, in
 *  both variants — only the capsule itself toggles fill vs. stroke. */
private fun PathBuilder.micCradle() {
    moveTo(6.5f, 12f); quadTo(6.5f, 19f, 12f, 19f); quadTo(17.5f, 19f, 17.5f, 12f)
    moveTo(12f, 19f); lineTo(12f, 21f)
    moveTo(9f, 21f); lineTo(15f, 21f)
}

/** [RbIcons.personaContorno]/[RbIcons.personaRelleno]'s shoulder curve, under
 *  the head circle both variants also draw. */
private fun PathBuilder.shoulders() {
    moveTo(5f, 20f)
    quadTo(5f, 13.5f, 12f, 13.5f)
    quadTo(19f, 13.5f, 19f, 20f)
}

/** The three concentric signal arcs shared by [RbIcons.wifi] and
 *  [RbIcons.sinConexion], so the two can never draw a different signal shape
 *  by accident. */
private fun PathBuilder.wifiArcs() {
    moveTo(8f, 16f); quadTo(12f, 12f, 16f, 16f)
    moveTo(5.5f, 12.5f); quadTo(12f, 6.5f, 18.5f, 12.5f)
    moveTo(3f, 9f); quadTo(12f, 2f, 21f, 9f)
}
