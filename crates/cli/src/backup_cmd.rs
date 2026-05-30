//! `pharma backup` sub-commands: create, restore, list.
//!
//! Backups are tar+gzip archives of the SurrealKv data directory (the parent
//! of `cfg.db.path`) stored under `<data_dir>/backups/` by default.  The
//! same format is used by `POST /api/v1/admin/backup` so archives are
//! interchangeable.
//!
//! Safety notes:
//! - `backup create`: SurrealKv is an LSM store; a hot backup is
//!   crash-recoverable on restore (WAL replay).  For a fully quiesced
//!   snapshot stop the service first.
//! - `backup restore`: overwrites the live data dir.  The server MUST be
//!   stopped first.  The command checks for a listening port before
//!   proceeding and aborts with a Spanish error message if it detects one.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use chrono::Utc;
use clap::Subcommand;
use sha2::{Digest, Sha256};

// ──────────────────────────────────────────────────────────────────────────────
// Clap definitions
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum BackupCmd {
    /// Crear un snapshot tar.gz del directorio de datos.
    Create {
        /// Ruta de salida.  Por defecto: `./backups/pharma-backup-<ts>.tar.gz`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Restaurar la base de datos desde un snapshot.  El servidor debe estar
    /// detenido (`sc stop PharmaServer`) antes de ejecutar este comando.
    Restore {
        /// Ruta al archivo .tar.gz generado por `backup create`.
        path: PathBuf,
    },
    /// Listar backups disponibles (más reciente primero).
    List {
        /// Directorio donde buscar.  Por defecto: `./backups/`.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

// ──────────────────────────────────────────────────────────────────────────────
// Entry point
// ──────────────────────────────────────────────────────────────────────────────

pub async fn run(cmd: BackupCmd) -> anyhow::Result<()> {
    let cfg = pharma_core::config::AppConfig::load()?;
    let db_path = PathBuf::from(&cfg.db.path);

    match cmd {
        BackupCmd::Create { output } => cmd_create(&db_path, output),
        BackupCmd::Restore { path } => cmd_restore(&db_path, &path, &cfg.bind),
        BackupCmd::List { dir } => cmd_list(&db_path, dir),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// backup create
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_create(db_path: &Path, output: Option<PathBuf>) -> anyhow::Result<()> {
    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    let out_path = match output {
        Some(p) => p,
        None => {
            let backups_dir = data_dir.join("backups");
            std::fs::create_dir_all(&backups_dir)
                .with_context(|| format!("crear directorio {}", backups_dir.display()))?;
            let ts = Utc::now().format("%Y%m%d-%H%M%S");
            backups_dir.join(format!("pharma-backup-{ts}.tar.gz"))
        }
    };

    // Ensure the output parent directory exists.
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("crear directorio {}", parent.display()))?;
        }
    }

    println!("Exportando base de datos...");

    let file = std::fs::File::create(&out_path)
        .with_context(|| format!("crear {}", out_path.display()))?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);

    // Pack the SurrealKv data subdir under `surreal/`.
    if db_path.exists() {
        tar.append_dir_all("surreal", db_path)
            .with_context(|| format!("empaquetar {}", db_path.display()))?;
    }

    // Pack agent.key if present so federation identity survives the restore.
    let key_path = data_dir.join("agent.key");
    if key_path.exists() {
        let mut kf = std::fs::File::open(&key_path)
            .with_context(|| format!("leer {}", key_path.display()))?;
        tar.append_file("agent.key", &mut kf)
            .context("agregar agent.key al archivo")?;
    }

    let gz = tar.into_inner().context("finalizar tar")?;
    gz.finish()?.flush().context("finalizar gzip")?;

    let meta = std::fs::metadata(&out_path)
        .with_context(|| format!("leer metadatos de {}", out_path.display()))?;
    let size_kb = meta.len() / 1024;

    println!("✓ Backup creado: {} ({} KB)", out_path.display(), size_kb);
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// backup restore
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_restore(db_path: &Path, archive: &Path, bind_addr: &str) -> anyhow::Result<()> {
    // Verify the server is not running by trying to connect to its port.
    if server_is_listening(bind_addr) {
        return Err(anyhow!(
            "El servidor debe estar detenido antes de restaurar. \
             Usa `sc stop PharmaServer` y vuelve a intentar."
        ));
    }

    if !archive.exists() {
        return Err(anyhow!("Archivo no encontrado: {}", archive.display()));
    }

    println!("Restaurando base de datos desde {}...", archive.display());

    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    // Remove the current surreal dir before extracting so stale files don't
    // linger after a partial restore.
    if db_path.exists() {
        std::fs::remove_dir_all(db_path)
            .with_context(|| format!("limpiar {}", db_path.display()))?;
    }

    let f = std::fs::File::open(archive).with_context(|| format!("abrir {}", archive.display()))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);

    for entry in tar.entries().context("leer entradas del archivo")? {
        let mut entry = entry.context("entrada inválida en el archivo")?;
        let entry_path = entry
            .path()
            .context("ruta inválida en el archivo")?
            .into_owned();
        let entry_path_str = entry_path.to_string_lossy();

        // Map `surreal/...` → `<data_dir>/surreal/...` and `agent.key` → `<data_dir>/agent.key`.
        let dest = if entry_path_str.starts_with("surreal") {
            data_dir.join(&*entry_path)
        } else if entry_path_str == "agent.key" {
            data_dir.join("agent.key")
        } else {
            // Skip unrecognised entries.
            continue;
        };

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("crear directorio {}", parent.display()))?;
        }

        // Only extract regular files (skip directory entries — create_dir_all above handles them).
        if entry.header().entry_type().is_file() {
            let mut out = std::fs::File::create(&dest)
                .with_context(|| format!("crear {}", dest.display()))?;
            std::io::copy(&mut entry, &mut out)
                .with_context(|| format!("escribir {}", dest.display()))?;
        }
    }

    println!("✓ Restauración completada.");
    Ok(())
}

