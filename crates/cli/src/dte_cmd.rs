//! `pharma dte`, `pharma caf`, `pharma cert` subcommands — Fase 9.1.k.
//!
//! Admin CLI para gestión local de DTE (Documentos Tributarios Electrónicos
//! SII Chile). Operaciones soportadas:
//!
//! - `dte list/show/export/cancel/stats` — explorar y exportar DTEs persistidos.
//! - `caf import/list/next` — gestión de Códigos de Autorización de Folios.
//! - `cert import/list/info` — gestión del cert digital (.pfx) encrypt-at-rest.
//!
//! Reuse del crate `dte` (`dte::caf::parse_xml`, `dte::cert::encrypt_at_rest`).
//! Acceso a DB via `db::connect` con la `AppConfig` cargada del proceso.
//! Resolución de tenant: explícito `--tenant <slug>`; si no, autoresolución a
//! la única tenant si existe; si no, error explícito.
//!
//! Operaciones destructivas (`dte cancel`) requieren `--confirm` literal — no
//! hay shorthand `-y`. Passphrase para `cert import` se lee solo via
//! `--passphrase-env VAR` (env var) o prompt oculto (`rpassword`); nunca como
//! flag plano para no contaminar el shell history.

use anyhow::{anyhow, bail, Context};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Args, Subcommand};
use db::Db;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use surrealdb::sql::Thing;

// ============================================================================
// Subcommand definitions (clap).
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum DteCmd {
    /// List DTEs filtrable por tenant, estado, tipo, rango fechas.
    List(DteListArgs),
    /// Show full detail of one DTE (XML, items, totales, track_id).
    Show(DteShowArgs),
    /// Export a DTE to file or stdout in XML or JSON.
    Export(DteExportArgs),
    /// Cancel a DTE (only transitions Draft|Signed → Cancelled).
    /// Requires `--confirm` explicitly.
    Cancel(DteCancelArgs),
    /// Snapshot estilo X (resumen mensual de DTE emitidos).
    Stats(DteStatsArgs),
}

