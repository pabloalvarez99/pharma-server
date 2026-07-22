//! Invalida el build de `db` cuando cambia `migrations/`.
//!
//! `migrate.rs` embebe el directorio con `include_dir!` a tiempo de compilación,
//! pero cargo NO rastrea el contenido de un directorio embebido: agregar
//! `migrations/NNNN_*.surql` no recompilaba este crate, así que el binario salía
//! con el set VIEJO y la migración nueva nunca se aplicaba al arrancar (falla
//! silenciosa: sobre una tabla SCHEMAFULL los campos no definidos se descartan
//! sin error). Declarar el directorio como dependencia lo evita.

use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    println!("cargo:rerun-if-changed={}", dir.display());
    // Además cada archivo: en algunas plataformas el mtime del directorio no
    // cambia al editar un archivo existente.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            println!("cargo:rerun-if-changed={}", e.path().display());
        }
    }
}