/// Returns `true` when a TCP listener is accepting on the server's bind address.
/// Parsing `bind_addr` (e.g. `"0.0.0.0:8080"`) — fall back to port 8080 on
/// localhost if parsing fails so the check is never silently skipped.
fn server_is_listening(bind_addr: &str) -> bool {
    // Extract the port from the bind address; default 8080.
    let port: u16 = bind_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = format!("127.0.0.1:{port}");
    // A successful TCP connect means something is listening.
    std::net::TcpStream::connect_timeout(
        &addr
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 8080))),
        std::time::Duration::from_millis(300),
    )
    .is_ok()
}

// ──────────────────────────────────────────────────────────────────────────────
// backup list
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_list(db_path: &Path, dir: Option<PathBuf>) -> anyhow::Result<()> {
    let backups_dir = match dir {
        Some(d) => d,
        None => {
            let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
            data_dir.join("backups")
        }
    };

    if !backups_dir.exists() {
        println!("No hay backups en {}.", backups_dir.display());
        return Ok(());
    }

    struct Entry {
        name: String,
        date: chrono::DateTime<chrono::Utc>,
        size_kb: u64,
    }

    let mut entries: Vec<Entry> = std::fs::read_dir(&backups_dir)
        .with_context(|| format!("leer directorio {}", backups_dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy().to_string();
            s.starts_with("pharma-backup-") && s.ends_with(".tar.gz")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let mtime = meta.modified().ok()?;
            let dur = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let dt = chrono::DateTime::from_timestamp(dur.as_secs() as i64, 0)?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                date: dt,
                size_kb: meta.len() / 1024,
            })
        })
        .collect();

    if entries.is_empty() {
        println!("No hay backups en {}.", backups_dir.display());
        return Ok(());
    }

    // Sort newest first.
    entries.sort_by_key(|e| std::cmp::Reverse(e.date));

    println!("{:<50}  {:<22}  SIZE (KB)", "ARCHIVO", "FECHA");
    for e in &entries {
        println!(
            "{:<50}  {:<22}  {}",
            e.name,
            e.date.format("%Y-%m-%d %H:%M:%S UTC"),
            e.size_kb
        );
    }
    println!("({} backup(s))", entries.len());

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// SHA-256 helper (used by tests; kept private so clippy doesn't warn about dead code)
// ──────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn sha256_of(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────────

    fn make_fake_data_dir() -> (TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("surreal");
        std::fs::create_dir_all(&db_path).unwrap();
        // Write a couple of fake SurrealKv files.
        std::fs::write(db_path.join("data.db"), b"fake-surreal-data").unwrap();
        std::fs::write(tmp.path().join("agent.key"), b"fake-agent-key").unwrap();
        (tmp, db_path)
    }

    // ── arg parsing tests (no DB required) ────────────────────────────────────

    #[test]
    fn parse_backup_create_no_output() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "create"]);
        assert!(matches!(cli.cmd, BackupCmd::Create { output: None }));
    }

    #[test]
    fn parse_backup_create_with_output() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "create", "--output", "/tmp/my.tar.gz"]);
        if let BackupCmd::Create { output: Some(p) } = cli.cmd {
            assert_eq!(p, PathBuf::from("/tmp/my.tar.gz"));
        } else {
            panic!("expected Create with output");
        }
    }

    #[test]
    fn parse_backup_restore() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "restore", "/tmp/snap.tar.gz"]);
        if let BackupCmd::Restore { path } = cli.cmd {
            assert_eq!(path, PathBuf::from("/tmp/snap.tar.gz"));
        } else {
            panic!("expected Restore");
        }
    }

    #[test]
    fn parse_backup_list_no_dir() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "list"]);
        assert!(matches!(cli.cmd, BackupCmd::List { dir: None }));
    }

    #[test]
    fn parse_backup_list_with_dir() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "list", "--dir", "/tmp/bk"]);
        if let BackupCmd::List { dir: Some(d) } = cli.cmd {
            assert_eq!(d, PathBuf::from("/tmp/bk"));
        } else {
            panic!("expected List with dir");
        }
    }

    // ── default path timestamp format ──────────────────────────────────────────

    #[test]
    fn default_output_path_has_correct_format() {
        // The timestamp in the filename must match `YYYYMMDD-HHMMSS`.
        let ts = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        // Must be 15 chars: 8 digits + dash + 6 digits.
        assert_eq!(ts.len(), 15);
        // First 8 chars are digits (date), then '-', then 6 digits (time).
        let (date_part, rest) = ts.split_at(8);
        assert!(date_part.chars().all(|c| c.is_ascii_digit()));
        let (sep, time_part) = rest.split_at(1);
        assert_eq!(sep, "-");
        assert!(time_part.chars().all(|c| c.is_ascii_digit()));
    }

    // ── roundtrip: create then restore ────────────────────────────────────────

    #[test]
    fn create_produces_valid_tar_gz() {
        let (_tmp, db_path) = make_fake_data_dir();
        let out_path = _tmp.path().join("backups").join("test.tar.gz");

        cmd_create(&db_path, Some(out_path.clone())).expect("create should succeed");

        assert!(out_path.exists());
        let meta = std::fs::metadata(&out_path).unwrap();
        assert!(meta.len() > 0, "archive must not be empty");

        // Check it's a readable tar.gz containing surreal/ entries.
        let f = std::fs::File::open(&out_path).unwrap();
        let gz = flate2::read::GzDecoder::new(f);
        let mut tar = tar::Archive::new(gz);
        let names: Vec<String> = tar
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("surreal")),
            "expected surreal entry, got {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "agent.key"),
            "expected agent.key, got {names:?}"
        );
    }

    #[test]
    fn restore_recreates_data_dir() {
        let (_tmp, db_path) = make_fake_data_dir();
        let archive = _tmp.path().join("snap.tar.gz");

        // Create a backup first.
        cmd_create(&db_path, Some(archive.clone())).expect("create");

        // Wipe the data dir to simulate a fresh install.
        std::fs::remove_dir_all(&db_path).unwrap();
        assert!(!db_path.exists());

        // Restore — pass a bind addr that is definitely not listening.
        cmd_restore(&db_path, &archive, "127.0.0.1:19999").expect("restore");

        assert!(db_path.exists(), "surreal dir must be restored");
        assert!(
            db_path.join("data.db").exists(),
            "data.db must be restored inside surreal/"
        );
        assert!(
            _tmp.path().join("agent.key").exists(),
            "agent.key must be restored"
        );
    }

    #[test]
    fn list_shows_backup_files() {
        let (_tmp, db_path) = make_fake_data_dir();

        // Create two backups.
        let bk_dir = _tmp.path().join("backups");
        std::fs::create_dir_all(&bk_dir).unwrap();
        std::fs::write(bk_dir.join("pharma-backup-20250101-120000.tar.gz"), b"a").unwrap();
        std::fs::write(bk_dir.join("pharma-backup-20250102-120000.tar.gz"), b"b").unwrap();
        // Non-backup file should be ignored.
        std::fs::write(bk_dir.join("not-a-backup.txt"), b"c").unwrap();

        // Should not error even though we're not using AppConfig.
        cmd_list(&db_path, Some(bk_dir.clone())).expect("list should succeed");
    }

    #[test]
    fn list_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        let bk_dir = tmp.path().join("backups");
        std::fs::create_dir_all(&bk_dir).unwrap();

        cmd_list(&db_path, Some(bk_dir)).expect("list empty dir should succeed");
    }

    #[test]
    fn restore_fails_if_archive_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        let missing = tmp.path().join("does-not-exist.tar.gz");

        let err = cmd_restore(&db_path, &missing, "127.0.0.1:19999")
            .expect_err("should fail on missing archive");
        assert!(
            err.to_string().contains("Archivo no encontrado"),
            "unexpected error: {err}"
        );
    }
}
