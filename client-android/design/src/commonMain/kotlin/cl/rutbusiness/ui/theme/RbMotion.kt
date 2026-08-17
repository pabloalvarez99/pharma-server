package cl.rutbusiness.ui.theme

import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * The product's motion vocabulary: three curves ([RbEasing]) and four
 * durations, each tied to a role rather than to a screen.
 *
 * A prototype jumps; a produced app transitions - but only if every screen
 * reaches for the *same* transition instead of inventing its own duration.
 * That is why call sites never write `tween(213)`: they ask [RbMotion] for the
 * role they are animating (a press, an element arriving, a destination
 * changing) and get back a spec that already matches everything else in the
 * product, and already honours the two reasons below to stand still.
 *
 * Two independent reasons to stand still, and both are honoured:
 *
 * 1. **The user asked.** "Quitar animaciones" in Android's accessibility
 *    settings, or a zeroed animator duration scale in Opciones de
 *    desarrollador. Read through [systemReducedMotion], which is
 *    `expect`/`actual` because the source of that flag is per-platform.
 * 2. **The device cannot afford it.** The reference device is a 60 Hz panel
 *    with 2 GB of RAM. Nothing here animates a layout size or a shadow; the
 *    specs below only ever drive color, alpha and translation - the three
 *    things the compositor moves without asking the CPU to lay out the page
 *    again.
 *
 * The CSS did this with `@media (prefers-reduced-motion: reduce)`, which
 * covered the spinner, the skeleton shimmer and the state reveal, but left
 * `.ui-btn`'s `transform: translateY(1px)` and the `.rb-btn:hover` lift
 * running. That is ported as: **there is no transform on press at all.** A
 * control that moves under the finger is harder to hit, which is the opposite
 * of what a 56dp target is for.
 *
 * When motion is reduced, every `fun <T>` below returns [snap], so an animated
 * value still *arrives* - the state is never stuck at its start. Nothing is
 * skipped, only shortened to zero. The reduction lives here, once, so a call
 * site can never forget to check it: forgetting is not a call the author
 * makes, because there is no branch to forget.
 *
 * **Which one to reach for:**
 * - [quick] - a control answering the finger: a press, a toggle, a border
 *   lighting up. Under 150ms or it reads as lag, not feedback.
 * - [standard] - an element arriving inside a screen that is already on
 *   screen: a state block appearing, a card settling into a list. Long enough
 *   to read as intentional, short enough not to make the dueña wait for her
 *   own list.
 * - [screenEnter] / [screenExit] - a whole destination changing (the three
 *   tabs in [cl.rutbusiness.app.ui.nav.ContenedorDeDestinos]). Always used as
 *   a pair, never alone: the outgoing destination leaves on [screenExit] while
 *   the incoming one arrives on [screenEnter], at the same time. Exit is
 *   shorter than enter on purpose - content leaving should get out of the way
 *   quickly, content arriving gets a beat longer so the eye has time to land
 *   on it before it is expected to read.
 *
 * **What none of these are for: the checkout path.** Cobrar is opened
 * hundreds of times a day by the same cashier; motion there has to *inform*
 * (where a number came from, that a tap registered), never *entertain*. A
 * transition enjoyed once and endured two hundred times is a bug wearing a
 * feature's clothes - so nothing in this file plays on a timer a cashier
 * cannot skip, and nothing decorative rides along with a charge.
 */
