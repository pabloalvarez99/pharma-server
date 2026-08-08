package cl.rutbusiness.ui.theme

import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics

/**
 * Accessibility markers, named once so a component asks for the meaning instead
 * of repeating the incantation.
 *
 * These are the Compose equivalents of the ARIA the web design system already
 * carried: `ui.ts` marked its error block `role="alert"` and its loading block
 * `role="status" aria-live="polite"`. Dropping them in the port would have
 * silently regressed the product for a TalkBack user.
 */

/** Marks text as a heading, so TalkBack can jump between sections. */
fun Modifier.rbHeading(): Modifier = semantics { heading() }

/**
 * Announced as soon as it appears, interrupting whatever is being read.
 * The Compose equivalent of `role="alert"`. For errors only - anything else
 * that interrupts is noise.
 */
fun Modifier.rbAssertive(): Modifier =
    semantics { liveRegion = LiveRegionMode.Assertive }

/**
 * Announced when the reader is free. The equivalent of
 * `aria-live="polite"`, used by the loading state so "Cargando..." reaches a
 * blind user without cutting off what they were listening to.
 */
fun Modifier.rbPolite(): Modifier =
    semantics { liveRegion = LiveRegionMode.Polite }
