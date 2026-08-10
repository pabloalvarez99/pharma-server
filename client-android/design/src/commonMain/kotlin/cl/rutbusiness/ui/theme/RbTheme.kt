package cl.rutbusiness.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier

/**
 * The RutBusiness theme.
 *
 * Wrap the whole app in it once. Everything below reads tokens through
 * [RbTheme], never through `MaterialTheme`: the Material palette has no place
 * to put "the border of a control must clear 3.0 against its surface", so the
 * design system carries its own.
 *
 * Note what is *not* here: no dynamic color. Material You would repaint the
 * product with whatever the user's wallpaper produced, and a wallpaper-derived
 * ramp has no contrast guarantee at all - it would silently undo the grading in
 * [RbColors] on exactly the devices that need it most.
 */
object RbTheme {
    val colors: RbColors
        @Composable @ReadOnlyComposable get() = LocalRbColors.current

    val typography: RbTypography
        @Composable @ReadOnlyComposable get() = LocalRbTypography.current

    val dimens: RbDimens
        @Composable @ReadOnlyComposable get() = LocalRbDimens.current

    val shapes: RbShapes
        @Composable @ReadOnlyComposable get() = LocalRbShapes.current

    val motion: RbMotion
        @Composable @ReadOnlyComposable get() = LocalRbMotion.current
}

/**
 * @param darkTheme dark set when true. Defaults to the system setting; the
 *   showcase overrides it to render both side by side.
 * @param reducedMotion overrides the system's reduce-animations flag. Defaults
 *   to [systemReducedMotion]. Only tests and the showcase pass it explicitly.
 */
@Composable
fun RbTheme(
    darkTheme: Boolean = isSystemInDarkThemeCompat(),
    reducedMotion: Boolean = systemReducedMotion(),
    content: @Composable () -> Unit,
) {
    val colors = if (darkTheme) RbDarkColors else RbLightColors
    val motion = remember(reducedMotion) { RbMotion(reduced = reducedMotion) }

    CompositionLocalProvider(
        LocalRbColors provides colors,
        LocalRbTypography provides RbDefaultTypography,
        LocalRbDimens provides RbDefaultDimens,
        LocalRbShapes provides RbDefaultShapes,
        LocalRbMotion provides motion,
        // Text with no explicit style still lands on the graded body ramp
        // rather than Material's 16sp default.
        LocalTextStyle provides RbDefaultTypography.body,
        LocalContentColor provides colors.textPrimary,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.background),
        ) {
            content()
        }
    }
}

/**
 * The system's light/dark preference.
 *
 * Wrapped rather than calling `isSystemInDarkTheme()` at the call site so the
 * dependency is named in one place if the multiplatform source of this flag
 * ever needs an `expect`/`actual` of its own.
 */
@Composable
@ReadOnlyComposable
private fun isSystemInDarkThemeCompat(): Boolean =
    androidx.compose.foundation.isSystemInDarkTheme()
