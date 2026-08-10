//! Índice de respaldos cifrados (migración 0047) — la parte que toca la base.
//!
//! Separado de [`crate::user_backup`] a propósito: aquel módulo es puro
//! (formas de wire + validadores) y es el contrato que Android replica. Este
//! habla con SurrealDB y no lo replica nadie.
//!
//! **Los bytes no están acá.** Esta tabla guarda punteros al bucket y metadatos
//! públicos (tamaño, sha del ciphertext, fecha). Nada de lo que devuelve
//! permite abrir un respaldo: la llave la tiene la dueña en el cuaderno.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};
use crate::user_backup::EncryptedBackupMeta;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Una fila del índice. `retrieval_hash` no sale nunca hacia el cliente: se usa
/// para comparar, y devolverlo convertiría el listado en el material que hace
/// falta para atacar la prueba offline.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackupRow {
    pub backup_id: String,
    pub object_key: String,
    pub format_version: u16,
    pub ciphertext_sha256: String,
    pub size_bytes: u64,
    pub label: Option<String>,
    pub uploaded_at: DateTime<Utc>,
}

/// Columnas que puede ver un cliente. Excluye `retrieval_hash` y el `tenant`.
const CLIENT_COLS: &str =
    "backup_id, object_key, format_version, ciphertext_sha256, size_bytes, label, uploaded_at";

impl BackupRow {
    /// A la forma de wire que ya conoce la app.
    pub fn to_meta(&self, tenant_id: &str) -> EncryptedBackupMeta {
        EncryptedBackupMeta {
            tenant_id: tenant_id.to_string(),
            format_version: self.format_version,
            ciphertext_sha256_hex: self.ciphertext_sha256.clone(),
            size_bytes: self.size_bytes,
            uploaded_at_unix: self.uploaded_at.timestamp(),
            label: self.label.clone(),
            backup_id: Some(self.backup_id.clone()),
        }
    }
}

/// Lo que hay que anotar al aceptar una subida.
pub struct NewBackup<'a> {
    pub backup_id: &'a str,
    pub object_key: &'a str,
    pub format_version: u16,
    pub ciphertext_sha256: &'a str,
    pub size_bytes: u64,
    pub label: Option<&'a str>,
    /// `SHA-256(prueba_retiro)` en hex. `None` = sobre sin rescate sin sesión.
    pub retrieval_hash: Option<&'a str>,
}

/// Anota un sobre nuevo.
pub async fn insert(db: &Db, tenant: &Thing, nuevo: &NewBackup<'_>) -> DomainResult<BackupRow> {
    let q = format!(
        "CREATE user_backup SET tenant = $t, backup_id = $bid, object_key = $key, \
         format_version = $fv, ciphertext_sha256 = $sha, size_bytes = $size, \
         label = $label, retrieval_hash = $rh RETURN {CLIENT_COLS}"
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("bid", nuevo.backup_id.to_string()))
        .bind(("key", nuevo.object_key.to_string()))
        .bind(("fv", nuevo.format_version as i64))
        .bind(("sha", nuevo.ciphertext_sha256.to_string()))
        .bind(("size", nuevo.size_bytes as i64))
        .bind(("label", nuevo.label.map(|s| s.to_string())))
        .bind(("rh", nuevo.retrieval_hash.map(|s| s.to_string())))
        .await?
        .check()?;
    let row: Option<BackupRow> = r.take(0)?;
    row.ok_or(DomainError::NotFound)
}

/// Los sobres de un negocio, del más nuevo al más viejo.
pub async fn list_newest_first(db: &Db, tenant: &Thing) -> DomainResult<Vec<BackupRow>> {
    let q = format!(
        "SELECT {CLIENT_COLS} FROM user_backup WHERE tenant = $t ORDER BY uploaded_at DESC"
    );
    let mut r = db.query(q).bind(("t", tenant.clone())).await?;
    Ok(r.take(0)?)
}

