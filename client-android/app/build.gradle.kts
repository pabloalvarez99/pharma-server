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
        versionCode = 1
        versionName = "0.1.0"

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
            isEnable = true
            reset()
            include("armeabi-v7a", "arm64-v8a", "x86", "x86_64")
            isUniversalApk = false
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
        }
        release {
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

    debugImplementation(libs.compose.ui.tooling)

    testImplementation(libs.junit)
    // La tarjeta de propuesta es el peor caso de la app al 200% de escala y se
    // mide en la JVM para que la prueba corra en cada commit y no cuando
    // alguien se acuerde de enchufar un teléfono.
    testImplementation(libs.robolectric)
    testImplementation(libs.compose.ui.test.junit4)
    debugImplementation(libs.compose.ui.test.manifest)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.test.espresso.core)
}
