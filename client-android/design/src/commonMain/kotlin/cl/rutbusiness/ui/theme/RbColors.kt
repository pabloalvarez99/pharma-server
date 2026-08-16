package cl.rutbusiness.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * RutBusiness color tokens, ported from the web design system
 * (`client/src/brand.css` `--rb-*` + the `.ui-*` block) and re-graded so every
 * pair used by a component meets the hardware-floor rules:
 *
 *  - body text is AAA (>= 7.0) on every surface it can land on;
 *  - every other text/icon pair is AA (>= 4.5) as a hard floor;
 *  - control boundaries and input borders are >= 3.0 (WCAG 1.4.11);
 *  - no light grey on white and no low-contrast tint anywhere.
 *
 * Ratios are not eyeballed: [cl.rutbusiness.ui.theme.contrastRatio] implements
 * the WCAG 2.1 relative-luminance formula and `RbContrastTest` asserts every
 * pair below. If a token changes, that test fails.
 *
 * What changed versus the CSS, and why (each of these failed a rule):
 *
 * | CSS token / rule                  | Measured        | Replacement            |
 * |-----------------------------------|-----------------|------------------------|
 * | light `--rb-txt-3` #97a08c on #fff| 2.72 FAIL       | [textTertiary] #666e5c |
 * | dark  `--rb-txt-3` #5c6880        | 2.80 FAIL on raise | [textTertiary] #828fa8 |
 * | light `--rb-danger` #ff5d6c       | 2.99 FAIL       | [dangerText] #b3000f   |
 * | light `--rb-amber`  #f5b53d       | 1.65 FAIL       | [warnText] #7d5006     |
 * | light `--rb-info`   #5aa9ff       | 2.46 FAIL       | [infoText] #00519f     |
 * | `.ui-btn-danger` white on #ff5d6c | 2.99 FAIL       | [dangerFill] + white   |
 * | `--rb-line` as an input border    | 1.27 / 1.31     | [outlineStrong] >= 3.0 |
 *
 * The light theme in `brand.css` never declared its own status ramp - it
 * inherited the dark one, which is why amber/danger/info all failed on white.
 *
 * Gradients were dropped from filled controls. A label sitting over a
 * luminance *range* has no single measurable ratio, and the gradient costs an
 * extra shader on the reference device. Fills are flat.
 */
@Immutable
data class RbColors(
    /** Deepest app background. `--rb-bg`. */
    val background: Color,
    /** Default card / sheet surface. `--rb-surface`. */
    val surface: Color,
    /** Inset or alternating surface. `--rb-surface-2`. */
    val surfaceVariant: Color,
    /** Raised surface: dialogs, menus. `--rb-raise`. */
    val surfaceRaised: Color,

    /** Hairline divider between rows. Decorative only - never a control edge. */
    val outline: Color,
    /** Border of anything interactive (fields, outlined buttons). >= 3.0. */
    val outlineStrong: Color,

    /** Body copy. AAA on every surface above. `--rb-txt`. */
    val textPrimary: Color,
    /** Supporting copy and field labels. AAA. `--rb-txt-2`, re-graded. */
    val textSecondary: Color,
    /** The dimmest text the system allows. AA. `--rb-txt-3`, re-graded. */
    val textTertiary: Color,

    /** Brand accent for icons, selected marks and emphasis on a surface. */
    val brandText: Color,
    /** Flat fill of the primary button. */
    val brandFill: Color,
    /** Label on [brandFill]. */
    val onBrandFill: Color,
    /** Tinted brand background for chips and empty-state marks. */
    val brandContainer: Color,

    /** Destructive text and icons on a surface. */
    val dangerText: Color,
    /** Flat fill of the destructive button. */
    val dangerFill: Color,
    /** Label on [dangerFill]. */
    val onDangerFill: Color,
    /** Tinted danger background for the error state block. */
    val dangerContainer: Color,

    /** Warning text and icons. `--rb-amber`, re-graded for light. */
    val warnText: Color,
    /** Informational text and icons. `--rb-info`, re-graded for light. */
    val infoText: Color,

    /** Focus ring. Drawn as a solid stroke, not a soft glow: a blurred halo is
     *  invisible on a cheap panel in daylight. */
    val focusRing: Color,
    /** Dialog scrim. */
    val scrim: Color,

    /** True when this is the dark set. Lets a component pick an asset variant
     *  without re-deriving it from luminance. */
    val isDark: Boolean,
)

/**
 * Dark set — puesto de noche. Deep teal-night canvas (not cold Slack navy);
 * brand is a luminous ripe-lime that still clears AA on every surface.
 * outlineStrong is graded against surfaceRaised, the lightest dark surface.
 */
val RbDarkColors = RbColors(
    background = Color(0xFF0A1416),
    surface = Color(0xFF101C1F),
    surfaceVariant = Color(0xFF142226),
    surfaceRaised = Color(0xFF18282D),

    outline = Color(0xFF2A3F45),
    // Graded against surfaceRaised, the *lightest* dark surface — grading only
    // against the base background is how borders slipped under 3.0 on dialogs.
    outlineStrong = Color(0xFF5E7A82),

    textPrimary = Color(0xFFEEF5F2),
    textSecondary = Color(0xFFA3B8B4),
    textTertiary = Color(0xFF7E9692),

    brandText = Color(0xFF9AD63A),
    brandFill = Color(0xFF9AD63A),
    onBrandFill = Color(0xFF0A1404),
    brandContainer = Color(0xFF1A2E14),

    dangerText = Color(0xFFFF8F99),
    dangerFill = Color(0xFFD5001A),
    onDangerFill = Color(0xFFFFFFFF),
    dangerContainer = Color(0xFF2D1524),

    warnText = Color(0xFFF5B53D),
    infoText = Color(0xFF6BB2FF),

    focusRing = Color(0xFFB8E85A),
    scrim = Color(0xB3000000),

    isDark = true,
)

/**
 * Light set — feria al amanecer. Sun-washed kraft paper and cream surfaces;
 * brand is tomato-leaf / ripe-lime green (not generic SaaS mint). Status ramp
 * is light-specific: the CSS light theme reused dark amber/danger/info and
 * every one of them failed on white.
 */
val RbLightColors = RbColors(
    background = Color(0xFFF5EDE0),
    surface = Color(0xFFFFFBF4),
    surfaceVariant = Color(0xFFF8F1E6),
    surfaceRaised = Color(0xFFFFFBF4),

    outline = Color(0xFFD4C6A8),
    outlineStrong = Color(0xFF8A7650),

    textPrimary = Color(0xFF1A1F14),
    textSecondary = Color(0xFF4A4F3E),
    textTertiary = Color(0xFF5F6552),

    brandText = Color(0xFF3D6B12),
    brandFill = Color(0xFF3A6A0F),
    onBrandFill = Color(0xFFFFFFFF),
    brandContainer = Color(0xFFE5F0C8),

    dangerText = Color(0xFFB3000F),
    dangerFill = Color(0xFFC1001A),
    onDangerFill = Color(0xFFFFFFFF),
    dangerContainer = Color(0xFFF6DBDF),

    warnText = Color(0xFF7D5006),
    infoText = Color(0xFF00519F),

    focusRing = Color(0xFF3A6A0F),
    scrim = Color(0x99000000),

    isDark = false,
)

internal val LocalRbColors = staticCompositionLocalOf { RbDarkColors }
