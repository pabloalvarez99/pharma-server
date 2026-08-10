import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    // Los DTOs del agente (`ui/assist/AssistApi.kt`) son `@Serializable` y viven
    // en este módulo. Sin el plugin, kotlinx no genera sus serializers y la
    // primera llamada revienta en tiempo de ejecución con "Serializer for class
    // ... is not found" — algo que ningún test de UI ve, porque la UI no
    // serializa. Lo encontró `AssistApiEnVivoTest` hablando con un server real.
    alias(libs.plugins.kotlin.serialization)
}

// ---------------------------------------------------------------------------
// Versión
//
// `versionName` (lo que lee una persona) se sube a mano en `version.properties`
// cuando cambia lo que la app hace. `versionCode` (lo que mira Play Store para
// saber qué es más nuevo) NO se toca a mano: sale de la cantidad de commits.
//
// Play rechaza para siempre un `versionCode` repetido y no deja bajarlo. Atarlo
// al historial de git lo hace subir solo — nadie tiene que acordarse — y dos
// builds del mismo commit dan el mismo número, que es lo que hace reproducible
// un artefacto.
// ---------------------------------------------------------------------------

/** Corre git en el repo y devuelve su salida, o `null` si no hay git ni repo. */
fun gitDice(vararg argumentos: String): String? = try {
    val proceso = ProcessBuilder(listOf("git") + argumentos)
        .directory(rootDir)
        .redirectErrorStream(true)
        .start()
    val salida = proceso.inputStream.bufferedReader().readText().trim()
    if (proceso.waitFor() == 0) salida else null
} catch (_: Exception) {
    null
}

/**
 * `RB_VERSION_CODE` existe para el único caso en que contar commits miente: un
 * CI que clona con `depth=1` ve un solo commit y publicaría `versionCode = 1`
 * encima de una versión más alta. El día que haya CI Android, o clona completo
 * o pasa el número.
 */
val versionCodeDeEsteBuild: Int = run {
    val forzado = System.getenv("RB_VERSION_CODE")?.trim().orEmpty()
    if (forzado.isNotEmpty()) {
        return@run forzado.toIntOrNull()
            ?: throw GradleException("RB_VERSION_CODE=\"$forzado\" no es un entero.")
    }
    val commits = gitDice("rev-list", "--count", "HEAD")?.toIntOrNull()
        ?: throw GradleException(
            "No se pudo contar los commits con git, así que no hay versionCode. " +
                "Si estás construyendo fuera de un clon de git, pasá RB_VERSION_CODE=<entero>.",
        )
    commits
}

val versionNameDeEsteBuild: String = run {
    val archivo = rootProject.file("version.properties")
    if (!archivo.isFile) {
        throw GradleException("Falta client-android/version.properties.")
    }
    val propiedades = Properties().apply { archivo.inputStream().use { load(it) } }
    val base = propiedades.getProperty("versionName")?.trim().orEmpty()
    if (base.isEmpty()) {
        throw GradleException("Falta `versionName` en client-android/version.properties.")
    }
    // El número entre paréntesis es lo único que distingue dos builds de la
    // misma versión, y es lo que se ve en Ajustes > Aplicaciones del teléfono.
    // Sin él, dos APK sideloadeados de commits distintos son indistinguibles
    // mirando el aparato, que es justo lo que hay que poder hacer al probar.
    "$base ($versionCodeDeEsteBuild)"
}

// ---------------------------------------------------------------------------
// Firma de release — REGLA 3, no negociable
//
// El keystore y sus claves viven FUERA del repo. Dos fuentes, en este orden:
//
//   1. `client-android/keystore.properties` — gitignorado.
//      Plantilla en `keystore.properties.ejemplo`.
//   2. Variables de entorno: RB_KEYSTORE_FILE, RB_KEYSTORE_PASSWORD,
//      RB_KEY_ALIAS, RB_KEY_PASSWORD.
//
// Si no hay ninguna, el build de release FALLA con el motivo escrito. Nunca cae
// a la firma de debug: un APK firmado con la clave de debug se instala igual y
// parece sano, pero después no se puede actualizar con el APK de verdad porque
// la firma no coincide — y eso se descubre en el teléfono de la dueña, con la
// app instalada, no acá.
// ---------------------------------------------------------------------------

val archivoDeFirma = rootProject.file("keystore.properties")

val propiedadesDeFirma = Properties().apply {
    if (archivoDeFirma.isFile) archivoDeFirma.inputStream().use { load(it) }
}