@Immutable
data class RbMotion(
    /** True when the user asked the system for less animation. */
    val reduced: Boolean,
) {
    /** A control answering the finger: press, toggle, a border lighting up. */
    fun <T> quick(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = QUICK_MS, easing = RbEasing.standard)

    /**
     * An element arriving inside a screen that is already showing: a state
     * block appearing, a row settling into a list that is already visible.
     */
    fun <T> standard(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = STANDARD_MS, easing = RbEasing.standard)

    /**
     * The incoming half of a destination change - see [RbMotion] for the
     * enter/exit pairing rule. Longer than [standard]: a whole destination
     * arriving is a bigger event than one element inside it, and deserves the
     * extra beat to register.
     */
    fun <T> screenEnter(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = SCREEN_ENTER_MS, easing = RbEasing.enter)

    /**
     * The outgoing half of a destination change. Shorter than [screenEnter]
     * on purpose - what is leaving should clear the stage, not linger on it.
     */
    fun <T> screenExit(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = SCREEN_EXIT_MS, easing = RbEasing.exit)

    /**
     * The looping spinner sweep.
     *
     * Returns `null` when motion is reduced. A `null` here is a contract, not
     * an omission: [cl.rutbusiness.ui.components.RbLoadingState] reads it and
     * draws a **static** ring plus the "Cargando..." label, so the screen still
     * says it is working without a single frame of animation. The same
     * `null`-means-static contract is why nothing else in this file returns an
     * `infiniteRepeatable`: an animation with no end is the one shape of motion
     * a Compose test clock can never call idle on, so the budget keeps
     * "runs forever" to this one call site, gated the same way as everything
     * else.
     */
    fun spinnerSweep(): AnimationSpec<Float>? =
        if (reduced) {
            null
        } else {
            infiniteRepeatable(
                animation = tween(durationMillis = SPINNER_SWEEP_MS, easing = LinearEasing),
            )
        }

    companion object {
        /** [quick] - a press has to answer before it reads as lag. */
        internal const val QUICK_MS = 120

        /** [standard] - an element arriving where the dueña is already looking. */
        internal const val STANDARD_MS = 200

        /** [screenEnter] - a whole destination arriving; longer than [STANDARD_MS]. */
        internal const val SCREEN_ENTER_MS = 260

        /** [screenExit] - shorter than [SCREEN_ENTER_MS]: leave quickly, arrive gently. */
        internal const val SCREEN_EXIT_MS = 180

        /** [spinnerSweep] - a full rotation of the loading ring. */
        internal const val SPINNER_SWEEP_MS = 900
    }
}

/**
 * The three owned easing curves behind [RbMotion].
 *
 * Compose's `tween` defaults to `FastOutSlowInEasing` for everything, which is
 * how a product ends up with fifteen screens that each *feel* slightly
 * different despite nobody choosing that on purpose - the default is the same
 * curve Android uses for its own chrome, so it reads as "system", not "this
 * product". Naming the curves is what makes them a decision instead of an
 * accident.
 *
 * Three curves for four durations, not four: [RbMotion.quick] and
 * [RbMotion.standard] both reach for [standard] here, because everything
 * either one animates (a color, a state block) starts and ends already inside
 * the viewport - there is no "off-screen" to rush out of or ease into.
 * [enter] and [exit] exist only for the
 * [RbMotion.screenEnter]/[RbMotion.screenExit] pair, where there *is* an
 * off-screen: content entering should decelerate into place so the eye can
 * follow it to rest, content leaving should accelerate away so it reads as
 * "gone", not "still sliding".
 */
internal object RbEasing {
    /** Gentle at both ends. Used by [RbMotion.quick] and [RbMotion.standard]. */
    val standard: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    /** Starts fast, decelerates into rest. Used by [RbMotion.screenEnter]. */
    val enter: Easing = CubicBezierEasing(0f, 0f, 0.2f, 1f)

    /** Starts slow, accelerates away. Used by [RbMotion.screenExit]. */
    val exit: Easing = CubicBezierEasing(0.4f, 0f, 1f, 1f)
}

/**
 * Whether the platform reports that the user wants reduced motion.
 *
 * `actual` per platform: Android reads `Settings.Global.ANIMATOR_DURATION_SCALE`
 * (0f means the user turned animations off). iOS will read
 * `UIAccessibility.isReduceMotionEnabled`. Keeping the read behind
 * `expect`/`actual` is what lets every composable in this package stay free of
 * `android.*` imports.
 */
@Composable
@ReadOnlyComposable
expect fun systemReducedMotion(): Boolean

internal val LocalRbMotion = staticCompositionLocalOf { RbMotion(reduced = false) }
