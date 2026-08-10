//! Dónde viven los bytes del respaldo.
//!
//! Tres implementaciones detrás de un trait chico:
//!
//! - [`NoStore`] — valida y no guarda. Es el default y es lo correcto para una
//!   instalación on-prem: los bytes del dueño no se van a la nube de nadie
//!   salvo que él lo pida.
//! - [`MemoryStore`] — laboratorio. Muere con el proceso.
//! - [`S3Store`] — R2 u otro S3-compatible. El de producción.
//!
//! **Ninguna de las tres puede leer un respaldo**: reciben un `Vec<u8>` que ya
//! salió cifrado del teléfono y lo devuelven igual. No hay una llave en este
//! archivo, ni una forma de pedirla.
//!
//! ## Lo que NO hace: listar el bucket
//!
//! No existe un `list()`. El índice de qué respaldos tiene cada negocio vive en
//! SurrealDB ([`super::repo`]), y es una decisión de costo además de una de
//! diseño: en R2 `ListObjects` es Class A (US$ 4,50 por millón) igual que un
//! `PUT`, mientras que `DeleteObject` es gratis. Listar el bucket en cada
//! subida para saber qué rotar duplicaría la parte cara de la factura.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::sigv4::{self, S3Credentials, SignRequest};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("el bucket no está configurado en este nodo")]
    NotConfigured,
    #[error("el bucket contestó {status}")]
    Upstream { status: u16 },
    #[error("no se pudo hablar con el bucket: {0}")]
    Transport(String),
    #[error("clave de objeto inválida")]
    BadKey,
}

/// Guardar, traer y borrar bytes opacos. Nada más.
#[async_trait::async_trait]
pub trait BlobStore: Send + Sync {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
    /// Para el `reason` honesto de la respuesta y para el health check.
    fn kind(&self) -> &'static str;

    /// ¿Este store **retiene** los bytes? `false` ⇒ la API contesta
    /// `accepted: false` sin inventar que guardó.
    ///
    /// Es distinto de [`BlobStore::durable`] y la diferencia importa: la
    /// memoria retiene (el sobre se puede volver a bajar en la misma corrida)
    /// pero no es durable (se va con el proceso). Si una sola bandera
    /// respondiera las dos preguntas, o el laboratorio no podría ejercitar el
    /// camino real, o un nodo sin bucket contestaría que guardó.
    fn guarda(&self) -> bool;

    /// ¿Sobrevive a un reinicio? Sólo el bucket de verdad. Se usa para avisar
    /// en el arranque y para no prometer nada en el health check.
    fn durable(&self) -> bool;
}

// --- clave del objeto ---------------------------------------------------------

/// Clave dentro del bucket: `<prefix><tenant>/<backup_id>`.
///
/// Los dos componentes se sanean a `[a-z0-9_-]`: el `tenant_id` viene del JWT y
/// el `backup_id` lo inventa el server, pero un id de SurrealDB trae `:` y un
/// slug puede traer cualquier cosa si algún día el alta deja de validarlo. Una
/// clave con `../` escaparía del prefijo y pisaría el respaldo de otro negocio,
/// y ese bug se paga con los datos de una persona, no con un 500.
pub fn object_key(prefix: &str, tenant_id: &str, backup_id: &str) -> Result<String, StoreError> {
    let t = sanitize_component(tenant_id);
    let b = sanitize_component(backup_id);
    if t.is_empty() || b.is_empty() {
        return Err(StoreError::BadKey);
    }
    let p = prefix.trim_start_matches('/');
    Ok(format!("{p}{t}/{b}"))
}

fn sanitize_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.trim().chars() {
        match c {
            'a'..='z' | '0'..='9' | '-' | '_' => out.push(c),
            'A'..='Z' => out.push(c.to_ascii_lowercase()),
            // `:` de los record ids de Surreal (`tenant:abc`) y cualquier otra
            // cosa se aplanan a `_`. Aplanar y no rechazar mantiene la clave
            // estable para tenants que ya existen.
            _ => out.push('_'),
        }
    }
    // Un componente que quedó sólo en separadores no identifica nada.
    if out.chars().all(|c| c == '_' || c == '-') {
        return String::new();
    }
    out
}

// --- none ---------------------------------------------------------------------

/// Valida y no guarda.
pub struct NoStore;

#[async_trait::async_trait]
impl BlobStore for NoStore {
    async fn put(&self, _key: &str, _bytes: Vec<u8>) -> Result<(), StoreError> {
        Err(StoreError::NotConfigured)
    }
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(None)
    }
    async fn delete(&self, _key: &str) -> Result<(), StoreError> {
        Ok(())
    }
    fn kind(&self) -> &'static str {
        "none"
    }
    fn guarda(&self) -> bool {
        false
    }
    fn durable(&self) -> bool {
        false
    }
}

// --- memoria (lab) ------------------------------------------------------------