fun datoDeFirma(propiedad: String, variableDeEntorno: String): String? =
    (propiedadesDeFirma.getProperty(propiedad) ?: System.getenv(variableDeEntorno))
        ?.trim()
        ?.takeIf { it.isNotEmpty() }

val rutaDelKeystore = datoDeFirma("storeFile", "RB_KEYSTORE_FILE")
val claveDelKeystore = datoDeFirma("storePassword", "RB_KEYSTORE_PASSWORD")
val aliasDeLaClave = datoDeFirma("keyAlias", "RB_KEY_ALIAS")
// Lo habitual es que la clave del alias sea la misma que la del keystore;
// `keytool -genkeypair` acepta ese caso con Enter.
val claveDeLaClave = datoDeFirma("keyPassword", "RB_KEY_PASSWORD") ?: claveDelKeystore

/** Ruta relativa: se resuelve contra `client-android/`, no contra el cwd. */
val keystoreDeRelease = rutaDelKeystore?.let { ruta ->
    file(ruta).let { if (it.isAbsolute) it else rootProject.file(ruta) }
}

val comoConfigurarLaFirma = """
    |Poné los datos en client-android/keystore.properties (gitignorado):
    |
    |    storeFile=C:/ruta/FUERA/del/repo/rutbusiness-release.jks
    |    storePassword=<la clave del keystore>
    |    keyAlias=rutbusiness
    |    keyPassword=<la clave del alias, si es distinta>
    |
    |...o en las variables de entorno RB_KEYSTORE_FILE, RB_KEYSTORE_PASSWORD,
    |RB_KEY_ALIAS y RB_KEY_PASSWORD.
    |
    |Para crear un keystore de desarrollo:
    |    pwsh client-android/scripts/crear-keystore-dev.ps1
    |
    |REGLA 3: ni el keystore ni su clave entran a git, al vault, ni a Notion.
    |Se archiva el puntero (dónde está y cómo obtenerla), nunca el valor.
""".trimMargin()

val motivoSinFirma: String? = when {
    rutaDelKeystore == null || claveDelKeystore == null || aliasDeLaClave == null ->
        "El build de release no tiene firma configurada y NO cae a firma de debug.\n\n" +
            comoConfigurarLaFirma
    keystoreDeRelease?.isFile != true ->
        "El keystore de release no existe en la ruta configurada:\n" +
            "    $keystoreDeRelease\n\n" +
            comoConfigurarLaFirma
    else -> null
}

/**
 * Corta el build de release antes de empaquetar si no hay firma.
 *
 * Sin esto AGP produce en silencio un `app-...-release-unsigned.apk` que
 * `adb install` rechaza más tarde con un error que no dice nada de keystores.
 */
val exigirFirmaDeRelease = tasks.register("exigirFirmaDeRelease") {
    group = "verification"
    description = "Falla si el build de release no encuentra el keystore."
    val motivo = motivoSinFirma
    doLast {
        if (motivo != null) throw GradleException(motivo)
    }
}

tasks.configureEach {
    val empaquetaRelease = (name.startsWith("package") || name.startsWith("bundle")) &&
        name.contains("Release")
    if (empaquetaRelease) dependsOn(exigirFirmaDeRelease)
}

// ---------------------------------------------------------------------------
// APK por ABI vs. AAB: no se pueden pedir en la misma corrida
//
// El AAB ya parte por ABI adentro — es su razón de ser — así que `splits.abi`
// sólo tiene sentido en el camino APK. AGP no acepta los dos juntos y aborta con
// "Multiple shrunk-resources files found ... Please disable building multiple
// APKs when building an Android app bundle" (issuetracker 402800800).
//
// Entonces `splits.abi` se apaga cuando la corrida es de bundle. El peligro de
// eso es silencioso: con los splits apagados, `assembleRelease` produciría un
// APK universal, que es exactamente lo que el piso de hardware prohíbe. Por eso
// pedir las dos cosas juntas no se resuelve a medias, se rechaza.
// ---------------------------------------------------------------------------

val tareasPedidas = gradle.startParameter.taskNames.map { it.substringAfterLast(':') }
val pideBundle = tareasPedidas.any { it.startsWith("bundle") }
val pideApk = tareasPedidas.any { it.startsWith("assemble") || it.startsWith("install") }

if (pideBundle && pideApk) {
    throw GradleException(
        """
        |No se pueden construir el AAB y los APK en la misma corrida: AGP no deja
        |combinar `splits.abi` con el bundle, y apagar los splits produciría un APK
        |universal — prohibido por el piso de hardware.
        |
        |Corré las dos, una después de la otra:
        |    ./gradlew assembleRelease
        |    ./gradlew bundleRelease
        """.trimMargin(),
    )
}

