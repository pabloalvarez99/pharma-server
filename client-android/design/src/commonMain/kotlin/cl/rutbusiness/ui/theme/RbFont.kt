package cl.rutbusiness.ui.theme

import androidx.compose.ui.text.font.FontFamily

/**
 * The product's one bundled type family: Public Sans, SIL OFL 1.1.
 *
 * `RbTypography.kt` used to argue against bundling any font at all, because
 * `brand.css` asked for three families (Fraunces + IBM Plex Sans + IBM Plex
 * Mono) and three families really do cost megabytes. That argument does not
 * hold against **one** family. Measured, not estimated: four static weights
 * of Public Sans (Regular/Medium/SemiBold/Bold, the exact four this type
 * scale uses) total 338,568 bytes - about 330 KiB, roughly 1.8% of the
 * 18.3 MB APK ADR-0021 measured on the reference device. That is the
 * single highest-leverage change available for "looks produced": the system
 * sans is the one thing every unstyled Android screen already looks like.
 *
 * Two choices that were made on purpose, not by default:
 *
 * 1. **Public Sans over Inter.** Inter's own variable file is 879,708 bytes
 *    by itself (it ships an optical-size axis and a much larger glyph set)
 *    - more than double the entire four-weight set used here. Work Sans's
 *    variable file measured 361,072 bytes for the weight axis alone, still
 *    more than four static Public Sans weights combined. Public Sans is
 *    also the USWDS accessibility-first face - a reasonable thematic fit
 *    for a product whose hardware floor is explicitly the older person on
 *    the cheap phone.
 * 2. **Static weight files, not one variable file plus runtime axis
 *    interpolation.** [androidx.compose.ui.text.font.Font]'s own KDoc says
 *    variation settings "on API 26 and above ... are applied to a variable
 *    font when the font is loaded" - below API 26 they are silently
 *    ignored and every weight collapses to the font's single default
 *    instance. `minSdk = 23` is a deliberate, recent decision (see
 *    `libs.versions.toml`), so API 23-25 is real population, not a
 *    rounding error - and it is exactly the cheap/old end of the hardware
 *    floor. A single variable file would have quietly undone this same
 *    ola's weight-contrast goal on precisely the devices the floor exists
 *    to protect. Four small static cuts cost more bytes (338 KiB instead of
 *    ~103 KiB for the variable file alone) but render the declared weight
 *    everywhere, no OS-version caveat.
 *
 * `expect`/`actual` for the same reason [systemReducedMotion] is: loading a
 * bundled font from resources needs the generated `R` class, which only
 * exists on Android, so the real loading lives in `androidMain` and
 * `commonMain` only declares the shape.
 */
expect val RbBrandFont: FontFamily
