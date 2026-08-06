package cl.rutbusiness.ui.theme

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Enforces the ADR-0021 rule that lets this UI compile for iOS later:
 * **no `android.*` import in `commonMain`.**
 *
 * A comment asking for that discipline gets broken by the first person who
 * needs a `Context` in a hurry, and the breakage is invisible until someone
 * tries the iOS target months later. This walks the source tree and fails the
 * build instead.
 *
 * The one legal place for platform code is `androidMain` - today that is
 * `RbMotion.android.kt`, the `actual` behind [systemReducedMotion].
 */
class RbCmpDisciplineTest {

    private val forbidden = Regex("""^\s*import\s+(android|androidx\.core|dalvik|java\.awt)\.""")

    /**
     * Locates `commonMain/kotlin`.
     *
     * Normally it walks up from the working directory, so the test does not
     * care whether Gradle runs it from the module or the root. The
     * `rb.sourceRoot` system property overrides that, for a build whose sources
     * live outside its own project directory.
     */
    private fun commonMainDir(): File {
        val sourceRoot: String? = System.getProperty("rb.sourceRoot")
        if (sourceRoot != null) {
            val fromProperty = File(sourceRoot, "composeApp/src/commonMain/kotlin")
            if (fromProperty.isDirectory) return fromProperty
            throw AssertionError("rb.sourceRoot=$sourceRoot has no composeApp/src/commonMain/kotlin")
        }

        val workingDir = System.getProperty("user.dir") ?: "."
        var dir: File? = File(workingDir)
        while (dir != null) {
            val candidate = File(dir, "composeApp/src/commonMain/kotlin")
            if (candidate.isDirectory) return candidate
            val here = File(dir, "src/commonMain/kotlin")
            if (here.isDirectory) return here
            dir = dir.parentFile
        }
        throw AssertionError("could not locate commonMain/kotlin from $workingDir")
    }

    @Test
    fun `commonMain has no platform imports`() {
        val root = commonMainDir()
        val offenders = mutableListOf<String>()

        root.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .forEach { file ->
                file.readLines().forEachIndexed { index, line ->
                    if (forbidden.containsMatchIn(line)) {
                        offenders += "${file.relativeTo(root)}:${index + 1}: ${line.trim()}"
                    }
                }
            }

        assertTrue(
            "commonMain must stay platform-free so the UI can build for iOS. " +
                "Move these behind expect/actual in androidMain:\n" +
                offenders.joinToString("\n"),
            offenders.isEmpty(),
        )
    }

    /**
     * Text sizes must be `sp`, never `dp`.
     *
     * A `dp` font size is a size the older person cannot change from Ajustes,
     * which is the exact failure the hardware floor forbids. Catching it by
     * reading the typography source is crude but it is the only check that
     * fires before the layout is on a screen.
     */
    @Test
    fun `no text size is declared in dp`() {
        val typography = File(commonMainDir(), "cl/rutbusiness/ui/theme/RbTypography.kt")
        assertTrue("RbTypography.kt not found at $typography", typography.isFile)

        val offenders = typography.readLines()
            .mapIndexedNotNull { index, line ->
                val trimmed = line.trim()
                val isCode = !trimmed.startsWith("*") && !trimmed.startsWith("//")
                if (isCode && Regex("""(fontSize|lineHeight)\s*=\s*[\d.]+\.dp""").containsMatchIn(line)) {
                    "${index + 1}: $trimmed"
                } else {
                    null
                }
            }

        assertTrue(
            "type sizes must be sp so the system font scale applies:\n" +
                offenders.joinToString("\n"),
            offenders.isEmpty(),
        )
    }
}
