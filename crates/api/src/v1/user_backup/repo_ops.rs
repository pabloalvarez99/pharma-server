//! Rotación y retención — lo que hace que "gratis para siempre" tenga un techo.
//!
//! Dos relojes distintos:
//!
//! * **Rotación** corre en cada subida y es por negocio: entra la nueva, sale
//!   la más vieja. Acota cuánto ocupa un tenant *activo*.
//! * **Retención** corre periódicamente y es global: barre sobres que nadie
//!   tocó en `retention_days`. Acota cuánto ocupa un tenant que **dejó de
//!   usar la app** — que si no, paga espacio para siempre por alguien que ya
//!   no está.
//!
//! Las dos borran el índice primero y el objeto después. Un objeto huérfano
//! cuesta unos centavos hasta la barrida siguiente; una fila que apunta a un
//! objeto borrado es una app que ofrece un respaldo que no existe.

use std::sync::Arc;

use surrealdb::sql::Thing;

use super::store::BlobStore;
use super::Runtime;

/// Deja como mucho `max_versions_per_tenant` sobres del negocio.
///
/// Best-effort a propósito: la subida **ya entró**. Si rotar falla (bucket
/// caído, conflicto de escritura), lo correcto es loguear y seguir — hacer
/// fallar el POST le diría a la dueña "no se guardó tu respaldo" cuando sí se
/// guardó, y la llevaría a reintentar, que es justo lo que se quería evitar.
/// El sobre de más lo barre la retención.
pub async fn rotate(rt: &Runtime, db: &Arc<db::Db>, tenant: &Thing) {
    let rows = match domain::user_backup_repo::list_newest_first(db, tenant).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "user-backup: no se pudo listar para rotar");
            return;
        }
    };
    let ids: Vec<String> = rows.iter().map(|r| r.backup_id.clone()).collect();
    let sobran = domain::user_backup::versions_to_evict(&rt.quota(), &ids);
    if sobran.is_empty() {
        return;
    }

    match domain::user_backup_repo::delete_rows(db, tenant, &sobran).await {
        Ok(keys) => {
            for k in keys {
                if let Err(e) = rt.store.delete(&k).await {
                    tracing::warn!(error = %e, key = %k, "user-backup: huérfano tras rotar");
                }
            }
            tracing::debug!(cuantos = sobran.len(), "user-backup: rotados");
        }
        Err(e) => tracing::warn!(error = %e, "user-backup: no se pudo rotar"),
    }
}

/// Barre los sobres más viejos que `retention_days`, de todos los negocios.
///
/// Devuelve cuántos borró. `retention_days = 0` no barre nada.
///
/// **Qué pasa con los bytes de alguien que deja de usar la app**: a los
/// `retention_days` (400 por default, o sea más de un año) se van. 400 y no 90
/// porque un puesto de feria puede estar meses parado —invierno, una
/// enfermedad, un viaje— y volver; borrarle el respaldo por no haber vendido en
/// tres meses sería castigar exactamente el caso que el respaldo cubre. Y no
/// "para siempre" porque un dato que nadie va a volver a pedir es costo y
/// riesgo, no servicio.
pub async fn sweep_expired(
    db: &Arc<db::Db>,
    store: &Arc<dyn BlobStore>,
    retention_days: u32,
) -> usize {
    if retention_days == 0 {
        return 0;
    }

    // Los contadores diarios de ayer para atrás ya no le sirven a nadie: la
    // cuota sólo mira el día en curso. Se limpian acá y no en su propio job
    // porque son basura del mismo tamaño y del mismo dueño que los sobres, y un
    // job más es una cosa más que se puede olvidar de andar.
    let ayer = domain::user_backup_repo::dia_utc(chrono::Utc::now() - chrono::Duration::days(1));
    if let Err(e) = domain::user_backup_repo::purgar_uso_anterior_a(db, &ayer).await {
        tracing::warn!(error = %e, "user-backup: no se pudieron purgar los contadores viejos");
    }

    let vencidos = match domain::user_backup_repo::expired(db, retention_days).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "user-backup: no se pudo listar vencidos");
            return 0;
        }
    };
    if vencidos.is_empty() {
        return 0;
    }
    let ids: Vec<String> = vencidos.iter().map(|(id, _)| id.clone()).collect();
    if let Err(e) = domain::user_backup_repo::delete_by_ids(db, &ids).await {
        tracing::warn!(error = %e, "user-backup: no se pudieron borrar vencidos del índice");
        return 0;
    }
    for (_, key) in &vencidos {
        if let Err(e) = store.delete(key).await {
            tracing::warn!(error = %e, key = %key, "user-backup: huérfano tras retención");
        }
    }
    tracing::info!(cuantos = vencidos.len(), dias = retention_days, "user-backup: retención");
    vencidos.len()
}
