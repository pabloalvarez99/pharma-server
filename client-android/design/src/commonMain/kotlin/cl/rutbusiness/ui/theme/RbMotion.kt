package cl.rutbusiness.ui.theme

import androidx.compose.animation.core.AnimationSpec
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
 * Motion budget.
 *
 * Two independent reasons to stand still, and both are honoured:
 *
 * 1. **The user asked.** "Quitar animaciones" in Android's accessibility
 *    settings, or a zeroed animator duration scale in Opciones de
 *    desarrollador. Read through [systemReducedMotion], which is
 *    `expect`/`actual` because the source of that flag is per-platform.
 * 2. **The device cannot afford it.** The reference device is a 60 Hz panel
 *    with 2 GB of RAM. Nothing here animates a layout size or a shadow; the
 *    specs below only ever drive color and alpha.
 *
 * The CSS did this with `@media (prefers-reduced-motion: reduce)`, which
 * covered the spinner, the skeleton shimmer and the state reveal, but left
 * `.ui-btn`'s `transform: translateY(1px)` and the `.rb-btn:hover` lift
 * running. That is ported as: **there is no transform on press at all.** A
 * control that moves under the finger is harder to hit, which is the opposite
 * of what a 56dp target is for.
 *
 * When motion is reduced, [RbMotion.spec] returns [snap], so an animated value
 * still *arrives* - the state is never stuck at its start. Nothing is skipped,
 * only shortened to zero.
 */
@Immutable
data class RbMotion(
    /** True when the user asked the system for less animation. */
    val reduced: Boolean,
) {
    /** Short transition: color of a pressed control. */
    fun <T> quick(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = 120)

    /** Standard transition: a state block appearing. */
    fun <T> standard(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(durationMillis = 200)

    /**
     * The looping spinner sweep.
     *
     * Returns `null` when motion is reduced. A `null` here is a contract, not
     * an omission: [cl.rutbusiness.ui.components.RbLoadingState] reads it and
     * draws a **static** ring plus the "Cargando..." label, so the screen still
     * says it is working without a single frame of animation.
     */
    fun spinnerSweep(): AnimationSpec<Float>? =
        if (reduced) {
            null
        } else {
            infiniteRepeatable(
                animation = tween(durationMillis = 900, easing = LinearEasing),
            )
        }
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
