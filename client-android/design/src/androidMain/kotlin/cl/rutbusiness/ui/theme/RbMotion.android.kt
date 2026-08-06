package cl.rutbusiness.ui.theme

import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.platform.LocalContext

/**
 * Android side of [systemReducedMotion].
 *
 * This file is the reason the composables can stay clean: it is the only place
 * in the design system that imports `android.*`, and it lives in `androidMain`,
 * not `commonMain`.
 *
 * `ANIMATOR_DURATION_SCALE` is the setting behind both "Quitar animaciones" in
 * Accesibilidad and the animation scales in Opciones de desarrollador. It reads
 * 0f when the user turned animation off. Reading it through
 * [Settings.Global.getFloat] with a default of 1f means a device that does not
 * expose the key behaves as if animation were on, which is the safe default -
 * we would rather animate a device that did not ask us not to than freeze one
 * that never opted in.
 */
@Composable
@ReadOnlyComposable
actual fun systemReducedMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    val scale = Settings.Global.getFloat(
        resolver,
        Settings.Global.ANIMATOR_DURATION_SCALE,
        1f,
    )
    return scale == 0f
}
