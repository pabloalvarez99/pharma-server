//! Serialización por tenant de las secciones críticas que mutan inventario.
//!
//! # Por qué existe (BUG-003 / BUG-004)
//!
//! El MVCC de SurrealKv aborta la transacción perdedora de un write-write
//! concurrente al COMMIT con un conflicto *retryable* y —peor— filtra escrituras
//! parciales de la transacción multi-statement abortada, corrompiendo el
//! contador cacheado `product.stock` (60 ventas en paralelo → ~59 fallan con
//! DB_ERROR y `product.stock` lee 199 tras 2 commits desde 100 iniciales).
//! Serializar por tenant elimina el conflicto de raíz — mismo enfoque probado
//! que `crates/dte/src/caf.rs::ASSIGN_LOCK` para asignación de folios.
//!
//! # Por qué UN SOLO lock y no uno por flujo
//!
//! Venta, devolución y transferencia entre sucursales escriben las MISMAS filas:
//! `product`, `stock_movement` y —desde la migración 0041— la fila de
//! `product_branch_stock` que mantiene el evento `product_branch_stock_maint`.
//! Un lock por flujo (uno para ventas, otro para transferencias) NO serializa
//! venta-contra-transferencia: las dos entrarían a la vez a su sección crítica
//! sobre el mismo producto y reaparecería exactamente el conflicto write-write
//! que este mecanismo existe para eliminar. Todos los mutadores de inventario
//! comparten [`tenant_stock_lock`].
//!
//! Tenants distintos nunca comparten lock, así que el throughput multi-tenant no
//! se ve afectado; dentro de un tenant la demanda real (unos pocos cajeros +
//! transferencias esporádicas) está órdenes de magnitud por debajo del techo
//! serializado.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use surrealdb::sql::Thing;
use tokio::sync::Mutex as AsyncMutex;

type LockMap = std::sync::Mutex<HashMap<String, Arc<AsyncMutex<()>>>>;

static STOCK_LOCKS: OnceLock<LockMap> = OnceLock::new();

/// Lock compartido por tenant para toda sección crítica que mute inventario
/// (venta POS, devolución, transferencia entre sucursales).
///
/// El caller mantiene el guard mientras dura el check→commit y lo suelta apenas
/// la transacción cierra.
pub fn tenant_stock_lock(tenant: &Thing) -> Arc<AsyncMutex<()>> {
    let locks = STOCK_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("STOCK_LOCKS mutex poisoned");
    guard
        .entry(tenant.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}
