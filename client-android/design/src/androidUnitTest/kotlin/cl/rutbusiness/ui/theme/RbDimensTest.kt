package cl.rutbusiness.ui.theme

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Proves the shape and depth scales are actually scales - ascending,
 * unambiguous tiers - rather than numbers that happen to be declared near
 * each other.
 *
 * These are pure JVM assertions on [Dp][androidx.compose.ui.unit.Dp] values;
 * nothing here renders, so there is no Robolectric cost to asserting the
 * system stays coherent every time a token changes.
 */
class RbDimensTest {

    @Test
    fun `corner radii ascend from control to sheet`() {
        val d = RbDefaultDimens
        assertTrue(
            "a field's corner must be smaller than a card's: ${d.radiusField} vs ${d.radiusCard}",
            d.radiusField < d.radiusCard,
        )
        assertTrue(
            "a card's corner must be smaller than a dialog's: ${d.radiusCard} vs ${d.radiusDialog}",
            d.radiusCard < d.radiusDialog,
        )
        assertTrue(
            "a dialog's corner must be smaller than a pill's: ${d.radiusDialog} vs ${d.radiusPill}",
            d.radiusDialog < d.radiusPill,
        )
    }

    @Test
    fun `elevation ascends from card to raised`() {
        val d = RbDefaultDimens
        assertTrue("a card must actually lift off the mesa", d.elevationCard > 0.dp)
        assertTrue(
            "a raised surface (dialog, menu) must sit above a card: " +
                "${d.elevationCard} vs ${d.elevationRaised}",
            d.elevationCard < d.elevationRaised,
        )
    }

    @Test
    fun `spacing ramp is strictly ascending`() {
        val d = RbDefaultDimens
        val ramp = listOf(d.space1, d.space2, d.space3, d.space4, d.space5)
        ramp.zipWithNext().forEach { (smaller, bigger) ->
            assertTrue("$smaller must be smaller than $bigger in the spacing ramp", smaller < bigger)
        }
    }

    @Test
    fun `dialog shape carries radiusDialog, distinct from the card shape`() {
        // RoundedCornerShape does not expose its corner size for direct Dp
        // comparison, so this proves the distinction the only way available
        // without rendering: the two tokens that feed the shapes differ, and
        // RbDefaultShapes is built directly from RbDefaultDimens with no
        // intermediate transform to lose that difference.
        assertTrue(
            "RbShapes.dialog must be built from a different radius than RbShapes.card",
            RbDefaultDimens.radiusDialog != RbDefaultDimens.radiusCard,
        )
    }
}
