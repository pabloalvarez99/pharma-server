//! Crate fino del server embebido en Android.
//!
//! Forma del artefacto: `cdylib` (`libservidor_android.so`), cargado con
//! `System.loadLibrary` dentro del proceso de la app. SELinux niega `execve`
//! de un binario suelto desde `untrusted_app` — ver commit `0f8ecc5`.
//!
//! Frontera (ADR-0021): este crate es **solo ciclo de vida** (arrancar, puerto,
//! parar). Nada del dominio se expone por JNI. La app habla Ktor/HTTP contra
//! `http://127.0.0.1:<puerto>` igual que contra un server de red.
//!
//! H2: exporta un símbolo C y fuerza el link de la entrada de producción
//! (`api::run`) para que el tamaño del `.so` refleje el grafo real bajo LTO,
//! no un shell vacío. H3+ agrega JNI.

/// Versión del puente (no del producto). String C null-terminated.
#[no_mangle]
pub extern "C" fn rutbusiness_servidor_version() -> *const std::os::raw::c_char {
    // Bajo LTO + strip, un `cdylib` que solo toca `default_config()` queda en
    // ~300 KB: el linker tira axum/surreal/etc. porque nadie los alcanza.
    // H5 va a llamar `api::run`; H2 tiene que medir ESE grafo.
    //
    // `black_box(false)` es opaco para el optimizador: el cuerpo se linkea
    // aunque en runtime nunca se ejecute. No se llama de verdad — no arranca
    // el server ni toca disco/red.
    if std::hint::black_box(false) {
        force_link_production_graph();
    }

    static VERSION: &[u8] = b"0.0.1-h2\0";
    VERSION.as_ptr().cast()
}

#[inline(never)]
fn force_link_production_graph() {
    // current_thread: el .so no debe spawnear un pool multi-thread solo por
    // un path muerto de medición; H5 pondrá su propio runtime.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return,
    };
    let _ = rt.block_on(api::run(api::default_config()));
}