android {
    namespace = "cl.rutbusiness.app"
    compileSdk = 36
    // Sin esto AGP no encuentra `strip` y empaqueta los .so nativos de Compose
    // con símbolos de debug adentro: megas de más en un teléfono sin espacio.
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "cl.rutbusiness.app"
        // PISO DE HARDWARE (CLAUDE.md, regla 1): Android 6.0, un teléfono de
        // 2015. La app Tauri era minSdk 24 y dejaba fuera aparatos que este
        // producto sí quiere servir; 23 los recupera casi todos.
        //
        // No bajamos a 21 (decisión del founder, 2026-08-07): AndroidX dejó de
        // soportar API 21-22 a mitad de 2025, así que sostener 21 obliga a
        // congelar Compose, Material3 y lifecycle en versiones sin parches de
        // seguridad. Para una app que mueve plata, quedarse sin parches por
        // llegar a teléfonos de 2014 —que además tienen 1 GB de RAM y no la van
        // a correr bien— es mal negocio.
        minSdk = 23
        targetSdk = 36
        // Ver el bloque "Versión" arriba: el código sale de git, el nombre de
        // `version.properties`. Ninguno se edita acá.
        versionCode = versionCodeDeEsteBuild
        versionName = versionNameDeEsteBuild

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables.useSupportLibrary = true

        // A dónde apunta "Crear mi negocio". Ver `ui/alta/Nube.kt`.
        //
        // Es un dato de la compilación y **no** una constante en el código
        // fuente, a propósito: una dirección de producción escrita en un .kt es
        // una decisión que nadie tomó y que después nadie puede cambiar sin
        // recompilar el mundo. Acá se pasa explícito al armar el APK:
        //
        //     ./gradlew :app:assembleRelease -Prb.urlNube=https://api.rutbusiness.cl
        //
        // Vacío por defecto — que es lo que hay en el repo — el alta pregunta
        // dónde está el computador del negocio en vez de inventarse un destino.
        // Nada de esto es un secreto: es una dirección pública, la misma que
        // cualquiera ve en el tráfico de la app. La clave de aprovisionamiento
        // NO viaja acá ni a ninguna parte del APK (ver `ui/alta/AltaApi.kt`).
        buildConfigField(
            "String",
            "URL_NUBE",
            "\"${providers.gradleProperty("rb.urlNube").getOrElse("")}\"",
        )
    }

    // PISO DE HARDWARE (regla 2): NUNCA un APK universal. Un teléfono con 8 GB
    // de almacenamiento no puede pagar 4 arquitecturas. El AAB ya parte por ABI
    // solo; esto cubre el camino APK (`assembleRelease`, sideload, QA).
    splits {
        abi {
            // Apagado sólo cuando la corrida es de bundle; ver el bloque de
            // arriba. En el camino APK siempre está prendido.
            isEnable = !pideBundle
            reset()
            include("armeabi-v7a", "arm64-v8a", "x86", "x86_64")
            isUniversalApk = false
        }
    }

    signingConfigs {
        // Sólo se crea si hay datos: si no, `release` queda sin `signingConfig`
        // y `exigirFirmaDeRelease` corta el build antes de empaquetar.
        if (motivoSinFirma == null) {
            create("release") {
                storeFile = keystoreDeRelease
                storePassword = claveDelKeystore
                keyAlias = aliasDeLaClave
                keyPassword = claveDeLaClave
                // v1 (JAR signing) hace falta de verdad: el esquema v2 llegó con
                // Android 7.0 y el piso de hardware es Android 6.0 (minSdk 23).
                // Sin v1 el APK no instala en el aparato objetivo.
                enableV1Signing = true
                enableV2Signing = true
                // v3 sólo suma: es el esquema que permite rotar la clave sin
                // perder la identidad de la app en Android 9+.
                enableV3Signing = true
            }
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
        }
        release {
            signingConfig = signingConfigs.findByName("release")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            // PISO DE HARDWARE (regla 6): arranque en frío medido en el aparato
            // lento. `src/main/baseline-prof.txt` se compila AOT al instalar.
            isProfileable = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    buildFeatures {
        compose = true
        // Sólo por `URL_NUBE`. AGP lo apaga por defecto desde 8.0.
        buildConfig = true
    }

    packaging {
        resources.excludes += setOf(
            "/META-INF/{AL2.0,LGPL2.1}",
            "/META-INF/DEPENDENCIES",
            "/META-INF/INDEX.LIST",
        )
    }

    testOptions {
        unitTests {
            isIncludeAndroidResources = true
            all {
                // Fuentes reales: con los gráficos por defecto de Robolectric el
                // motor de texto es un stub que devuelve ~0,5dp por carácter, y
                // toda medición de ancho pasa sin significar nada.
                it.systemProperty("robolectric.graphicsMode", "NATIVE")
                // `AssistApiEnVivoTest` habla con un pharma-api de verdad y se
                // salta solo si no hay ninguno. Gradle no pasa los `-D` de la
                // línea de comandos al JVM de tests, así que se reenvían acá.
                listOf(
                    "rb.assist.baseUrl",
                    "rb.assist.tenant",
                    "rb.assist.email",
                    "rb.assist.password",
                ).forEach { clave ->
                    System.getProperty(clave)?.let { valor -> it.systemProperty(clave, valor) }
                }
            }
        }
    }

    sourceSets["main"].java.srcDir("src/main/kotlin")
    sourceSets["test"].java.srcDir("src/test/kotlin")
    // La capa Bluetooth sólo se puede probar contra el framework de verdad:
    // `ImpresoraBluetoothAndroidTest` corre en un aparato con
    // `connectedDebugAndroidTest`.
    sourceSets["androidTest"].java.srcDir("src/androidTest/kotlin")
}

/**
 * Las pruebas unitarias corren en debug y sólo en debug.
 *
 * Casi todas montan Compose, y para eso hace falta la `ComponentActivity` que
 * hospeda la composición: la trae `compose.ui.test.manifest`, que entra por
 * `debugImplementation` porque su `AndroidManifest` no puede terminar en el APK
 * que se publica. En release ese manifiesto no está y Robolectric muere con
 * "Unable to resolve activity for Intent ... androidx.activity.ComponentActivity"
 * — un rojo permanente en `./gradlew test` que no decía nada sobre el código.
 *
 * No se pierde cobertura: no hay fuentes propias de release, así que
 * `testReleaseUnitTest` compilaba exactamente el mismo Kotlin dos veces.
 */
androidComponents {
    beforeVariants(selector().withBuildType("release")) { variante ->
        (variante as com.android.build.api.variant.HasHostTestsBuilder)
            .hostTests[com.android.build.api.variant.HostTestBuilder.UNIT_TEST_TYPE]
            ?.enable = false
    }
}

dependencies {
    implementation(project(":core"))
    implementation(project(":design"))

    // Sin BOM todavía: las versiones vienen del techo de minSdk 21 que ya no
    // rige. Volver al BOM es tarea aparte. Ver gradle/libs.versions.toml.
    implementation(libs.compose.runtime)
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.text)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.foundation)
    implementation(libs.compose.animation)
    implementation(libs.compose.material3)

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    // `LocalLifecycleOwner`: el visor de la cámara se ata al ciclo de vida real
    // para que la sesión de cámara se suelte al irse a segundo plano.
    implementation(libs.androidx.lifecycle.runtime.compose)
    // Instala el baseline profile en el arranque de aparatos viejos.
    implementation(libs.androidx.profileinstaller)

    // Escáner por cámara. Todo lo que sabe de CameraX y de ML Kit vive en
    // `cl.rutbusiness.app.camara`; `ui/` habla con la interfaz `CamaraDeCodigos`
    // y no importa nada de esto.
    implementation(libs.camerax.core)
    implementation(libs.camerax.camera2)
    implementation(libs.camerax.lifecycle)
    implementation(libs.camerax.view)
    implementation(libs.mlkit.barcode.scanning)
    // Encode de QR para la tarjeta de rescate (ADR-0022). Solo `core`, sin
    // wrappers de cámara: el escaneo ya lo hace ML Kit.
    implementation(libs.zxing.core)

    debugImplementation(libs.compose.ui.tooling)

    testImplementation(libs.junit)
    // La tarjeta de propuesta es el peor caso de la app al 200% de escala y se
    // mide en la JVM para que la prueba corra en cada commit y no cuando
    // alguien se acuerde de enchufar un teléfono.
    testImplementation(libs.robolectric)
    testImplementation(libs.compose.ui.test.junit4)
    // `Dispatchers.setMain`: sin esto no se puede probar nada que pase adentro
    // de un `viewModelScope.launch`, que es donde vive el comportamiento de
    // todos los ViewModel de la app. `:core` ya la usaba.
    testImplementation(libs.kotlinx.coroutines.test)
    debugImplementation(libs.compose.ui.test.manifest)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.test.espresso.core)
}
