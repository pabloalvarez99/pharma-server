use anyhow::Context;
use pharma_core::config::DbConfig;
use surrealdb::engine::local::{Db as LocalDb, SurrealKv};
use surrealdb::Surreal;

pub type Db = Surreal<LocalDb>;

pub async fn connect(cfg: &DbConfig) -> anyhow::Result<Db> {
    let db = Surreal::new::<SurrealKv>(cfg.path.as_str())
        .await
        .with_context(|| format!("opening surrealkv at {}", cfg.path))?;
    db.use_ns(&cfg.namespace).use_db(&cfg.database).await?;
    tracing::info!(ns = %cfg.namespace, db = %cfg.database, path = %cfg.path, engine = "surrealkv", "surrealdb connected");
    Ok(db)
}