/// Cuándo entró el último sobre — la cuota de frecuencia se apoya en esto.
pub async fn last_upload(db: &Db, tenant: &Thing) -> DomainResult<Option<DateTime<Utc>>> {
    let mut r = db
        .query("SELECT uploaded_at FROM user_backup WHERE tenant = $t ORDER BY uploaded_at DESC LIMIT 1")
        .bind(("t", tenant.clone()))
        .await?;
    #[derive(Deserialize)]
    struct SoloFecha {
        uploaded_at: DateTime<Utc>,
    }
    let row: Option<SoloFecha> = r.take(0)?;
    Ok(row.map(|x| x.uploaded_at))
}

// --- contador diario (migración 0048) -----------------------------------------

/// Día UTC en `YYYY-MM-DD`, que es la clave del contador.
pub fn dia_utc(cuando: DateTime<Utc>) -> String {
    cuando.format("%Y-%m-%d").to_string()
}

/// Cuántos sobres subió el negocio hoy.
///
/// **No** se cuenta `user_backup`: la rotación borra las filas viejas, así que
/// contar ahí topea en `max_versions_per_tenant` y el tope diario nunca se
/// dispararía. El contador es la única evidencia que la rotación no pisa (ver
/// el encabezado de `migrations/0048_user_backup_uso_diario.surql`).
pub async fn uploads_hoy(db: &Db, tenant: &Thing, dia: &str) -> DomainResult<u32> {
    let mut r = db
        .query("SELECT subidas FROM user_backup_uso WHERE tenant = $t AND dia = $d LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("d", dia.to_string()))
        .await?;
    #[derive(Deserialize)]
    struct SoloCuenta {
        subidas: i64,
    }
    let row: Option<SoloCuenta> = r.take(0)?;
    Ok(row.map(|x| x.subidas.max(0) as u32).unwrap_or(0))
}

/// Suma uno al contador del día. Se llama **después** de que el PUT salió bien.
///
/// Contar después y no antes es deliberado: un PUT que falla no cuesta Class A
/// y no tiene por qué gastarle el cupo a la dueña. El costo de equivocarse para
/// este lado es que un atacante que provoque errores de bucket no consume su
/// propio tope — pero tampoco genera factura, que es lo que el tope protege.
pub async fn contar_subida(db: &Db, tenant: &Thing, dia: &str) -> DomainResult<()> {
    // UPSERT sobre el índice UNIQUE (tenant, dia): crea la fila la primera vez
    // del día y suma en las siguientes, en una sola ida a la base.
    db.query(
        "UPSERT user_backup_uso SET tenant = $t, dia = $d, \
         subidas = (subidas ?? 0) + 1 WHERE tenant = $t AND dia = $d",
    )
    .bind(("t", tenant.clone()))
    .bind(("d", dia.to_string()))
    .await?
    .check()?;
    Ok(())
}

/// Borra contadores más viejos que [`dia`]. La barrida de retención los limpia
/// junto con los sobres; sin esto la tabla crece una fila por negocio por día
/// para siempre.
pub async fn purgar_uso_anterior_a(db: &Db, dia: &str) -> DomainResult<()> {
    db.query("DELETE user_backup_uso WHERE dia < $d")
        .bind(("d", dia.to_string()))
        .await?
        .check()?;
    Ok(())
}

/// Un sobre del negocio por id. `None` si no existe **o si es de otro tenant**:
/// misma respuesta a propósito, para que nadie enumere ids ajenos.
pub async fn find_for_tenant(
    db: &Db,
    tenant: &Thing,
    backup_id: &str,
) -> DomainResult<Option<BackupRow>> {
    let q = format!(
        "SELECT {CLIENT_COLS} FROM user_backup WHERE tenant = $t AND backup_id = $bid LIMIT 1"
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("bid", backup_id.to_string()))
        .await?;
    Ok(r.take(0)?)
}

/// El sobre más nuevo que calza con un hash de prueba de retiro, dentro de un
/// negocio identificado por slug. Es el corazón del rescate sin sesión.
///
/// La consulta pide **las dos cosas**: el slug (que está impreso en la tarjeta)
/// y el hash de la prueba (que sale de las palabras). Con el slug solo no
/// alcanza; con la prueba sola tampoco.
pub async fn find_newest_by_retrieval(
    db: &Db,
    tenant_slug: &str,
    retrieval_hash: &str,
) -> DomainResult<Option<(Thing, BackupRow)>> {
    let q = format!(
        "SELECT tenant, {CLIENT_COLS} FROM user_backup \
         WHERE retrieval_hash = $rh AND tenant.slug = $slug \
         ORDER BY uploaded_at DESC LIMIT 1"
    );
    #[derive(Deserialize)]
    struct ConTenant {
        tenant: Thing,
        #[serde(flatten)]
        row: BackupRow,
    }
    let mut r = db
        .query(q)
        .bind(("rh", retrieval_hash.to_string()))
        .bind(("slug", tenant_slug.to_string()))
        .await?;
    let found: Option<ConTenant> = r.take(0)?;
    Ok(found.map(|c| (c.tenant, c.row)))
}

/// Borra las filas de estos ids y devuelve las claves de bucket a limpiar.
///
/// Se borra el índice **primero** y el objeto después, y ése orden importa: un
/// objeto huérfano en el bucket cuesta unos centavos hasta la barrida de
/// retención, mientras que una fila que apunta a un objeto ya borrado es un
/// respaldo que la app ofrece y que al bajarlo no está — o sea, mentirle a
/// alguien sobre que tiene respaldo.
pub async fn delete_rows(db: &Db, tenant: &Thing, backup_ids: &[String]) -> DomainResult<Vec<String>> {
    if backup_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = db
        .query(
            "DELETE user_backup WHERE tenant = $t AND backup_id IN $ids \
             RETURN BEFORE",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", backup_ids.to_vec()))
        .await?
        .check()?;
    #[derive(Deserialize)]
    struct SoloKey {
        object_key: String,
    }
    let rows: Vec<SoloKey> = r.take(0)?;
    Ok(rows.into_iter().map(|x| x.object_key).collect())
}

/// Todo lo de un negocio — "borrá mis datos". Devuelve las claves del bucket.
pub async fn delete_all_for_tenant(db: &Db, tenant: &Thing) -> DomainResult<Vec<String>> {
    let mut r = db
        .query("DELETE user_backup WHERE tenant = $t RETURN BEFORE")
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    #[derive(Deserialize)]
    struct SoloKey {
        object_key: String,
    }
    let rows: Vec<SoloKey> = r.take(0)?;
    Ok(rows.into_iter().map(|x| x.object_key).collect())
}

/// Sobres más viejos que `dias`, de **todos** los tenants — barrida de
/// retención. Devuelve `(backup_id, object_key)` para poder borrar los dos
/// lados.
pub async fn expired(db: &Db, dias: u32) -> DomainResult<Vec<(String, String)>> {
    if dias == 0 {
        return Ok(Vec::new());
    }
    let corte = Utc::now() - chrono::Duration::days(dias as i64);
    // `sql::Datetime` y no el `DateTime<Utc>` pelado: un chrono crudo se
    // serializa como string y `uploaded_at < $corte` compara datetime contra
    // string, que nunca es cierto — la barrida no barrería nada y no fallaría.
    // Es la convención del resto del repo (ver `expenses::service`).
    let mut r = db
        .query("SELECT backup_id, object_key FROM user_backup WHERE uploaded_at < $corte")
        .bind(("corte", surrealdb::sql::Datetime::from(corte)))
        .await?;
    #[derive(Deserialize)]
    struct Fila {
        backup_id: String,
        object_key: String,
    }
    let rows: Vec<Fila> = r.take(0)?;
    Ok(rows.into_iter().map(|f| (f.backup_id, f.object_key)).collect())
}

/// Borra por id sin filtrar tenant — sólo para la barrida de retención, que ya
/// eligió las filas con [`expired`].
pub async fn delete_by_ids(db: &Db, backup_ids: &[String]) -> DomainResult<()> {
    if backup_ids.is_empty() {
        return Ok(());
    }
    db.query("DELETE user_backup WHERE backup_id IN $ids")
        .bind(("ids", backup_ids.to_vec()))
        .await?
        .check()?;
    Ok(())
}
