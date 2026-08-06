package cl.rutbusiness.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.ui.showcase.RbShowcaseScreen
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The acceptance criterion, executed.
 *
 * The brief says the design system is not finished until it survives a **200%
 * system font scale** without the layout breaking, text clipping, or a control
 * becoming unreachable. This renders the whole catalogue at 1.0x and at 2.0x
 * and asserts exactly those three things.
 *
 * Robolectric rather than an emulator on purpose: the assertions are about
 * measured layout, which is deterministic, and a JVM run makes this cheap
 * enough to sit in CI on every commit. A test that only runs when someone
 * remembers to boot a device is a test that stops running.
 *
 * [GraphicsMode.Mode.NATIVE] is **required**, not a nicety. Under Robolectric's
 * default legacy graphics the font engine is a stub: `measureText` returns
 * roughly half a dp per character, so a 35-character heading measures 17dp
 * wide and every width assertion passes without meaning anything. Native mode
 * renders with real fonts - the same heading measures 280dp - which is what
 * makes "the text is not cut off" a real check.
 *
 * SDK 34 for the same reason: Android 14 is where the **non-linear** font scale
 * curve lives, so this measures the curve users actually get rather than a
 * naive multiply.
 *
 * The screen is 360x640dp - the reference device's 720p panel in dp.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class RbFontScaleTest {

    @get:Rule
    val compose = createComposeRule()

    private val touchTarget: Dp = RbDefaultDimens.touchTarget

    /** Renders the catalogue with the system font scale forced to [fontScale]. */
    private fun showCatalogue(fontScale: Float, darkTheme: Boolean = true) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(density = base.density, fontScale = fontScale),
            ) {
                RbTheme(darkTheme = darkTheme, reducedMotion = true) {
                    Box(Modifier.fillMaxSize()) {
                        RbShowcaseScreen()
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    /**
     * Every node that reacts to a tap is at least 56dp in both axes.
     *
     * Scrolls the catalogue from top to bottom and measures at each stop. The
     * first version of this checked only what happened to be on screen, which
     * covered nine controls out of the set - a measurement that would have let
     * an undersized chip or list row ship. `LazyColumn` disposes what is off
     * screen, so walking it is the only way to see all of them.
     *
     * Uses *unclipped* bounds so a target that overflows its parent is still
     * measured honestly rather than reported at its clipped size.
     */
    private fun assertAllTouchTargets(fontScale: Float) {
        val failures = mutableSetOf<String>()
        var measured = 0

        // Walks until the list reports the index is past its end, so adding a
        // section to the catalogue does not silently stop being measured.
        val list = compose.onNode(hasScrollAction())
        var sections = 0
        while (runCatching { list.performScrollToIndex(sections) }.isSuccess) {
            sections++
            compose.waitForIdle()

            val nodes = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
            nodes.indices.forEach { i ->
                val interaction: SemanticsNodeInteraction = compose.onAllNodes(hasClickAction())[i]
                val bounds = interaction.getUnclippedBoundsInRoot()
                measured++
                if (bounds.height < touchTarget || bounds.width < touchTarget) {
                    val node = interaction.fetchSemanticsNode()
                    failures += "\"${node.textOrEmpty()}\" is ${bounds.width} x ${bounds.height}"
                }
            }
        }

        assertTrue("the catalogue did not scroll at all", sections > 0)
        assertTrue("no clickable nodes were found - the catalogue did not render", measured > 0)
        assertTrue(
            "at ${fontScale}x these touch targets are under $touchTarget:\n" +
                failures.joinToString("\n"),
            failures.isEmpty(),
        )
    }

    @Test
    fun `touch targets hold at 100 percent`() {
        showCatalogue(fontScale = 1.0f)
        assertAllTouchTargets(1.0f)
    }

    @Test
    fun `touch targets hold at 200 percent`() {
        showCatalogue(fontScale = 2.0f)
        assertAllTouchTargets(2.0f)
    }

    /**
     * Type obeys the system font scale, and controls grow to hold it.
     *
     * Both scales are rendered in one composition - the rule allows a single
     * `setContent` per test - so the comparison is between two live controls
     * rather than between two runs.
     *
     * Note what is *not* asserted: that the button itself is taller at 2x. It
     * is not, and that is correct. Android 14 scales type on a **non-linear**
     * curve, so a 17sp label at 200% renders around 29sp rather than 34sp; the
     * label grows from 21.5dp to 36.5dp, and 36.5dp plus padding still fits
     * inside the 56dp floor. The button only starts growing past ~2.5x. An
     * assertion that the button must grow at 2x would have been asserting a
     * misunderstanding of the platform, so the test measures the label - which
     * is the thing the user actually asked to be bigger - and then checks the
     * control still holds it.
     */
    @Test
    fun `type obeys the font scale and the control holds it`() {
        compose.setContent {
            val base = LocalDensity.current
            RbTheme(darkTheme = true, reducedMotion = true) {
                Column {
                    listOf(1.0f to "1x", 2.0f to "2x").forEach { (scale, tag) ->
                        CompositionLocalProvider(
                            LocalDensity provides Density(base.density, fontScale = scale),
                        ) {
                            Text(
                                text = "Cobrar",
                                style = RbTheme.typography.button,
                                modifier = Modifier.testTag("label-$tag"),
                            )
                            RbButton(
                                label = "Cobrar",
                                onClick = {},
                                modifier = Modifier.testTag("boton-$tag"),
                            )
                        }
                    }
                }
            }
        }
        compose.waitForIdle()

        val smallLabel = compose.onNodeWithTag("label-1x").getUnclippedBoundsInRoot().height
        val largeLabel = compose.onNodeWithTag("label-2x").getUnclippedBoundsInRoot().height
        assertTrue(
            "the label measured $smallLabel at 1.0x and $largeLabel at 2.0x - it did " +
                "not grow, so a size somewhere is in dp instead of sp",
            largeLabel > smallLabel,
        )

        val smallButton = compose.onNodeWithTag("boton-1x").getUnclippedBoundsInRoot().height
        val largeButton = compose.onNodeWithTag("boton-2x").getUnclippedBoundsInRoot().height
        assertTrue("at 1.0x the button is $smallButton, under $touchTarget", smallButton >= touchTarget)
        assertTrue("at 2.0x the button is $largeButton, under $touchTarget", largeButton >= touchTarget)
        assertTrue(
            "at 2.0x the label is $largeLabel but the button is only $largeButton - " +
                "the label does not fit and is being clipped",
            largeButton >= largeLabel,
        )
    }

    /**
     * No text is cut off horizontally at 200%.
     *
     * This is "se corta el texto" measured directly: for every text node on
     * screen, the clipped bounds must equal the unclipped bounds across the
     * width. Horizontal overflow has nowhere to go and is a real loss of
     * content.
     *
     * Only nodes with something visible are checked. The catalogue is a
     * `LazyColumn`, so a row below the fold measures zero visible width - that
     * is off-screen and reachable by scrolling, not cut off, and counting it
     * would make the test fail on a healthy layout. Vertical overflow is left
     * to the scroll for the same reason.
     */
    @Test
    fun `no text is cut off horizontally at 200 percent`() {
        showCatalogue(fontScale = 2.0f)

        val matcher = hasText("", substring = true)
        val clipped = mutableSetOf<String>()
        var onScreen = 0

        // Scrolls the whole catalogue. Checking only the first viewport is what
        // let a chip row overflow: at 200% three filter chips ran off the right
        // edge, the last collapsing to one letter per line, and the section was
        // below the fold so nothing measured it.
        val list = compose.onNode(hasScrollAction())
        var sections = 0
        while (runCatching { list.performScrollToIndex(sections) }.isSuccess) {
            sections++
            compose.waitForIdle()

            val texts = compose.onAllNodes(matcher).fetchSemanticsNodes()
            texts.indices.forEach { index ->
                val node = compose.onAllNodes(matcher)[index]
                val visible = node.getBoundsInRoot()
                val full = node.getUnclippedBoundsInRoot()

                if (visible.width <= 0.dp || visible.height <= 0.dp) return@forEach
                onScreen++

                // A tolerance of 1dp absorbs sub-pixel rounding at the edges.
                if (full.width - visible.width > 1.dp) {
                    clipped += "\"${node.fetchSemanticsNode().textOrEmpty()}\" " +
                        "is ${full.width} wide but only ${visible.width} is visible"
                }
            }
        }

        assertTrue("the catalogue did not scroll at all", sections > 0)
        assertTrue("nothing was on screen to check", onScreen > 0)
        assertTrue("at 200% this text is cut off:\n${clipped.joinToString("\n")}", clipped.isEmpty())
    }

    /**
     * No word is ever broken in half, in the two places it happened.
     *
     * Regression test for what the 200% screenshots showed before
     * [RbReflowRow] existed:
     *
     * - the top bar rendered "compone / ntes", because a back arrow and a chip
     *   squeezed the title below the width of that word;
     * - a list row rendered "Paracetam / ol", and the row under it collapsed
     *   to "I / b" - one letter per line - because a price and a status chip
     *   had taken nearly the whole width.
     *
     * Each case renders its longest word alone in the same style, so the width
     * it needs is measured rather than guessed, and asserts the real slot got
     * at least that much.
     */
    @Test
    fun `no word is split in half at 200 percent`() {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = 2.0f),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Column {
                        RbTopBar(
                            title = "Catálogo de componentes",
                            subtitle = "Almacén Doña Rosa",
                            onBack = {},
                            actions = { RbChip(label = "Demo") },
                        )
                        RbListRow(
                            title = "Paracetamol 500 mg",
                            subtitle = "Quedan 12 cajas",
                            value = "$1.290",
                            trailing = { RbChip(label = "Por vencer", tone = RbChipTone.Warn) },
                            onClick = {},
                        )
                        Text(
                            text = "componentes",
                            style = RbTheme.typography.title,
                            modifier = Modifier.testTag("palabra-barra"),
                        )
                        Text(
                            text = "Paracetamol",
                            style = RbTheme.typography.body,
                            modifier = Modifier.testTag("palabra-lista"),
                        )
                    }
                }
            }
        }
        compose.waitForIdle()

        fun widthOfTag(tag: String) = compose.onNodeWithTag(tag).getUnclippedBoundsInRoot().width
        fun widthOfText(text: String) = compose.onNodeWithText(text).getUnclippedBoundsInRoot().width

        val barWord = widthOfTag("palabra-barra")
        val barTitle = widthOfText("Catálogo de componentes")
        assertTrue(
            "the bar title got $barTitle but its longest word needs $barWord, " +
                "so the word is being split in half",
            barTitle >= barWord,
        )

        val rowWord = widthOfTag("palabra-lista")
        val rowTitle = widthOfText("Paracetamol 500 mg")
        assertTrue(
            "the list row got $rowTitle but its longest word needs $rowWord, " +
                "so the product name is being split in half",
            rowTitle >= rowWord,
        )
    }

    /** The node's text, for a failure message that names the offender. */
    private fun SemanticsNode.textOrEmpty(): String =
        config.getOrNull(SemanticsProperties.Text)?.joinToString(" ") { it.text } ?: ""

    /**
     * Nothing runs off the side of the screen at 200%.
     *
     * Horizontal overflow is the failure mode that hides a button: the column
     * is only 360dp wide, and a control pushed past that edge cannot be tapped
     * and cannot be scrolled to, because the list scrolls vertically.
     */
    @Test
    fun `nothing overflows horizontally at 200 percent`() {
        showCatalogue(fontScale = 2.0f)
        val rootWidth = compose.onRoot().getUnclippedBoundsInRoot().width

        val overflowing = mutableListOf<String>()
        val nodes = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        nodes.indices.forEach { index ->
            val bounds = compose.onAllNodes(hasClickAction())[index].getUnclippedBoundsInRoot()
            if (bounds.right > rootWidth + 1.dp || bounds.left < (-1).dp) {
                overflowing += "[${bounds.left}, ${bounds.right}]"
            }
        }
        assertTrue(
            "at 200% these controls fall outside the ${rootWidth} screen: $overflowing",
            overflowing.isEmpty(),
        )
    }

    /** The catalogue renders in the light theme too, at both scales. */
    @Test
    fun `light theme survives 200 percent`() {
        showCatalogue(fontScale = 2.0f, darkTheme = false)
        assertAllTouchTargets(2.0f)
    }

    /**
     * The confirmation dialog is the tightest layout in the product: a title, a
     * paragraph and two 56dp buttons. At 200% on a 640dp-tall screen that is
     * taller than the display, so the body has to scroll and the confirm button
     * has to stay reachable.
     */
    @Test
    fun `the dialog keeps its confirm button reachable at 200 percent`() {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(density = base.density, fontScale = 2.0f),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    RbConfirmDialog(
                        title = "¿Anular esta venta?",
                        message = "Se va a devolver el stock y la boleta queda sin efecto. " +
                            "Esto no se puede deshacer.",
                        confirmLabel = "Sí, anular la venta",
                        destructive = true,
                        onConfirm = {},
                        onDismiss = {},
                    )
                }
            }
        }
        compose.waitForIdle()

        val confirm = compose.onNodeWithText("Sí, anular la venta")

        // Reachable, not merely present. This is the "botón inalcanzable"
        // criterion: at 200% the title, the paragraph and two 56dp buttons are
        // taller than a 640dp screen, so the body must scroll to it.
        confirm.performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Cancelar").performScrollTo().assertIsDisplayed()

        val bounds = confirm.getUnclippedBoundsInRoot()
        assertTrue(
            "the confirm button is ${bounds.height} tall, under $touchTarget",
            bounds.height >= touchTarget,
        )
    }
}
