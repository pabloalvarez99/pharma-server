//! Crate fino del server embebido en Android.
//!
//! Forma del artefacto: `cdylib` (`libservidor_android.so`), cargado con
//! `System.loadLibrary("servidor_android")` dentro del proceso de la app.
//! SELinux niega `execve` de un binario suelto desde `untrusted_app`.
//!
//! Frontera (ADR-0021): **solo ciclo de vida** (arrancar, puerto, parar) y
//! probes. Nada del dominio se expone por JNI. La app habla Ktor/HTTP contra
//! `http://127.0.0.1:<puerto>`.

use std::path::Path;

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use pharma_core::config::DbConfig;
use serde::Deserialize;

/// Versión del puente (C ABI, no JNI).
#[no_mangle]
pub extern "C" fn rutbusiness_servidor_version() -> *const std::os::raw::c_char {
    if std::hint::black_box(false) {
        force_link_production_graph();
    }
    static VERSION: &[u8] = b"0.0.1-h4\0";
    VERSION.as_ptr().cast()
}

/// H3 — JNI mínimo: string fijo `"h3-ok"`.
#[no_mangle]
pub extern "system" fn Java_cl_rutbusiness_servidor_PuenteNativo_nativeSaludo(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    jni_string_result(env, || Ok(String::from("h3-ok")))
}

/// H4 — SurrealKV en el directorio privado de la app.
///
/// Abre la base en `ruta`, escribe un marker, cierra el handle, reabre y lee.
/// Devuelve `"PROBE OK marker=… bytes=…"` o `"PROBE FAIL …"`.
///
/// Kotlin: `PuenteNativo.nativeProbe(filesDir.resolve("…").absolutePath)`.
#[no_mangle]
pub extern "system" fn Java_cl_rutbusiness_servidor_PuenteNativo_nativeProbe(
    mut env: JNIEnv,
    _class: JClass,
    ruta: JString,
) -> jstring {
    let path = match env.get_string(&ruta) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(e) => {
            return jni_string_result(env, || {
                Ok(format!("PROBE FAIL jni path: {e}"))
            });
        }
    };

    jni_string_result(env, || {
        // catch_unwind around the whole probe is inside jni_string_result.
        Ok(match run_probe_blocking(&path) {
            Ok(msg) => msg,
            Err(e) => format!("PROBE FAIL {e:#}"),
        })
    })
}

/// Shared JNI exit: catch_unwind + new_string. Never panics across FFI.
fn jni_string_result(
    env: JNIEnv,
    f: impl FnOnce() -> Result<String, String> + std::panic::UnwindSafe,
) -> jstring {
    let text = match std::panic::catch_unwind(f) {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => e,
        Err(_) => String::from("PROBE FAIL panic"),
    };
    match env.new_string(text) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn run_probe_blocking(dir: &str) -> anyhow::Result<String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(probe_surrealkv(dir))
}

#[derive(Debug, Deserialize)]
struct ProbeRow {
    marker: String,
}

/// Open → write → drop connection → reopen → read. Same path as production
/// (`db::connect` + SurrealKV), intentional.
async fn probe_surrealkv(dir: &str) -> anyhow::Result<String> {
    std::fs::create_dir_all(dir)?;

    let cfg = DbConfig {
        path: dir.to_string(),
        namespace: "spike".into(),
        database: "spike".into(),
    };

    let marker = format!(
        "h4-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    // --- session 1: write ---
    {
        let conn = db::connect(&cfg).await?;
        // Idempotent across test re-runs.
        let _ = conn.query("DELETE probe:one;").await;
        conn.query("CREATE probe:one SET marker = $m")
            .bind(("m", marker.clone()))
            .await?
            .check()?;
        // Explicit drop before reopen — proves durability, not a live handle.
        drop(conn);
    }

    // --- session 2: reopen and read ---
    let read_back = {
        let conn = db::connect(&cfg).await?;
        let mut r = conn.query("SELECT marker FROM probe:one").await?.check()?;
        let row: Option<ProbeRow> = r.take(0)?;
        row.map(|p| p.marker).unwrap_or_default()
    };

    anyhow::ensure!(
        read_back == marker,
        "releyó '{read_back}', esperaba '{marker}' (no sobrevivió al reabrir)"
    );

    let bytes = dir_size(Path::new(dir))?;
    anyhow::ensure!(bytes > 0, "el directorio quedó vacío: no persistió nada");

    Ok(format!("PROBE OK marker={marker} bytes={bytes}"))
}

fn dir_size(p: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    if !p.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        let md = entry.metadata()?;
        total += if md.is_dir() {
            dir_size(&entry.path())?
        } else {
            md.len()
        };
    }
    Ok(total)
}

#[inline(never)]
fn force_link_production_graph() {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return,
    };
    let _ = rt.block_on(api::run(api::default_config()));
}
