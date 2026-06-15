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
//!   Restore is *validate-before-wipe*: the archive is fully extracted to a
//!   staging directory and verified to contain a `surreal/` payload BEFORE
//!   the live data dir is touched, so a corrupt or truncated archive can
//!   never destroy live data.  Restore also asks for explicit confirmation
//!   unless `--yes` is passed (non-interactive runs without `--yes` abort).

use std::io::{BufRead, Read, Write};
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
    /// Crear un snapshot tar.gz del directorio de datos (alias: `now`).
    #[command(visible_alias = "now")]
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
        /// Confirmar la restauración sin preguntar (sobrescribe los datos
        /// actuales).  Sin esta opción se solicita confirmación interactiva.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Inspeccionar el snapshot e informar qué restauraría SIN tocar los
        /// datos actuales (validación previa).  No requiere el servidor
        /// detenido ni confirmación.
        #[arg(long)]
        dry_run: bool,
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
        BackupCmd::Restore { path, yes, dry_run } => {
            cmd_restore(&db_path, &path, &cfg.bind, yes, dry_run)
        }
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

fn cmd_restore(
    db_path: &Path,
    archive: &Path,
    bind_addr: &str,
    yes: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    // Dry-run is a READ-ONLY validation: inspect the archive and report what a
    // restore WOULD write, without touching the live data dir. Safe to run
    // while the server is up and without confirmation — so handle it before any
    // other check and return.
    if dry_run {
        return print_dry_run(db_path, archive);
    }

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

    // Explicit confirmation: restore overwrites the live data dir.  Skip the
    // prompt only when `--yes` is passed.  Non-interactive runs (no `--yes`,
    // EOF/closed stdin) abort safely.
    if !yes {
        println!(
            "Vas a restaurar desde {} y SOBRESCRIBIR los datos actuales en {}.",
            archive.display(),
            db_path.display()
        );
        print!("¿Continuar? Escribe 's' para confirmar [s/N]: ");
        std::io::stdout().flush().ok();
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        if !confirm_restore(&mut reader) {
            return Err(anyhow!("Restauración cancelada por el usuario."));
        }
    }

    println!("Restaurando base de datos desde {}...", archive.display());

    restore_archive(db_path, archive)?;

    println!("✓ Restauración completada.");
    Ok(())
}

/// Dry-run inspection: reuse `jobs::backup::inspect_snapshot` (the same
/// validator the scheduler/API path uses) to report what a restore would write
/// WITHOUT extracting anything. Returns an error for a missing/corrupt archive
/// or one that carries no SurrealKv tree, so an operator can catch a bad backup
/// before it is too late to matter.
fn print_dry_run(db_path: &Path, archive: &Path) -> anyhow::Result<()> {
    let rep = jobs::inspect_snapshot(archive)
        .with_context(|| format!("inspeccionar {}", archive.display()))?;

    let kb = rep.total_uncompressed_bytes / 1024;
    println!("Inspección de snapshot (dry-run, no se modificó nada):");
    println!("  Archivo:            {}", rep.path.display());
    println!("  Entradas:           {}", rep.entries);
    println!("  Archivos surreal/:  {}", rep.surreal_files);
    println!("  Tamaño descomprimido: {} KB", kb);
    println!(
        "  Tiene árbol surreal/: {}",
        if rep.has_surreal_tree { "sí" } else { "no" }
    );
    println!(
        "  Tiene agent.key:    {}",
        if rep.has_agent_key { "sí" } else { "no" }
    );

    if rep.is_restorable() {
        println!(
            "✓ Snapshot restaurable. Una restauración SOBRESCRIBIRÍA los datos en {}.",
            db_path.display()
        );
        println!(
            "  Para restaurar: detén el servidor (`sc stop PharmaServer`) y ejecuta \
             `pharma backup restore {}`.",
            archive.display()
        );
        Ok(())
    } else {
        Err(anyhow!(
            "El snapshot NO es restaurable: no contiene un árbol `surreal/` con datos. \
             Una restauración dejaría la base vacía."
        ))
    }
}

