plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.compose)
}

// Design system: tema, tipografía, componentes base y la pantalla de catálogo.
//
// Es un módulo multiplataforma y no parte de `:app` por una razón concreta:
// `:app` es `com.android.application` + `kotlin.android`, y ahí `expect/actual`
// no compila. La regla de ADR-0021 es que la capa de UI pueda construirse para
// iOS sin tocarla, así que los composables viven en `commonMain` sin un solo
// import `android.*`, y lo específico de plataforma queda detrás de
// `expect/actual` en `androidMain` — hoy eso es un solo `actual`, el que lee la
// preferencia de "quitar animaciones" del sistema.
//
// Sumar iOS = agregar `iosArm64()` acá y escribir ese `actual` en un `iosMain`.
// `RbCmpDisciplineTest` falla el build si alguien mete un `android.*` en
// `commonMain`, para que la disciplina no dependa de que alguien se acuerde.
kotlin {
    androidTarget {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(libs.compose.runtime)
            implementation(libs.compose.ui)
            implementation(libs.compose.ui.graphics)
            implementation(libs.compose.ui.text)
            implementation(libs.compose.animation)
            // `api`, no `implementation`: `:app` construye pantallas con estos
            // tipos (Modifier, Text, RbTheme), así que los ve igual.
            api(libs.compose.foundation)
            api(libs.compose.material3)
        }
    }
}

android {
    namespace = "cl.rutbusiness.ui"
    compileSdk = 36

    defaultConfig {
        minSdk = 21
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    testOptions {
        unitTests {
            // Robolectric necesita los recursos empaquetados para levantar el
            // host de Compose; sin esto los tests de layout no arrancan.
            isIncludeAndroidResources = true

            all {
                // Los tests de escala miden layout real, y para eso necesitan
                // fuentes reales: con los gráficos por defecto de Robolectric
                // el motor de texto es un stub que devuelve ~0,5dp por
                // carácter. Con eso un título de 35 caracteres mide 17dp y
                // toda aserción de ancho pasa sin significar nada.
                it.systemProperty("robolectric.graphicsMode", "NATIVE")
            }
        }
    }
}

dependencies {
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.compose.ui.test.junit4)
    // `testImplementation` además de `debugImplementation`: este artefacto no
    // trae código, trae un `AndroidManifest.xml` que declara
    // `androidx.activity.ComponentActivity`, que es la que `createComposeRule`
    // levanta. Colgado sólo de `debug`, `testReleaseUnitTest` no lo ve y los 8
    // tests de escala mueren con "Unable to resolve activity for Intent ...
    // cmp=cl.rutbusiness.ui.test/androidx.activity.ComponentActivity" — o sea
    // la mitad de la suite del design system no corría en `./gradlew test`.
    testImplementation(libs.compose.ui.test.manifest)
    debugImplementation(libs.compose.ui.test.manifest)
}
