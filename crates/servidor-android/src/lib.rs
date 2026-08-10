//! Crate fino del server embebido en Android.
//!
//! Forma del artefacto: `cdylib` (`libservidor_android.so`), cargado con
//! `System.loadLibrary("servidor_android")` dentro del proceso de la app.
//! SELinux niega `execve` de un binario suelto desde `untrusted_app`.
//!
//! Frontera (ADR-0021): **solo ciclo de vida** (arrancar, puerto, parar) y
//! probes. Nada del dominio se expone por JNI. La app habla Ktor/HTTP contra
//! `http://127.0.0.1:<puerto>`.
//!
//! - H2: force-link de `api::run` para medir el `.so` real bajo LTO.
//! - H3: JNI mínimo (`nativeSaludo`) — prueba build → jniLibs → dlopen → JNI.
//! - H4/H5: probe KV y arranque consumen el seam de `api` en main (no se
//!   construye acá).

use jni::objects::JClass;
use jni::sys::jstring;
use jni::JNIEnv;

/// Versión del puente (C ABI, no JNI). Útil para depurar sin ART.
#[no_mangle]
pub extern "C" fn rutbusiness_servidor_version() -> *const std::os::raw::c_char {
    // Force-link del grafo de producción (H2). `black_box(false)` es opaco:
    // el cuerpo se linkea; no se ejecuta en runtime.
    if std::hint::black_box(false) {
        force_link_production_graph();
    }
    static VERSION: &[u8] = b"0.0.1-h3\0";
    VERSION.as_ptr().cast()
}

/// H3 — JNI más chico posible: devuelve un string fijo.
///
/// Kotlin: `cl.rutbusiness.servidor.PuenteNativo.nativeSaludo()`.
/// Cada entrada JNI va en `catch_unwind`: con `panic=unwind` un panic que
/// cruza el borde FFI es UB; con abort se lleva la app. Las dos no, una.
#[no_mangle]
pub extern "system" fn Java_cl_rutbusiness_servidor_PuenteNativo_nativeSaludo(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let text = match std::panic::catch_unwind(|| String::from("h3-ok")) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match env.new_string(text) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
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