/// Reads one line and returns `true` only for an affirmative answer
/// (`s`/`si`/`sí`/`y`/`yes`, case-insensitive).  EOF/empty/anything else =>
/// `false` (safe default: do not overwrite).
fn confirm_restore(reader: &mut impl BufRead) -> bool {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return false; // EOF / closed stdin
    }
    matches!(
        line.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    )
}

/// Validate-before-wipe restore.  Extracts the archive into a staging dir,
/// verifies it contains a `surreal/` payload, and only then swaps it into the
/// live data dir.  If the archive is corrupt/truncated or lacks `surreal/`,
/// returns an error WITHOUT touching the live data dir.
fn restore_archive(db_path: &Path, archive: &Path) -> anyhow::Result<()> {
    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));

    // Staging dir sibling of db_path so the final rename stays on one volume.
    let staging = data_dir.join(format!(
        ".restore-staging-{}",
        Utc::now().format("%Y%m%d-%H%M%S-%f")
    ));
    // Best-effort cleanup of a leftover staging dir from a crashed run.
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging)
        .with_context(|| format!("crear staging {}", staging.display()))?;

    // Extract fully into staging; on ANY failure, drop staging and bail with
    // the live data dir untouched.
    let extract = || -> anyhow::Result<bool> {
        let f =
            std::fs::File::open(archive).with_context(|| format!("abrir {}", archive.display()))?;
        let gz = flate2::read::GzDecoder::new(f);
        let mut tar = tar::Archive::new(gz);

        let mut saw_surreal = false;
        for entry in tar.entries().context("leer entradas del archivo")? {
            let mut entry = entry.context("entrada inválida en el archivo")?;
            let entry_path = entry
                .path()
                .context("ruta inválida en el archivo")?
                .into_owned();
            let entry_path_str = entry_path.to_string_lossy();

            // Map `surreal/...` → `<staging>/surreal/...` and `agent.key` →
            // `<staging>/agent.key`; reject path-traversal entries.
            let dest = if entry_path_str.starts_with("surreal") {
                saw_surreal = true;
                staging.join(&*entry_path)
            } else if entry_path_str == "agent.key" {
                staging.join("agent.key")
            } else {
                continue;
            };
            if !dest.starts_with(&staging) {
                return Err(anyhow!(
                    "Entrada con ruta no permitida en el archivo: {entry_path_str}"
                ));
            }

            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("crear directorio {}", parent.display()))?;
            }
            if entry.header().entry_type().is_file() {
                let mut out = std::fs::File::create(&dest)
                    .with_context(|| format!("crear {}", dest.display()))?;
                std::io::copy(&mut entry, &mut out)
                    .with_context(|| format!("escribir {}", dest.display()))?;
            }
        }
        Ok(saw_surreal)
    };

    let result = extract();
    let saw_surreal = match result {
        Ok(true) => true,
        Ok(false) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(anyhow!(
                "El archivo no contiene datos de SurrealKv (carpeta `surreal/`); \
                 restauración abortada, los datos actuales no fueron tocados."
            ));
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(
                e.context("archivo inválido o corrupto; los datos actuales no fueron tocados")
            );
        }
    };
    debug_assert!(saw_surreal);

    // Staging validated.  NOW swap into place.
    if db_path.exists() {
        std::fs::remove_dir_all(db_path)
            .with_context(|| format!("limpiar {}", db_path.display()))?;
    }
    std::fs::rename(staging.join("surreal"), db_path).with_context(|| {
        format!(
            "mover {} → {}",
            staging.join("surreal").display(),
            db_path.display()
        )
    })?;

    let staged_key = staging.join("agent.key");
    if staged_key.exists() {
        let dest_key = data_dir.join("agent.key");
        std::fs::rename(&staged_key, &dest_key)
            .with_context(|| format!("mover {}", dest_key.display()))?;
    }

    let _ = std::fs::remove_dir_all(&staging);
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
        if let BackupCmd::Restore { path, yes, dry_run } = cli.cmd {
            assert_eq!(path, PathBuf::from("/tmp/snap.tar.gz"));
            assert!(!yes, "yes defaults to false");
            assert!(!dry_run, "dry_run defaults to false");
        } else {
            panic!("expected Restore");
        }
    }

    #[test]
    fn parse_backup_restore_dry_run() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "restore", "/tmp/snap.tar.gz", "--dry-run"]);
        if let BackupCmd::Restore { dry_run, yes, .. } = cli.cmd {
            assert!(dry_run, "--dry-run must set dry_run=true");
            assert!(!yes, "yes still defaults to false");
        } else {
            panic!("expected Restore");
        }
    }

    #[test]
    fn parse_backup_restore_yes() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        let cli = TestCli::parse_from(["pharma", "restore", "/tmp/snap.tar.gz", "--yes"]);
        if let BackupCmd::Restore { yes, dry_run, .. } = cli.cmd {
            assert!(yes, "--yes must set yes=true");
            assert!(!dry_run, "dry_run still defaults to false");
        } else {
            panic!("expected Restore");
        }
    }

    #[test]
    fn parse_backup_now_alias() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            cmd: super::BackupCmd,
        }

        // `now` is a visible alias of `create`.
        let cli = TestCli::parse_from(["pharma", "now"]);
        assert!(matches!(cli.cmd, BackupCmd::Create { output: None }));
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

        // Restore — pass a bind addr that is definitely not listening, --yes
        // to skip the interactive prompt.
        cmd_restore(&db_path, &archive, "127.0.0.1:19999", true, false).expect("restore");

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

        let err = cmd_restore(&db_path, &missing, "127.0.0.1:19999", true, false)
            .expect_err("should fail on missing archive");
        assert!(
            err.to_string().contains("Archivo no encontrado"),
            "unexpected error: {err}"
        );
    }

    // ── confirmation prompt ──────────────────────────────────────────────────

    #[test]
    fn confirm_accepts_affirmatives() {
        for ans in ["s\n", "S\n", "si\n", "sí\n", "y\n", "yes\n", "  s  \n"] {
            let mut r = std::io::Cursor::new(ans.as_bytes().to_vec());
            assert!(confirm_restore(&mut r), "{ans:?} should confirm");
        }
    }

    #[test]
    fn confirm_rejects_negatives_and_eof() {
        for ans in ["", "n\n", "no\n", "\n", "x\n", "yep\n"] {
            let mut r = std::io::Cursor::new(ans.as_bytes().to_vec());
            assert!(!confirm_restore(&mut r), "{ans:?} should NOT confirm");
        }
    }

    // ── validate-before-wipe restore ─────────────────────────────────────────

    #[test]
    fn restore_corrupt_archive_keeps_live_data() {
        let (_tmp, db_path) = make_fake_data_dir();
        // A garbage file that is NOT a valid tar.gz.
        let bad = _tmp.path().join("corrupt.tar.gz");
        std::fs::write(&bad, b"this is not a gzip archive at all").unwrap();

        let err =
            restore_archive(&db_path, &bad).expect_err("corrupt archive must fail to restore");
        assert!(
            err.to_string().contains("no fueron tocados")
                || err
                    .chain()
                    .any(|c| c.to_string().contains("no fueron tocados")),
            "error should state live data was untouched: {err:#}"
        );
        // Live data still intact.
        assert!(db_path.join("data.db").exists(), "live data.db preserved");
        assert_eq!(
            std::fs::read(db_path.join("data.db")).unwrap(),
            b"fake-surreal-data"
        );
        // No staging dir left behind.
        let leftovers: Vec<_> = std::fs::read_dir(_tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".restore-staging")
            })
            .collect();
        assert!(leftovers.is_empty(), "staging dir must be cleaned up");
    }

    #[test]
    fn restore_archive_without_surreal_keeps_live_data() {
        let (_tmp, db_path) = make_fake_data_dir();
        // Build a valid tar.gz that contains ONLY agent.key (no surreal/).
        let archive = _tmp.path().join("no-surreal.tar.gz");
        {
            let f = std::fs::File::create(&archive).unwrap();
            let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar = tar::Builder::new(gz);
            let mut kf = std::fs::File::open(_tmp.path().join("agent.key")).unwrap();
            tar.append_file("agent.key", &mut kf).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }

        let err = restore_archive(&db_path, &archive)
            .expect_err("archive without surreal/ must be rejected");
        assert!(
            err.to_string().contains("no contiene datos de SurrealKv"),
            "unexpected error: {err}"
        );
        assert!(db_path.join("data.db").exists(), "live data preserved");
    }

    #[test]
    fn restore_archive_valid_swaps_in_place() {
        let (_tmp, db_path) = make_fake_data_dir();
        let archive = _tmp.path().join("good.tar.gz");
        cmd_create(&db_path, Some(archive.clone())).expect("create");

        // Mutate live data so we can prove the restore replaced it.
        std::fs::write(db_path.join("data.db"), b"STALE-mutated").unwrap();

        restore_archive(&db_path, &archive).expect("valid restore");

        assert_eq!(
            std::fs::read(db_path.join("data.db")).unwrap(),
            b"fake-surreal-data",
            "restore must replace live data with archived contents"
        );
        assert!(_tmp.path().join("agent.key").exists());
        // No staging dir left behind.
        let leftovers: Vec<_> = std::fs::read_dir(_tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".restore-staging")
            })
            .collect();
        assert!(leftovers.is_empty(), "staging dir must be cleaned up");
    }

    // ── dry-run inspection ───────────────────────────────────────────────────

    #[test]
    fn dry_run_on_valid_archive_reports_and_touches_nothing() {
        let (_tmp, db_path) = make_fake_data_dir();
        let archive = _tmp.path().join("snap.tar.gz");
        cmd_create(&db_path, Some(archive.clone())).expect("create");

        // Mutate live data — dry-run must NOT overwrite it.
        std::fs::write(db_path.join("data.db"), b"LIVE-untouched").unwrap();

        // bind addr irrelevant for dry-run; pass a listening-looking one to prove
        // the server check is skipped in dry-run mode.
        cmd_restore(&db_path, &archive, "127.0.0.1:80", false, true).expect("dry-run ok");

        assert_eq!(
            std::fs::read(db_path.join("data.db")).unwrap(),
            b"LIVE-untouched",
            "dry-run must not overwrite live data"
        );
    }

    #[test]
    fn dry_run_missing_archive_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        let missing = tmp.path().join("nope.tar.gz");
        let err = cmd_restore(&db_path, &missing, "127.0.0.1:19999", false, true)
            .expect_err("dry-run on missing archive must error");
        assert!(err.to_string().contains("inspeccionar"), "got: {err:#}");
    }

    #[test]
    fn dry_run_archive_without_surreal_is_not_restorable() {
        let (_tmp, db_path) = make_fake_data_dir();
        // Valid tar.gz with ONLY agent.key (no surreal/ tree).
        let archive = _tmp.path().join("keyonly.tar.gz");
        {
            let f = std::fs::File::create(&archive).unwrap();
            let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar = tar::Builder::new(gz);
            let mut kf = std::fs::File::open(_tmp.path().join("agent.key")).unwrap();
            tar.append_file("agent.key", &mut kf).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let err = cmd_restore(&db_path, &archive, "127.0.0.1:19999", false, true)
            .expect_err("non-restorable snapshot must error in dry-run");
        assert!(
            err.to_string().contains("NO es restaurable"),
            "got: {err:#}"
        );
    }

    // ── real SurrealKv round-trip: stock-ledger invariant survives restore ────

    /// `(product_id, stock, Σ batch.stock, Σ movement.delta)` for every product
    /// of `tenant`, sorted by id for a deterministic before/after compare.
    async fn ledger_snapshot(
        db: &db::Db,
        tenant: &surrealdb::sql::Thing,
    ) -> Vec<(String, i64, i64, i64)> {
        #[derive(serde::Deserialize)]
        struct P {
            id: surrealdb::sql::Thing,
            stock: i64,
        }
        let mut r = db
            .query("SELECT id, stock FROM product WHERE tenant = $t")
            .bind(("t", tenant.clone()))
            .await
            .unwrap();
        let products: Vec<P> = r.take(0).unwrap();
        let mut out = Vec::new();
        for p in products {
            let mut br = db
                .query("SELECT VALUE stock FROM product_batch WHERE tenant = $t AND product = $p")
                .bind(("t", tenant.clone()))
                .bind(("p", p.id.clone()))
                .await
                .unwrap();
            let batch_stocks: Vec<i64> = br.take(0).unwrap();
            let mut mr = db
                .query("SELECT VALUE delta FROM stock_movement WHERE tenant = $t AND product = $p")
                .bind(("t", tenant.clone()))
                .bind(("p", p.id.clone()))
                .await
                .unwrap();
            let deltas: Vec<i64> = mr.take(0).unwrap();
            out.push((
                p.id.to_string(),
                p.stock,
                batch_stocks.iter().sum(),
                deltas.iter().sum(),
            ));
        }
        out.sort();
        out
    }

    /// End-to-end: seed a real file-backed SurrealKv store, snapshot it, wipe the
    /// data dir, restore, reopen — and prove the stock ledger invariant
    /// (`product.stock == Σ batch.stock == Σ movement.delta`) survives byte-for-
    /// byte. This is the real disaster-recovery path; the other tests use fake
    /// archive bytes and never reopen a DB.
    #[tokio::test]
    async fn restore_roundtrip_preserves_stock_ledger_invariant() {
        use pharma_core::config::DbConfig;
        use surrealdb::sql::Thing;

        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        let migrations =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations"));
        let cfg = DbConfig {
            path: db_path.to_string_lossy().into_owned(),
            namespace: "test".into(),
            database: "test".into(),
        };

        // 1) Seed a real SurrealKv store, capture the ledger.
        let tenant_id;
        let before;
        {
            let dbh = db::connect(&cfg).await.expect("connect surrealkv");
            db::run_migrations(&dbh, migrations).await.expect("migrate");
            let tenant: Thing = dbh
                .query("CREATE tenant SET name='T', slug='t' RETURN id")
                .await
                .unwrap()
                .take::<Option<Thing>>((0, "id"))
                .unwrap()
                .unwrap();
            domain::seed::seed_demo(&dbh, &tenant, "pharmacy", false)
                .await
                .expect("seed demo");
            before = ledger_snapshot(&dbh, &tenant).await;
            assert!(!before.is_empty(), "expected seeded products");
            tenant_id = tenant;
            drop(dbh); // release the SurrealKv file lock before snapshotting
        }

        // 2) Snapshot the data dir → tar.gz.
        let archive = tmp.path().join("snap.tar.gz");
        cmd_create(&db_path, Some(archive.clone())).expect("create snapshot");

        // 3) Disaster: wipe the live data dir.
        std::fs::remove_dir_all(&db_path).unwrap();
        assert!(!db_path.exists());

        // 4) Restore from the snapshot (validate-before-wipe path).
        restore_archive(&db_path, &archive).expect("restore");
        assert!(db_path.exists(), "data dir recreated by restore");

        // 5) Reopen the restored store and prove the ledger is identical and the
        //    invariant still holds.
        let dbh = db::connect(&cfg).await.expect("reconnect surrealkv");
        let after = ledger_snapshot(&dbh, &tenant_id).await;
        assert_eq!(
            before, after,
            "stock ledger must be identical after restore"
        );
        for (id, stock, batch_sum, delta_sum) in &after {
            assert_eq!(stock, batch_sum, "product.stock == Σ batch.stock ({id})");
            assert_eq!(stock, delta_sum, "product.stock == Σ movement.delta ({id})");
        }
    }
}
