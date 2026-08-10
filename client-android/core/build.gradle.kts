plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.serialization)
}

// Módulo multiplataforma: red, modelos y sesión. Hoy solo compila para Android,
// pero el código vive en `commonMain` y lo específico de plataforma está detrás
// de `expect/actual`. Agregar iOS es agregar `iosArm64()` acá y escribir los
// `actual` de `androidMain` en un `iosMain` — no reescribir la capa de red.
kotlin {
    // `expect/actual` sobre clases sigue marcado Beta por Kotlin. Se usa igual:
    // es el mecanismo del lenguaje para esto y no hay alternativa estable.
    compilerOptions {
        freeCompilerArgs.add("-Xexpect-actual-classes")
    }

    androidTarget {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    sourceSets {
        commonMain.dependencies {
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
            api(libs.ktor.client.core)
            api(libs.ktor.client.content.negotiation)
            api(libs.ktor.serialization.kotlinx.json)
        }
        androidMain.dependencies {
            implementation(libs.ktor.client.okhttp)
            implementation(libs.kotlinx.coroutines.android)
            implementation(libs.androidx.datastore.preferences)
            // Entrar con Google (ADR-0022). Sólo `androidMain`: el contrato
            // `IdentidadGoogle` vive en `commonMain` sin saber que existen, y
            // por eso el día que haya iOS no hay que tocar la interfaz.
            //
            // Estas tres suman ~1 MB al APK y viajan aunque el build no traiga
            // client id. Se acepta: partirlas por flavor duplicaría la matriz de
            // builds (y el APK ya sale partido por ABI) para ahorrar menos de lo
            // que pesa el escáner de códigos.
            implementation(libs.androidx.credentials)
            implementation(libs.androidx.credentials.play.services.auth)
            implementation(libs.googleid)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.kotlinx.coroutines.test)
            implementation(libs.ktor.client.mock)
        }
    }
}

android {
    namespace = "cl.rutbusiness.core"
    compileSdk = 36

    defaultConfig {
        minSdk = 23
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