/// Guarda en el proceso. Sirve para E2E sin R2; **no** es durabilidad.
#[derive(Default)]
pub struct MemoryStore {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.blobs.lock().map(|g| g.len()).unwrap_or(0)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Todo lo guardado, para que un test pueda mirar **los bytes en reposo** y
    /// no sólo lo que sale por HTTP. Sin esto, "el server no filtra plaintext"
    /// se afirmaría sobre las respuestas y no sobre lo que quedó escrito.
    pub fn contenido(&self) -> Vec<(String, Vec<u8>)> {
        self.blobs
            .lock()
            .map(|g| g.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl BlobStore for MemoryStore {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        let mut g = self
            .blobs
            .lock()
            .map_err(|_| StoreError::Transport("lock envenenado".into()))?;
        g.insert(key.to_string(), bytes);
        Ok(())
    }
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let g = self
            .blobs
            .lock()
            .map_err(|_| StoreError::Transport("lock envenenado".into()))?;
        Ok(g.get(key).cloned())
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let mut g = self
            .blobs
            .lock()
            .map_err(|_| StoreError::Transport("lock envenenado".into()))?;
        g.remove(key);
        Ok(())
    }
    fn kind(&self) -> &'static str {
        "memory"
    }
    fn guarda(&self) -> bool {
        // Sí retiene: dentro de la misma corrida el sobre sube y vuelve a
        // bajar. Es lo que deja probar el camino completo sin R2.
        true
    }
    fn durable(&self) -> bool {
        // Mentiría si dijera true: el proceso se reinicia y los respaldos no
        // están. Que el lab se vea como lab.
        false
    }
}

// --- S3 / R2 ------------------------------------------------------------------

pub struct S3Store {
    client: reqwest::Client,
    /// Sin barra final.
    endpoint: String,
    host: String,
    bucket: String,
    creds: S3Credentials,
}

impl S3Store {
    pub fn new(
        endpoint: &str,
        bucket: &str,
        creds: S3Credentials,
    ) -> Result<Self, StoreError> {
        let endpoint = endpoint.trim().trim_end_matches('/').to_string();
        let host = endpoint
            .split("://")
            .nth(1)
            .unwrap_or(&endpoint)
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string();
        if host.is_empty() || bucket.trim().is_empty() {
            return Err(StoreError::NotConfigured);
        }
        let client = reqwest::Client::builder()
            // Un respaldo que no sube no puede colgar el hilo de la app: el
            // teléfono reintenta, la venta ya está guardada en disco.
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| StoreError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            host,
            bucket: bucket.trim().to_string(),
            creds,
        })
    }

    fn canonical_uri(&self, key: &str) -> String {
        // Path-style: R2 y S3 aceptan `/<bucket>/<key>`.
        sigv4::uri_encode_path(&format!("/{}/{}", self.bucket, key))
    }

    fn url(&self, key: &str) -> String {
        format!("{}{}", self.endpoint, self.canonical_uri(key))
    }

    async fn send(
        &self,
        method: reqwest::Method,
        key: &str,
        body: Option<Vec<u8>>,
    ) -> Result<reqwest::Response, StoreError> {
        let payload_sha = match &body {
            Some(b) => sigv4::hex_sha256(b),
            None => sigv4::EMPTY_PAYLOAD_SHA256.to_string(),
        };
        let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let headers = sigv4::sign(
            &SignRequest {
                method: method.as_str(),
                canonical_uri: &self.canonical_uri(key),
                host: &self.host,
                payload_sha256_hex: &payload_sha,
                amz_date: &amz_date,
            },
            &self.creds,
        );
        let mut req = self.client.request(method, self.url(key));
        for (k, v) in headers {
            req = req.header(k, v);
        }
        if let Some(b) = body {
            req = req.body(b);
        }
        req.send()
            .await
            .map_err(|e| StoreError::Transport(e.to_string()))
    }
}

#[async_trait::async_trait]
impl BlobStore for S3Store {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        let r = self.send(reqwest::Method::PUT, key, Some(bytes)).await?;
        let status = r.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(StoreError::Upstream { status });
        }
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let r = self.send(reqwest::Method::GET, key, None).await?;
        let status = r.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&status) {
            return Err(StoreError::Upstream { status });
        }
        let bytes = r
            .bytes()
            .await
            .map_err(|e| StoreError::Transport(e.to_string()))?;
        Ok(Some(bytes.to_vec()))
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let r = self.send(reqwest::Method::DELETE, key, None).await?;
        let status = r.status().as_u16();
        // 204 al borrar; 404 = ya no estaba, que es el estado que se quería.
        if status == 404 || (200..300).contains(&status) {
            return Ok(());
        }
        Err(StoreError::Upstream { status })
    }

    fn kind(&self) -> &'static str {
        "s3"
    }
    fn guarda(&self) -> bool {
        true
    }
    fn durable(&self) -> bool {
        true
    }
}

