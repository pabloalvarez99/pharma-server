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
        // PISO DE HARDWARE (CLAUDE.md, regla 1): Android 5.0. La app Tauri era
        // minSdk 24 y dejaba fuera aparatos que este producto sí quiere servir.
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables.useSupportLibrary = true
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
}

dependencies {
    implementation(project(":core"))
    implementation(project(":design"))

    // Sin BOM a propósito: el BOM arrastra Compose 1.10+, que exige minSdk 23.
    // Ver el comentario de arriba en gradle/libs.versions.toml.
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
    // Instala el baseline profile en el arranque de aparatos viejos.
    implementation(libs.androidx.profileinstaller)

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
