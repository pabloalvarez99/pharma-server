package cl.rutbusiness.ui.theme

import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import cl.rutbusiness.ui.R

/**
 * Android side of [RbBrandFont].
 *
 * Four static `.ttf` resources under `res/font/`, one per weight this type
 * scale actually declares (Normal/Medium/SemiBold/Bold) - see [RbFont.kt]
 * for why these are static cuts and not one variable file with runtime
 * [androidx.compose.ui.text.font.FontVariation]. Each [Font] entry's
 * `weight` is how [FontFamily] picks the right file for a requested
 * [androidx.compose.ui.text.TextStyle.fontWeight]; there is no synthesis
 * involved because a real cut exists for every weight in the scale.
 *
 * `runCatching` around the construction: a resource ID that fails to
 * resolve to real font bytes at load time (a corrupt or truncated file
 * surviving into a build, for instance) falls back to the system sans
 * rather than crashing text layout for the whole app. The four weights
 * inside stay real Public Sans on every device that can read them; only a
 * broken bundle loses the brand face, and even then the type scale itself
 * (sizes, line height, tracking) is unaffected because
 * [FontFamily.SansSerif] still honours every other field on the
 * [androidx.compose.ui.text.TextStyle].
 */
actual val RbBrandFont: FontFamily = runCatching {
    FontFamily(
        Font(R.font.rb_public_sans_regular, FontWeight.Normal),
        Font(R.font.rb_public_sans_medium, FontWeight.Medium),
        Font(R.font.rb_public_sans_semibold, FontWeight.SemiBold),
        Font(R.font.rb_public_sans_bold, FontWeight.Bold),
    )
}.getOrDefault(FontFamily.SansSerif)