/// Arma el store del nodo desde la config. Nunca falla el arranque: un bucket
/// mal configurado degrada a [`NoStore`] y lo dice en el log. Que el server no
/// levante porque el respaldo remoto está mal sería apagar el negocio entero
/// por un módulo opcional (ADR-0005).
pub fn build(cfg: &pharma_core::config::UserBackupConfig) -> Arc<dyn BlobStore> {
    match cfg.backend.as_str() {
        "memory" => {
            tracing::warn!(
                "user-backup: store en MEMORIA (lab). Los respaldos se pierden al reiniciar."
            );
            Arc::new(MemoryStore::new())
        }
        "s3" => {
            let creds = S3Credentials {
                access_key_id: cfg.access_key_id.trim().to_string(),
                secret_access_key: cfg.secret_access_key.trim().to_string(),
                region: cfg.region.trim().to_string(),
            };
            if creds.access_key_id.is_empty() || creds.secret_access_key.is_empty() {
                tracing::error!(
                    "user-backup: backend=s3 sin credenciales \
                     (PHARMA__USER_BACKUP__ACCESS_KEY_ID / __SECRET_ACCESS_KEY). \
                     El nodo NO guarda respaldos."
                );
                return Arc::new(NoStore);
            }
            match S3Store::new(&cfg.endpoint, &cfg.bucket, creds) {
                Ok(s) => {
                    tracing::info!(
                        bucket = %cfg.bucket,
                        endpoint = %cfg.endpoint,
                        "user-backup: bucket S3 conectado"
                    );
                    Arc::new(s)
                }
                Err(e) => {
                    tracing::error!(error = %e, "user-backup: bucket mal configurado; el nodo NO guarda");
                    Arc::new(NoStore)
                }
            }
        }
        _ => Arc::new(NoStore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_clave_no_puede_escapar_del_prefijo() {
        // Éste es el test que importa: un tenant hostil (o un id de Surreal con
        // caracteres raros) no puede escribir fuera de su carpeta ni pisar el
        // respaldo de otro negocio.
        let k = object_key("user-backup/", "../../otro", "abc").unwrap();
        assert!(!k.contains(".."), "{k}");
        assert!(k.starts_with("user-backup/"), "{k}");
        assert_eq!(k.matches('/').count(), 2, "{k}");

        let k2 = object_key("user-backup/", "tenant:abc", "mem-1-deadbeef").unwrap();
        assert_eq!(k2, "user-backup/tenant_abc/mem-1-deadbeef");
    }

    #[test]
    fn dos_tenants_distintos_no_colisionan() {
        let a = object_key("p/", "tenant:rosa", "b1").unwrap();
        let b = object_key("p/", "tenant:juan", "b1").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn una_clave_sin_nada_util_se_rechaza() {
        assert!(matches!(
            object_key("p/", "///", "b1"),
            Err(StoreError::BadKey)
        ));
        assert!(matches!(
            object_key("p/", "t", "  "),
            Err(StoreError::BadKey)
        ));
    }

    #[tokio::test]
    async fn el_store_de_memoria_devuelve_los_mismos_bytes() {
        let s = MemoryStore::new();
        assert!(s.get("x").await.unwrap().is_none());
        s.put("x", vec![1, 2, 3]).await.unwrap();
        assert_eq!(s.get("x").await.unwrap(), Some(vec![1, 2, 3]));
        s.delete("x").await.unwrap();
        assert!(s.get("x").await.unwrap().is_none());
        // Borrar algo que no está no es un error: el estado deseado ya es ése.
        s.delete("x").await.unwrap();
    }

    #[tokio::test]
    async fn el_store_none_no_promete_nada() {
        let s = NoStore;
        assert!(matches!(
            s.put("x", vec![1]).await,
            Err(StoreError::NotConfigured)
        ));
        assert!(s.get("x").await.unwrap().is_none());
        assert!(!s.durable());
    }

    #[test]
    fn ni_memoria_ni_none_se_declaran_durables() {
        // La respuesta al cliente sale de acá: si esto mintiera, la app diría
        // "guardado en la nube" sobre un store que se vacía al reiniciar.
        assert!(!MemoryStore::new().durable());
        assert!(!NoStore.durable());
    }

    #[test]
    fn config_a_medias_degrada_a_none_en_vez_de_tumbar_el_arranque() {
        let cfg = pharma_core::config::UserBackupConfig {
            backend: "s3".into(),
            bucket: "respaldos".into(),
            endpoint: "https://acct.r2.cloudflarestorage.com".into(),
            ..Default::default()
        };
        assert_eq!(build(&cfg).kind(), "none");
        assert!(!cfg.stores_anything());
        assert!(cfg.why_not_storing().unwrap().contains("a medias"));
    }

    #[test]
    fn config_completa_arma_el_store_s3() {
        let cfg = pharma_core::config::UserBackupConfig {
            backend: "s3".into(),
            bucket: "respaldos".into(),
            endpoint: "https://acct.r2.cloudflarestorage.com".into(),
            access_key_id: "AKIA".into(),
            secret_access_key: "shh".into(),
            ..Default::default()
        };
        let s = build(&cfg);
        assert_eq!(s.kind(), "s3");
        assert!(s.durable());
        assert!(cfg.stores_anything());
        assert!(cfg.why_not_storing().is_none());
    }
}
