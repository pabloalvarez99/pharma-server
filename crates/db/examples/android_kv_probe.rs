//! Spike probe: ¿SurrealKV abre una base en un directorio app-private de
//! Android, sin permisos especiales?
//!
//! Se cruza-compila para Android y se corre en el dispositivo. Usa
//! [`db::connect`] a propósito — el mismo camino que usa el server — así lo que
//! se prueba es el código de producción y no una maqueta.
//!
//! Lo que un directorio app-private (`/data/data/<pkg>/files/…`) tiene de
//! distinto y puede romper un motor de storage:
//! * lo posee el UID de la app, no shell; nadie más lo puede leer;
//! * SELinux lo etiqueta `app_data_file`, no `shell_data_file`;
//! * no hace falta ningún permiso declarado en el manifiesto para escribirlo;
//! * es donde tiene que vivir la base si el ERP corre dentro del teléfono.
//!
//! Uso: `android_kv_probe <dir-de-la-base>`
//!
//! Sale 0 con `PROBE OK` si escribió y releyó; distinto de 0 con `PROBE FAIL`
//! y el error exacto si no.

use std::path::Path;

use pharma_core::config::DbConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Probe {
    marker: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(marker) => {
            println!("PROBE OK marker={marker}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            println!("PROBE FAIL {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<String> {
    let dir = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("uso: android_kv_probe <dir-de-la-base>"))?;

    println!("path={dir}");
    println!("uid={}", uid());

    // La app crea su propio directorio de datos: no hay instalador que lo
    // prepare antes.
    std::fs::create_dir_all(&dir)?;

    let cfg = DbConfig {
        path: dir.clone(),
        namespace: "spike".into(),
        database: "spike".into(),
    };

    // 1. Abrir. Acá es donde falla si el motor necesita algo que el sandbox no
    //    da (mmap, locking, exec sobre el propio directorio…).
    let conn = db::connect(&cfg).await?;
    println!("open=ok");

    // 2. Escribir y releer: abrir un handle no prueba nada si después no
    //    persiste.
    let marker = format!("spike-{}", std::process::id());
    conn.query("CREATE probe:one SET marker = $m")
        .bind(("m", marker.clone()))
        .await?
        .check()?;

    let mut r = conn.query("SELECT marker FROM probe:one").await?.check()?;
    let row: Option<Probe> = r.take(0)?;
    let read_back = row.map(|p| p.marker).unwrap_or_default();
    anyhow::ensure!(
        read_back == marker,
        "releyó '{read_back}', esperaba '{marker}'"
    );
    println!("write_read=ok");

    // 3. Que haya quedado algo en disco. Un motor que sólo vivió en memoria no
    //    sirve para el caso de uso.
    let bytes = dir_size(Path::new(&dir))?;
    println!("bytes_on_disk={bytes}");
    anyhow::ensure!(bytes > 0, "el directorio quedó vacío: no persistió nada");

    Ok(marker)
}

fn dir_size(p: &Path) -> std::io::Result<u64> {
    let mut total = 0;
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

#[cfg(unix)]
fn uid() -> String {
    // Sin dependencias nuevas: el UID efectivo sale de /proc en Android.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "desconocido".into())
}

#[cfg(not(unix))]
fn uid() -> String {
    "n/a (host)".into()
}