#[derive(Args, Debug)]
pub struct DteListArgs {
    /// Tenant slug. If omitted and only one tenant exists, uses it.
    #[arg(long)]
    pub tenant: Option<String>,
    /// Filter by estado: draft|signed|sent|accepted|rejected|cancelled.
    #[arg(long)]
    pub estado: Option<String>,
    /// Filter by tipo SII (39|33|56|61|52).
    #[arg(long)]
    pub tipo: Option<i32>,
    /// Inclusive lower bound (YYYY-MM-DD, UTC).
    #[arg(long)]
    pub from: Option<String>,
    /// Inclusive upper bound (YYYY-MM-DD, UTC).
    #[arg(long)]
    pub to: Option<String>,
    /// Max rows (default 50).
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct DteShowArgs {
    /// DTE id (Surreal Thing string, e.g. `dte:abc123`).
    pub dte_id: String,
}

#[derive(Args, Debug)]
pub struct DteExportArgs {
    /// DTE id.
    pub dte_id: String,
    /// Output format: xml|json.
    #[arg(long)]
    pub format: String,
    /// Optional output file (default: stdout).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct DteCancelArgs {
    /// DTE id.
    pub dte_id: String,
    /// Razón de anulación (libre).
    #[arg(long)]
    pub reason: String,
    /// Confirmación explícita — destructive op, no `-y` shorthand.
    #[arg(long)]
    pub confirm: bool,
}

#[derive(Args, Debug)]
pub struct DteStatsArgs {
    #[arg(long)]
    pub tenant: Option<String>,
    /// Mes a resumir (YYYY-MM). Default: mes actual UTC.
    #[arg(long)]
    pub month: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum CafCmd {
    /// Parse + validar + persistir un CAF XML del SII.
    Import(CafImportArgs),
    /// Listar CAFs del tenant con folios restantes.
    List(CafListArgs),
    /// Peek del próximo folio (NO asigna).
    Next(CafNextArgs),
}

#[derive(Args, Debug)]
pub struct CafImportArgs {
    /// Path al CAF.xml entregado por SII.
    pub file: PathBuf,
    #[arg(long)]
    pub tenant: Option<String>,
}

#[derive(Args, Debug)]
pub struct CafListArgs {
    #[arg(long)]
    pub tenant: Option<String>,
}

#[derive(Args, Debug)]
pub struct CafNextArgs {
    #[arg(long)]
    pub tenant: Option<String>,
    /// Tipo DTE (39|33|56|61|52).
    #[arg(long)]
    pub tipo: i32,
}

#[derive(Subcommand, Debug)]
pub enum CertCmd {
    /// Import un .pfx encrypt-at-rest. Passphrase via --passphrase-env VAR
    /// o prompt oculto. NUNCA via flag plano.
    Import(CertImportArgs),
    /// List certs del tenant (RUT + vigencia). Nunca imprime el blob PFX.
    List(CertListArgs),
    /// Detalle del cert activo del tenant.
    Info(CertInfoArgs),
}

#[derive(Args, Debug)]
pub struct CertImportArgs {
    /// Path al cert.pfx.
    pub file: PathBuf,
    #[arg(long)]
    pub tenant: Option<String>,
    /// Nombre de la env var que contiene la passphrase. Si se omite, prompt
    /// oculto. La passphrase NUNCA viene por flag directo (shell history).
    #[arg(long)]
    pub passphrase_env: Option<String>,
}

#[derive(Args, Debug)]
pub struct CertListArgs {
    #[arg(long)]
    pub tenant: Option<String>,
}

#[derive(Args, Debug)]
pub struct CertInfoArgs {
    #[arg(long)]
    pub tenant: Option<String>,
}

// ============================================================================
// Persisted record shapes (subset — what CLI needs).
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantRow {
    id: Thing,
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DteRow {
    id: Thing,
    tenant: Thing,
    tipo: i32,
    folio: i64,
    fecha_emision: DateTime<Utc>,
    rut_emisor: String,
    rut_receptor: String,
    razon_social_receptor: String,
    monto_total: rust_decimal_for_cli::Decimal,
    estado: String,
    #[serde(default)]
    xml_firmado: Option<String>,
    #[serde(default)]
    track_id: Option<i64>,
    #[serde(default)]
    sii_glosa: Option<String>,
}

/// Local alias para no tener que añadir rust_decimal al manifest aparte —
/// reusa el re-export del crate `dte`.
mod rust_decimal_for_cli {
    pub use rust_decimal::Decimal;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CafRow {
    id: Thing,
    tenant: Thing,
    tipo_dte: i32,
    folio_desde: i64,
    folio_hasta: i64,
    next_folio: i64,
    fecha_autorizacion: DateTime<Utc>,
    rut_emisor: String,
    activo: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CertRow {
    id: Thing,
    tenant: Thing,
    rut_propietario: String,
    #[serde(default)]
    nombre_propietario: Option<String>,
    vigencia_desde: DateTime<Utc>,
    vigencia_hasta: DateTime<Utc>,
    activo: bool,
}

// ============================================================================
// Tenant resolution helpers.
// ============================================================================

async fn lookup_tenant(db: &Db, slug: &str) -> anyhow::Result<TenantRow> {
    let mut q = db
        .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await
        .context("lookup tenant by slug")?;
    let row: Option<TenantRow> = q.take(0)?;
    row.ok_or_else(|| anyhow!("tenant with slug '{slug}' not found"))
}

async fn resolve_tenant(db: &Db, explicit: Option<&str>) -> anyhow::Result<TenantRow> {
    if let Some(slug) = explicit {
        return lookup_tenant(db, slug).await;
    }
    let mut q = db
        .query("SELECT * FROM tenant LIMIT 2")
        .await
        .context("SELECT tenant")?;
    let rows: Vec<TenantRow> = q.take(0)?;
    match rows.len() {
        0 => Err(anyhow!(
            "no tenants in database — run `pharma tenant-create` first"
        )),
        1 => Ok(rows.into_iter().next().expect("len==1")),
        _ => Err(anyhow!(
            "multiple tenants found — ambiguous, pass --tenant <slug>"
        )),
    }
}

fn parse_date(s: &str, label: &str) -> anyhow::Result<DateTime<Utc>> {
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid --{label} '{s}' (expected YYYY-MM-DD)"))?;
    let dt = d
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid time"))?;
    Ok(Utc.from_utc_datetime(&dt))
}

// ============================================================================
// Entry point — dispatched from main.
// ============================================================================

pub async fn run_dte(cmd: DteCmd) -> anyhow::Result<()> {
    let cfg = pharma_core::config::AppConfig::load()?;
    let db = db::connect(&cfg.db).await?;
    match cmd {
        DteCmd::List(args) => dte_list(&db, args).await,
        DteCmd::Show(args) => dte_show(&db, args).await,
        DteCmd::Export(args) => dte_export(&db, args).await,
        DteCmd::Cancel(args) => dte_cancel(&db, args).await,
        DteCmd::Stats(args) => dte_stats(&db, args).await,
    }
}

pub async fn run_caf(cmd: CafCmd) -> anyhow::Result<()> {
    let cfg = pharma_core::config::AppConfig::load()?;
    let db = db::connect(&cfg.db).await?;
    match cmd {
        CafCmd::Import(args) => caf_import(&db, args).await,
        CafCmd::List(args) => caf_list(&db, args).await,
        CafCmd::Next(args) => caf_next(&db, args).await,
    }
}

pub async fn run_cert(cmd: CertCmd) -> anyhow::Result<()> {
    let cfg = pharma_core::config::AppConfig::load()?;
    let db = db::connect(&cfg.db).await?;
    match cmd {
        CertCmd::Import(args) => cert_import(&db, args).await,
        CertCmd::List(args) => cert_list(&db, args).await,
        CertCmd::Info(args) => cert_info(&db, args).await,
    }
}

// ============================================================================
// `dte ...` handlers.
// ============================================================================

async fn dte_list(db: &Db, args: DteListArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let mut sql = String::from("SELECT * FROM dte WHERE tenant = $tenant");
    if args.estado.is_some() {
        sql.push_str(" AND estado = $estado");
    }
    if args.tipo.is_some() {
        sql.push_str(" AND tipo = $tipo");
    }
    if args.from.is_some() {
        sql.push_str(" AND fecha_emision >= $from");
    }
    if args.to.is_some() {
        sql.push_str(" AND fecha_emision <= $to");
    }
    sql.push_str(" ORDER BY fecha_emision DESC LIMIT $limit");

    let mut q = db.query(sql).bind(("tenant", tenant.id.clone()));
    if let Some(e) = &args.estado {
        let estado = e.to_lowercase();
        if ![
            "draft",
            "signed",
            "sent",
            "accepted",
            "rejected",
            "cancelled",
        ]
        .contains(&estado.as_str())
        {
            bail!(
                "estado inválido '{estado}' (esperado: draft|signed|sent|accepted|rejected|cancelled)"
            );
        }
        q = q.bind(("estado", estado));
    }
    if let Some(t) = args.tipo {
        if dte::DteTipo::from_code(t).is_err() {
            bail!("tipo inválido '{t}' (esperado: 39|33|56|61|52)");
        }
        q = q.bind(("tipo", t));
    }
    if let Some(f) = &args.from {
        q = q.bind(("from", parse_date(f, "from")?));
    }
    if let Some(t) = &args.to {
        q = q.bind(("to", parse_date(t, "to")?));
    }
    q = q.bind(("limit", args.limit as i64));

    let mut res = q.await.context("SELECT dte")?;
    let rows: Vec<DteRow> = res.take(0)?;
    println!(
        "{:<40}  {:<4}  {:<8}  {:<10}  {:<12}  {:<14}  ESTADO",
        "ID", "TIPO", "FOLIO", "FECHA", "RUT_RECEPT", "MONTO"
    );
    for r in &rows {
        println!(
            "{:<40}  {:<4}  {:<8}  {:<10}  {:<12}  {:<14}  {}",
            r.id.to_string(),
            r.tipo,
            r.folio,
            r.fecha_emision.format("%Y-%m-%d"),
            r.rut_receptor,
            r.monto_total.to_string(),
            r.estado,
        );
    }
    println!("({} DTEs)", rows.len());
    Ok(())
}

async fn dte_show(db: &Db, args: DteShowArgs) -> anyhow::Result<()> {
    let thing = parse_thing(&args.dte_id, "dte")?;
    let mut res = db
        .query("SELECT * FROM $id")
        .bind(("id", thing))
        .await
        .context("SELECT dte by id")?;
    let row: Option<DteRow> = res.take(0)?;
    let row = row.ok_or_else(|| anyhow!("DTE not found: {}", args.dte_id))?;
    println!("{}", serde_json::to_string_pretty(&row)?);
    Ok(())
}

async fn dte_export(db: &Db, args: DteExportArgs) -> anyhow::Result<()> {
    let thing = parse_thing(&args.dte_id, "dte")?;
    let mut res = db
        .query("SELECT * FROM $id")
        .bind(("id", thing))
        .await
        .context("SELECT dte by id")?;
    let row: Option<DteRow> = res.take(0)?;
    let row = row.ok_or_else(|| anyhow!("DTE not found: {}", args.dte_id))?;
    let payload = match args.format.to_lowercase().as_str() {
        "xml" => row
            .xml_firmado
            .clone()
            .ok_or_else(|| anyhow!("DTE has no signed XML yet (estado={})", row.estado))?,
        "json" => serde_json::to_string_pretty(&row)?,
        other => bail!("format inválido '{other}' (esperado: xml|json)"),
    };
    if let Some(path) = args.out {
        std::fs::write(&path, payload).with_context(|| format!("write {}", path.display()))?;
        println!("exported to {}", path.display());
    } else {
        println!("{payload}");
    }
    Ok(())
}

async fn dte_cancel(_db: &Db, args: DteCancelArgs) -> anyhow::Result<()> {
    if !args.confirm {
        bail!(
            "destructive op — pass --confirm to cancel DTE {}",
            args.dte_id
        );
    }
    // TODO(fase-9.1.f): wire `dte::cancel::transition_to_cancelled` cuando
    // merge la branch `feat/dte-9-1-fgh-cancel-libro-xz`. Hoy es stub.
    bail!(
        "not yet wired — depends on branch feat/dte-9-1-fgh-cancel-libro-xz (reason recorded: {})",
        args.reason
    );
}

async fn dte_stats(db: &Db, args: DteStatsArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let (start, end) = month_bounds(args.month.as_deref())?;
    let mut res = db
        .query(
            "SELECT * FROM dte WHERE tenant = $tenant \
             AND fecha_emision >= $start AND fecha_emision < $end",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("start", start))
        .bind(("end", end))
        .await
        .context("SELECT dte stats")?;
    let rows: Vec<DteRow> = res.take(0)?;
    let mut by_estado: std::collections::BTreeMap<String, usize> = Default::default();
    let mut by_tipo: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut total_monto = rust_decimal::Decimal::ZERO;
    for r in &rows {
        *by_estado.entry(r.estado.clone()).or_default() += 1;
        *by_tipo.entry(r.tipo).or_default() += 1;
        total_monto += r.monto_total;
    }
    println!("Tenant:     {} ({})", tenant.slug, tenant.id);
    println!(
        "Período:    {} → {}",
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d")
    );
    println!("Total DTE:  {}", rows.len());
    println!("Monto:      ${}", total_monto);
    println!("Por estado:");
    for (k, v) in &by_estado {
        println!("  {:<12}  {}", k, v);
    }
    println!("Por tipo:");
    for (k, v) in &by_tipo {
        println!("  {:<4}          {}", k, v);
    }
    Ok(())
}

fn month_bounds(month: Option<&str>) -> anyhow::Result<(DateTime<Utc>, DateTime<Utc>)> {
    let (y, m) = match month {
        Some(s) => {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() != 2 {
                bail!("--month inválido '{s}' (esperado YYYY-MM)");
            }
            let y: i32 = parts[0].parse().context("year")?;
            let m: u32 = parts[1].parse().context("month")?;
            (y, m)
        }
        None => {
            let now = Utc::now();
            (
                now.format("%Y").to_string().parse()?,
                now.format("%m").to_string().parse()?,
            )
        }
    };
    let start_naive = NaiveDate::from_ymd_opt(y, m, 1)
        .ok_or_else(|| anyhow!("invalid YYYY-MM {y}-{m}"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("00:00:00"))?;
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end_naive = NaiveDate::from_ymd_opt(ny, nm, 1)
        .ok_or_else(|| anyhow!("invalid next month"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("00:00:00"))?;
    Ok((
        Utc.from_utc_datetime(&start_naive),
        Utc.from_utc_datetime(&end_naive),
    ))
}

// ============================================================================
// `caf ...` handlers.
// ============================================================================

async fn caf_import(db: &Db, args: CafImportArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let xml = std::fs::read_to_string(&args.file)
        .with_context(|| format!("read {}", args.file.display()))?;
    let caf = dte::caf::parse_xml(&xml).context("parse CAF XML")?;
    let mut res = db
        .query(
            "CREATE caf SET tenant = $tenant, tipo_dte = $tipo, \
             folio_desde = $desde, folio_hasta = $hasta, next_folio = $next, \
             fecha_autorizacion = $fa, rut_emisor = $rut, xml = $xml, \
             activo = $activo RETURN AFTER",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("tipo", caf.tipo_dte.code()))
        .bind(("desde", caf.folio_desde))
        .bind(("hasta", caf.folio_hasta))
        .bind(("next", caf.next_folio))
        .bind(("fa", caf.fecha_autorizacion))
        .bind(("rut", caf.rut_emisor.clone()))
        .bind(("xml", caf.xml.clone()))
        .bind(("activo", caf.activo))
        .await
        .context("CREATE caf")?;
    let row: Option<CafRow> = res.take(0)?;
    let row = row.ok_or_else(|| anyhow!("caf insert returned no row"))?;
    println!(
        "CAF imported: id={} tipo={} folios={}..{} rut={}",
        row.id, row.tipo_dte, row.folio_desde, row.folio_hasta, row.rut_emisor
    );
    Ok(())
}

async fn caf_list(db: &Db, args: CafListArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let mut res = db
        .query("SELECT * FROM caf WHERE tenant = $tenant ORDER BY tipo_dte, folio_desde")
        .bind(("tenant", tenant.id.clone()))
        .await
        .context("SELECT caf")?;
    let rows: Vec<CafRow> = res.take(0)?;
    println!(
        "{:<40}  {:<4}  {:<10}  {:<10}  {:<10}  {:<10}  ACTIVO",
        "ID", "TIPO", "DESDE", "HASTA", "NEXT", "RESTANTES"
    );
    for r in &rows {
        let restantes = if r.next_folio > r.folio_hasta {
            0
        } else {
            r.folio_hasta - r.next_folio + 1
        };
        println!(
            "{:<40}  {:<4}  {:<10}  {:<10}  {:<10}  {:<10}  {}",
            r.id.to_string(),
            r.tipo_dte,
            r.folio_desde,
            r.folio_hasta,
            r.next_folio,
            restantes,
            r.activo
        );
    }
    println!("({} CAFs)", rows.len());
    Ok(())
}

async fn caf_next(db: &Db, args: CafNextArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    if dte::DteTipo::from_code(args.tipo).is_err() {
        bail!("tipo inválido '{}' (esperado 39|33|56|61|52)", args.tipo);
    }
    let mut res = db
        .query(
            "SELECT * FROM caf WHERE tenant = $tenant AND tipo_dte = $tipo \
             AND activo = true AND next_folio <= folio_hasta \
             ORDER BY folio_desde ASC LIMIT 1",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("tipo", args.tipo))
        .await
        .context("SELECT caf peek")?;
    let rows: Vec<CafRow> = res.take(0)?;
    match rows.into_iter().next() {
        Some(r) => {
            println!(
                "Próximo folio: {} (CAF id={} tipo={} rango {}..{})",
                r.next_folio, r.id, r.tipo_dte, r.folio_desde, r.folio_hasta
            );
        }
        None => {
            println!(
                "No active CAF with folios available for tipo {}.",
                args.tipo
            );
        }
    }
    Ok(())
}

// ============================================================================
// `cert ...` handlers.
// ============================================================================

async fn cert_import(_db: &Db, args: CertImportArgs) -> anyhow::Result<()> {
    let _pfx_bytes =
        std::fs::read(&args.file).with_context(|| format!("read {}", args.file.display()))?;
    let _passphrase = match &args.passphrase_env {
        Some(var) => std::env::var(var)
            .with_context(|| format!("env var {var} not set (passphrase source)"))?,
        None => rpassword::prompt_password("PFX passphrase: ").context("read passphrase")?,
    };
    // TODO(fase-9.1.i): wire `dte::cert::encrypt_at_rest` cuando merge
    // `feat/dte-9-1-i-cert-encrypt`. Hoy `encrypt_at_rest` retorna stub error.
    bail!(
        "not yet wired — depends on branch feat/dte-9-1-i-cert-encrypt (cert encrypt-at-rest stub)"
    );
}

async fn cert_list(db: &Db, args: CertListArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let mut res = db
        .query(
            "SELECT id, tenant, rut_propietario, nombre_propietario, \
             vigencia_desde, vigencia_hasta, activo \
             FROM cert_digital WHERE tenant = $tenant ORDER BY vigencia_desde DESC",
        )
        .bind(("tenant", tenant.id.clone()))
        .await
        .context("SELECT cert_digital")?;
    let rows: Vec<CertRow> = res.take(0)?;
    println!(
        "{:<40}  {:<14}  {:<12}  {:<12}  ACTIVO",
        "ID", "RUT", "VIG_DESDE", "VIG_HASTA"
    );
    for r in &rows {
        println!(
            "{:<40}  {:<14}  {:<12}  {:<12}  {}",
            r.id.to_string(),
            r.rut_propietario,
            r.vigencia_desde.format("%Y-%m-%d"),
            r.vigencia_hasta.format("%Y-%m-%d"),
            r.activo
        );
    }
    println!("({} certs)", rows.len());
    Ok(())
}

async fn cert_info(db: &Db, args: CertInfoArgs) -> anyhow::Result<()> {
    let tenant = resolve_tenant(db, args.tenant.as_deref()).await?;
    let mut res = db
        .query(
            "SELECT id, tenant, rut_propietario, nombre_propietario, \
             vigencia_desde, vigencia_hasta, activo \
             FROM cert_digital WHERE tenant = $tenant AND activo = true LIMIT 1",
        )
        .bind(("tenant", tenant.id.clone()))
        .await
        .context("SELECT cert_digital active")?;
    let row: Option<CertRow> = res.take(0)?;
    match row {
        Some(r) => {
            println!("ID:              {}", r.id);
            println!("RUT:             {}", r.rut_propietario);
            if let Some(n) = &r.nombre_propietario {
                println!("Nombre:          {n}");
            }
            println!("Vigencia desde:  {}", r.vigencia_desde);
            println!("Vigencia hasta:  {}", r.vigencia_hasta);
            println!("Activo:          {}", r.activo);
        }
        None => println!("No active cert for tenant '{}'.", tenant.slug),
    }
    Ok(())
}

// ============================================================================
// Utilities.
// ============================================================================

fn parse_thing(s: &str, expected_table: &str) -> anyhow::Result<Thing> {
    let (table, key) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid id '{s}' (expected '{expected_table}:<key>')"))?;
    if table != expected_table {
        bail!("id table is '{table}', expected '{expected_table}'");
    }
    Ok(Thing::from((table, key)))
}
