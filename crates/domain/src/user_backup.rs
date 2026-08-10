//! User-held backup key (ADR-0022) — domain shapes + validation.
//!
//! Product decision: ciphertext lives in a RutBusiness bucket; the **key never
//! leaves the user's head / notebook**. Server stores opaque blobs only.
//!
//! Crypto (Argon2id + AES-GCM) runs **on the client**. This module freezes:
//! - wire shapes for upload / list / download metadata
//! - KDF + AEAD parameter names for `format_version = 1`
//! - pure validators so API and Android never diverge

use serde::{Deserialize, Serialize};

/// Current blob format. Bump when KDF / AEAD / packing changes.
pub const BACKUP_FORMAT_VERSION: u16 = 1;

/// Inner **plaintext snapshot** version (JSON before AES-GCM).
/// Distinct from [`BACKUP_FORMAT_VERSION`] (envelope crypto params).
pub const SNAPSHOT_FORMAT_VERSION: u16 = 1;

/// Documented path (API stub returns not-implemented until bucket is wired).
pub const USER_BACKUP_UPLOAD_PATH: &str = "/api/v1/user-backup";

/// Rescate sin sesión: `POST /api/v1/user-backup/rescue` (ADR-0023).
///
/// El único camino de vuelta para alguien cuyo teléfono ya no existe. No lleva
/// JWT porque quien perdió el teléfono no tiene forma de conseguir uno; lleva
/// la prueba de retiro derivada de la tarjeta del cuaderno.
pub const USER_BACKUP_RESCUE_PATH: &str = "/api/v1/user-backup/rescue";

/// List metadata for the tenant's opaque blobs.
pub const USER_BACKUP_LIST_PATH: &str = "/api/v1/user-backup";

/// Download one opaque blob: `GET /api/v1/user-backup/{backup_id}`.
pub const USER_BACKUP_DOWNLOAD_PREFIX: &str = "/api/v1/user-backup";

/// Known snapshot section keys (client packs; server never reads plaintext).
pub const SNAPSHOT_SECTION_PENDING_SALES: &str = "pending_sales";
pub const SNAPSHOT_SECTION_RUBRO: &str = "rubro";

// --- recovery phrase contract (client generates; server never sees plaintext) --

pub const RECOVERY_WORD_COUNT: usize = 12;
pub const RECOVERY_BLOCK_COUNT: usize = 8;
pub const RECOVERY_BLOCK_LEN: usize = 4;

/// Wire shape for "create recovery material" (client-local only for now).
///
/// The server must **never** receive the plaintext recovery phrase. Only a
/// verifier (hash) may be stored later if we add "did the user type the same
/// phrase" checks without holding the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMaterialPublic {
    pub tenant_id: String,
    pub created_at_unix: i64,
    pub label: Option<String>,
}

// --- format v1 crypto parameters (client-side; documented here for parity) ----

/// KDF algorithm name on the wire / in the envelope header.
///
/// Client Android today: **PBKDF2-HMAC-SHA256** (no NDK). Argon2id remains the
/// product target when a multiplatform lib lands; the envelope always labels
/// the real algorithm.
pub const KDF_ALG: &str = "pbkdf2-hmac-sha256";

/// Memory cost (KiB) for future Argon2id; ignored by PBKDF2 (still serialized).
pub const KDF_MEMORY_KIB: u32 = 65_536;

/// PBKDF2 iterations (OWASP ≥ 210_000 for SHA-256).
pub const KDF_ITERATIONS: u32 = 210_000;

/// Parallelism for future Argon2id; ignored by PBKDF2.
pub const KDF_PARALLELISM: u32 = 1;

/// Derived key length (bytes) → AES-256-GCM.
pub const KDF_OUTPUT_LEN: usize = 32;

/// AEAD algorithm name.
pub const AEAD_ALG: &str = "aes-256-gcm";

/// Salt length (bytes) stored with the envelope (not secret).
pub const KDF_SALT_LEN: usize = 16;

/// Nonce length for AES-GCM (bytes).
pub const AEAD_NONCE_LEN: usize = 12;

// --- prueba de retiro (rescate sin sesión) ------------------------------------
//
// El problema que resuelve: `GET /api/v1/user-backup/{id}` pide JWT, y alguien
// cuyo teléfono se perdió no tiene cómo conseguir uno. Un respaldo que sólo se
// puede bajar desde el aparato que se rompió no es un respaldo.
//
// La prueba de retiro es un segundo secreto derivado del **mismo** material del
// cuaderno, por un camino separado del de la llave de cifrado:
//
//   semilla            = 84 bits de la tarjeta (frase o bloques, ver Android
//                        `ClaveDelNegocio` — las dos formas dan los mismos bits)
//   salt_retiro        = SHA-256("rutbusiness-retiro:v1:" + tenant_slug)[..16]
//   clave_retiro       = PBKDF2-HMAC-SHA256(semilla, salt_retiro, 210_000, 32)
//   prueba_retiro      = HMAC-SHA256(clave_retiro, "rb1-retiro:v1:" + slug)
//   lo que guarda el server = SHA-256(prueba_retiro)
//
// Tres propiedades que sostienen el diseño:
//
// 1. **El server nunca puede descifrar.** `prueba_retiro` sale de una cadena
//    HMAC/PBKDF2 distinta de la llave AES, y las funciones son de una vía: con
//    la prueba (o con su hash) no se llega a la llave del sobre.
// 2. **El salt es determinista**, derivado del slug que está impreso en la
//    tarjeta. Tiene que serlo: el salt de la llave de cifrado vive DENTRO del
//    sobre, y para bajar el sobre hay que probar primero quién sos. Un salt
//    aleatorio sería un huevo dentro de su propia gallina.
// 3. **El margen de fuerza bruta no baja.** Si se filtra la base, el atacante
//    tiene SHA-256(prueba) y para llegar a la semilla tiene que pasar por el
//    mismo PBKDF2 de 210k: ~2^101 operaciones, el mismo número que protege al
//    sobre. Por eso la prueba se estira con PBKDF2 y no con un hash simple,
//    aunque para frenar adivinanzas online no haría falta.
//
// Costo: un PBKDF2 extra (~2 s en un teléfono de 2015). Se paga una vez y el
// resultado es estable para siempre, así que el cliente lo cachea y sólo lo
// recalcula en el teléfono nuevo — el día en que la persona está esperando
// igual.

/// Etiqueta de dominio del salt de la prueba de retiro. Cambiarla invalida
/// todas las pruebas ya registradas: es parte del formato, no un detalle.
pub const RETRIEVAL_SALT_PREFIX: &str = "rutbusiness-retiro:v1:";

/// Etiqueta de dominio del HMAC final. Separa la prueba de cualquier otro uso
/// futuro de `clave_retiro`.
pub const RETRIEVAL_HMAC_LABEL_PREFIX: &str = "rb1-retiro:v1:";

/// Salt (16 B) de la prueba de retiro para un slug — `SHA-256(prefijo||slug)`
/// truncado. Determinista y público: el slug está impreso en la tarjeta.
///
/// Vive en `domain` para que server y Android deriven **el mismo** salt. Si
/// divergen, la restauración falla el día del robo y no antes.
pub fn retrieval_salt(tenant_slug: &str) -> [u8; KDF_SALT_LEN] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(RETRIEVAL_SALT_PREFIX.as_bytes());
    h.update(normalize_slug(tenant_slug).as_bytes());
    let full = h.finalize();
    let mut out = [0u8; KDF_SALT_LEN];
    out.copy_from_slice(&full[..KDF_SALT_LEN]);
    out
}

/// Mensaje del HMAC final de la prueba de retiro.
pub fn retrieval_hmac_message(tenant_slug: &str) -> String {
    format!(
        "{}{}",
        RETRIEVAL_HMAC_LABEL_PREFIX,
        normalize_slug(tenant_slug)
    )
}

/// Slug canónico: minúsculas, sin espacios en los bordes. Que alguien escriba
/// "Puesto-Rosa " en el teléfono nuevo no puede costarle el respaldo.
pub fn normalize_slug(s: &str) -> String {
    s.trim().to_lowercase()
}

/// `SHA-256(prueba)` en hex — lo único que el server guarda de la prueba.
pub fn retrieval_hash_hex(proof: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(proof);
    hex_lower(&h.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

// --- upload wire shapes -------------------------------------------------------

/// Opaque backup object metadata (server sees only this + ciphertext bytes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedBackupMeta {
    pub tenant_id: String,
    /// Content version of the blob format (bump when AEAD params change).
    pub format_version: u16,
    /// SHA-256 of ciphertext (integrity, not confidentiality).
    pub ciphertext_sha256_hex: String,
    /// Byte length of ciphertext.
    pub size_bytes: u64,
    pub uploaded_at_unix: i64,
    /// Optional client-chosen label ("cuaderno 2026-08").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Server-assigned id when listing / after accept (opaque).
    /// Absent on client upload body (client does not invent ids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
}

/// Client → server upload body.
///
/// `ciphertext_base64` is the **only** payload bytes. No plaintext fields.
/// Server validates shape + sha256 match, then stores opaquely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEncryptedBackupRequest {
    pub meta: EncryptedBackupMeta,
    /// Standard base64 of the ciphertext envelope (see [`EnvelopeHeaderV1`]).
    pub ciphertext_base64: String,
    /// `SHA-256(prueba_retiro)` en hex — habilita el rescate sin sesión.
    ///
    /// Opcional **a propósito**: un cliente viejo que no la manda sigue
    /// subiendo igual, sólo que su sobre no se puede rescatar sin JWT. Hacerlo
    /// obligatorio rompería a las apps ya instaladas, que es exactamente a
    /// quien este carril viene a proteger.
    ///
    /// Nunca es la prueba en sí: el server guarda el hash y compara.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_hash_hex: Option<String>,
}

// --- rescate sin sesión -------------------------------------------------------

/// `POST /api/v1/user-backup/rescue` — teléfono nuevo, sin JWT.
///
/// Lo que la persona tiene: la tarjeta del cuaderno (slug + palabras/bloques).
/// El cliente deriva la prueba localmente y manda **la prueba**, nunca la
/// semilla ni la llave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescueBackupRequest {
    /// Slug del negocio, impreso en la tarjeta de rescate.
    pub tenant_slug: String,
    /// `prueba_retiro` en hex (32 bytes). El server hashea y compara.
    pub retrieval_proof_hex: String,
}

/// Respuesta del rescate: el sobre más nuevo del negocio.
///
/// Devuelve **sólo el más nuevo** y no una lista: una lista sería un oráculo
/// de enumeración (cuántos respaldos tiene, desde cuándo) para quien acierte
/// un slug, y para restaurar alcanza con el último.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescueBackupResponse {
    pub meta: EncryptedBackupMeta,
    pub ciphertext_base64: String,
    pub backup_id: String,
}

/// Por qué se rechaza un rescate. **Nunca se le cuenta al cliente cuál fue**:
/// todas salen como 404. Distinguir "ese negocio no existe" de "la prueba está
/// mal" le regala al que prueba slugs la mitad del trabajo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RescueRejection {
    BadSlug,
    BadProofHex,
    NoMatch,
}

/// Valida la forma del pedido de rescate (sin I/O). La prueba son 32 bytes en
/// hex; cualquier otra cosa no llega ni a consultar la base.
pub fn validate_rescue_request(req: &RescueBackupRequest) -> Result<Vec<u8>, RescueRejection> {
    if normalize_slug(&req.tenant_slug).is_empty() {
        return Err(RescueRejection::BadSlug);
    }
    let hexs = req.retrieval_proof_hex.trim();
    if hexs.len() != 64 || !hexs.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(RescueRejection::BadProofHex);
    }
    let mut out = Vec::with_capacity(32);
    let b = hexs.as_bytes();
    for pair in b.chunks(2) {
        let hi = (pair[0] as char).to_digit(16).ok_or(RescueRejection::BadProofHex)?;
        let lo = (pair[1] as char).to_digit(16).ok_or(RescueRejection::BadProofHex)?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

// --- cuotas -------------------------------------------------------------------

/// Límites del respaldo. Sin esto, la cuenta de costo del ADR-0023 es ficción y
/// el endpoint es hosting gratis para internet.
///
/// Los defaults salen de lo **medido** (`TamanoDelSobreTest`, 2026-08-09):
/// un sábado de feria son 65,8 KB y la cola al tope son 108 KB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupQuota {
    /// Tope por sobre. Default 4 MiB = 38x el techo real de hoy (108 KB), con
    /// lugar para que el snapshot crezca a llevar el negocio entero, y chico
    /// como para que el peor caso por tenant sea finito.
    pub max_envelope_bytes: u64,
    /// Versiones que se conservan. La nueva entra, la más vieja sale.
    /// 5 = una semana de feria. En R2 `DeleteObject` es gratis, así que rotar
    /// no cuesta nada; guardar para siempre sí.
    pub max_versions_per_tenant: u32,
    /// Piso de tiempo entre subidas del mismo tenant.
    ///
    /// Es control de **costo** antes que de abuso: a 1.000.000 de usuarios el
    /// 96% de la factura son los PUT (Class A, US$ 4,50 por millón), no los
    /// bytes. Una subida por día son US$ 1.643/año; una por hora, US$ 39.420.
    pub min_seconds_between_uploads: u64,
    /// Días que sobrevive un sobre sin que nadie lo toque. 0 = para siempre.
    pub retention_days: u32,
}

impl Default for BackupQuota {
    fn default() -> Self {
        Self {
            max_envelope_bytes: 4 * 1024 * 1024,
            max_versions_per_tenant: 5,
            min_seconds_between_uploads: 15 * 60,
            retention_days: 400,
        }
    }
}

/// Por qué una subida no entra por cuota (distinto de forma inválida).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaRejection {
    TooLarge { size: u64, max: u64 },
    TooSoon { wait_seconds: u64 },
}

impl QuotaRejection {
    pub fn message(&self) -> String {
        match self {
            Self::TooLarge { size, max } => format!(
                "el sobre pesa {size} bytes y el tope es {max}. \
                 Si tu negocio de verdad no cabe, escribinos: es un límite nuestro, no tuyo."
            ),
            Self::TooSoon { wait_seconds } => format!(
                "ya subiste un respaldo hace poco. Probá de nuevo en {} minuto(s). \
                 Lo que cobraste sigue guardado en el teléfono.",
                wait_seconds.div_ceil(60)
            ),
        }
    }

    /// Segundos que el cliente debe esperar (para `Retry-After`).
    pub fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            Self::TooSoon { wait_seconds } => Some(*wait_seconds),
            Self::TooLarge { .. } => None,
        }
    }
}

/// Chequea las cuotas de una subida. `last_upload_unix` es la fecha del sobre
/// más nuevo del tenant (`None` = primera subida).
pub fn check_quota(
    quota: &BackupQuota,
    envelope_len: u64,
    now_unix: i64,
    last_upload_unix: Option<i64>,
) -> Result<(), QuotaRejection> {
    if envelope_len > quota.max_envelope_bytes {
        return Err(QuotaRejection::TooLarge {
            size: envelope_len,
            max: quota.max_envelope_bytes,
        });
    }
    if quota.min_seconds_between_uploads > 0 {
        if let Some(last) = last_upload_unix {
            // Un reloj de teléfono adelantado deja `elapsed` negativo. Se trata
            // como "recién subió": no se puede confiar en una fecha del futuro
            // para abrir la puerta.
            let elapsed = now_unix.saturating_sub(last);
            let min = quota.min_seconds_between_uploads as i64;
            if elapsed < min {
                return Err(QuotaRejection::TooSoon {
                    wait_seconds: (min - elapsed).max(1) as u64,
                });
            }
        }
    }
    Ok(())
}

/// Cuáles sobran tras insertar uno nuevo. Recibe los ids ordenados **del más
/// nuevo al más viejo**, ya contando el recién subido, y devuelve los que hay
/// que borrar del bucket.
pub fn versions_to_evict(quota: &BackupQuota, newest_first: &[String]) -> Vec<String> {
    let keep = quota.max_versions_per_tenant.max(1) as usize;
    if newest_first.len() <= keep {
        return Vec::new();
    }
    newest_first[keep..].to_vec()
}

/// Server → client after accepting (or rejecting) an upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEncryptedBackupResponse {
    /// `true` only when the blob was persisted to the bucket.
    pub accepted: bool,
    /// Human reason when `accepted == false` (bucket missing, shape error…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Server-assigned id when accepted (opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
}

/// Server → client download body (ciphertext only + meta).
///
/// The recovery phrase is **never** on this wire. Client re-derives the key
/// locally from the notebook material + salt inside the envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadEncryptedBackupResponse {
    pub meta: EncryptedBackupMeta,
    pub ciphertext_base64: String,
    pub backup_id: String,
}

/// Header the client prefixes inside the ciphertext package **before** AEAD
/// encrypts the snapshot. Stored as plaintext JSON next to salt/nonce in the
/// outer envelope the server never interprets beyond length/hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvelopeHeaderV1 {
    pub format_version: u16,
    pub kdf: String,
    pub kdf_memory_kib: u32,
    pub kdf_iterations: u32,
    pub kdf_parallelism: u32,
    pub aead: String,
    /// Hex salt (32 hex chars for 16 bytes).
    pub salt_hex: String,
    /// Hex nonce (24 hex chars for 12 bytes).
    pub nonce_hex: String,
}

impl EnvelopeHeaderV1 {
    /// Canonical v1 header skeleton (caller fills salt/nonce hex).
    pub fn template(salt_hex: String, nonce_hex: String) -> Self {
        Self {
            format_version: BACKUP_FORMAT_VERSION,
            kdf: KDF_ALG.into(),
            kdf_memory_kib: KDF_MEMORY_KIB,
            kdf_iterations: KDF_ITERATIONS,
            kdf_parallelism: KDF_PARALLELISM,
            aead: AEAD_ALG.into(),
            salt_hex,
            nonce_hex,
        }
    }
}

// --- validation (pure) --------------------------------------------------------

/// True when a typed phrase has the expected word count (whitespace-split).
pub fn phrase_shape_ok(phrase: &str) -> bool {
    let n = phrase.split_whitespace().filter(|w| !w.is_empty()).count();
    n == RECOVERY_WORD_COUNT
}

/// True when a typed block string matches `XXXX-XXXX-…` (8×4 alnum).
pub fn blocks_shape_ok(blocks: &str) -> bool {
    let parts: Vec<&str> = blocks
        .split(|c: char| c == '-' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    parts.len() == RECOVERY_BLOCK_COUNT
        && parts.iter().all(|p| {
            p.len() == RECOVERY_BLOCK_LEN && p.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// Why an upload body is rejected (no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadValidationError {
    BadFormatVersion,
    EmptyTenant,
    BadSha256Hex,
    EmptyCiphertext,
    SizeMismatch { meta: u64, actual: u64 },
    Sha256Mismatch,
}

impl UploadValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::BadFormatVersion => format!(
                "format_version debe ser {BACKUP_FORMAT_VERSION} (v1 PBKDF2/AES-GCM; Argon2id futuro)"
            ),
            Self::EmptyTenant => "tenant_id vacío".into(),
            Self::BadSha256Hex => "ciphertext_sha256_hex debe ser 64 hex chars".into(),
            Self::EmptyCiphertext => "ciphertext_base64 vacío".into(),
            Self::SizeMismatch { meta, actual } => {
                format!("size_bytes={meta} no calza con ciphertext ({actual} bytes)")
            }
            Self::Sha256Mismatch => "sha256 del ciphertext no calza con meta".into(),
        }
    }
}

/// Validate upload **without** decoding crypto. `decoded_ciphertext` is the
/// raw bytes after base64 decode (caller decodes; domain stays free of base64).
pub fn validate_upload(
    meta: &EncryptedBackupMeta,
    decoded_ciphertext: &[u8],
    computed_sha256_hex: &str,
) -> Result<(), UploadValidationError> {
    if meta.format_version != BACKUP_FORMAT_VERSION {
        return Err(UploadValidationError::BadFormatVersion);
    }
    if meta.tenant_id.trim().is_empty() {
        return Err(UploadValidationError::EmptyTenant);
    }
    if !is_sha256_hex(&meta.ciphertext_sha256_hex) {
        return Err(UploadValidationError::BadSha256Hex);
    }
    if decoded_ciphertext.is_empty() {
        return Err(UploadValidationError::EmptyCiphertext);
    }
    let actual = decoded_ciphertext.len() as u64;
    if meta.size_bytes != actual {
        return Err(UploadValidationError::SizeMismatch {
            meta: meta.size_bytes,
            actual,
        });
    }
    if !eq_hex_ci(&meta.ciphertext_sha256_hex, computed_sha256_hex) {
        return Err(UploadValidationError::Sha256Mismatch);
    }
    Ok(())
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn eq_hex_ci(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.eq_ignore_ascii_case(&y))
}

/// Payload encoded in the rescue QR / printable card (no secrets of the server).
///
/// Format: `rutbusiness-rescue:v1:<tenant_slug>:<block0>-…-<block7>`
/// The phrase words are **not** put in the QR by default (too easy to photo-
/// share); blocks are shorter for the notebook. App restore accepts either.
pub fn rescue_qr_payload(tenant_slug: &str, bloques: &[String]) -> Option<String> {
    if bloques.len() != RECOVERY_BLOCK_COUNT {
        return None;
    }
    if !bloques.iter().all(|b| b.len() == RECOVERY_BLOCK_LEN) {
        return None;
    }
    let slug = tenant_slug.trim().to_lowercase();
    if slug.is_empty() {
        return None;
    }
    Some(format!(
        "rutbusiness-rescue:v1:{}:{}",
        slug,
        bloques.join("-")
    ))
}

/// Parse a rescue QR / typed payload. Returns `(tenant_slug, blocks joined)`.
pub fn parse_rescue_qr_payload(raw: &str) -> Option<(String, String)> {
    let s = raw.trim();
    let rest = s.strip_prefix("rutbusiness-rescue:v1:")?;
    let (slug, blocks) = rest.split_once(':')?;
    if slug.is_empty() || !blocks_shape_ok(blocks) {
        return None;
    }
    Some((slug.to_lowercase(), blocks.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_needs_twelve_words() {
        assert!(!phrase_shape_ok("uno dos"));
        assert!(phrase_shape_ok(
            "uno dos tres cuatro cinco seis siete ocho nueve diez once doce"
        ));
    }

    #[test]
    fn blocks_need_eight_of_four() {
        assert!(!blocks_shape_ok("AB3K"));
        assert!(blocks_shape_ok("AB3K-9F2Q-M7NP-4RST-WXY2-HJKL-QRST-VBNM"));
        assert!(blocks_shape_ok("AB3K 9F2Q M7NP 4RST WXY2 HJKL QRST VBNM"));
    }

    #[test]
    fn format_version_is_stable_v1() {
        assert_eq!(BACKUP_FORMAT_VERSION, 1);
        assert_eq!(SNAPSHOT_FORMAT_VERSION, 1);
        assert_eq!(KDF_ALG, "pbkdf2-hmac-sha256");
        assert_eq!(AEAD_ALG, "aes-256-gcm");
        assert_eq!(USER_BACKUP_UPLOAD_PATH, "/api/v1/user-backup");
        assert_eq!(SNAPSHOT_SECTION_PENDING_SALES, "pending_sales");
    }

    #[test]
    fn envelope_template_carries_v1_params() {
        let h = EnvelopeHeaderV1::template("aa".repeat(16), "bb".repeat(12));
        assert_eq!(h.format_version, 1);
        assert_eq!(h.kdf_memory_kib, KDF_MEMORY_KIB);
        assert_eq!(h.aead, AEAD_ALG);
    }

    #[test]
    fn validate_upload_happy_path() {
        let ct = b"not-real-ciphertext-but-long-enough";
        let sha = "a".repeat(64);
        let meta = EncryptedBackupMeta {
            tenant_id: "tenant:abc".into(),
            format_version: 1,
            ciphertext_sha256_hex: sha.clone(),
            size_bytes: ct.len() as u64,
            uploaded_at_unix: 1,
            label: None,
            backup_id: None,
        };
        assert!(validate_upload(&meta, ct, &sha).is_ok());
    }

    #[test]
    fn validate_upload_rejects_size_and_sha() {
        let ct = b"abc";
        let sha = "b".repeat(64);
        let meta = EncryptedBackupMeta {
            tenant_id: "t".into(),
            format_version: 1,
            ciphertext_sha256_hex: "a".repeat(64),
            size_bytes: 99,
            uploaded_at_unix: 1,
            label: None,
            backup_id: None,
        };
        assert!(matches!(
            validate_upload(&meta, ct, &sha),
            Err(UploadValidationError::SizeMismatch { .. })
        ));
        let meta2 = EncryptedBackupMeta {
            size_bytes: 3,
            ..meta
        };
        assert_eq!(
            validate_upload(&meta2, ct, &sha),
            Err(UploadValidationError::Sha256Mismatch)
        );
    }

    #[test]
    fn el_salt_de_retiro_es_estable_y_por_negocio() {
        // Estable: si este valor cambia, todas las pruebas ya registradas dejan
        // de servir y la gente que confió en su tarjeta no puede restaurar. Es
        // parte del formato, no un detalle de implementación.
        let a = retrieval_salt("puesto-rosa");
        let b = retrieval_salt("  Puesto-Rosa  ");
        assert_eq!(a, b, "el slug se normaliza: mayúsculas y espacios no cuentan");
        assert_ne!(
            retrieval_salt("puesto-rosa"),
            retrieval_salt("puesto-juan"),
            "dos negocios no pueden compartir salt"
        );
        assert_eq!(a.len(), KDF_SALT_LEN);
    }

    #[test]
    fn el_mensaje_del_hmac_lleva_el_slug_normalizado() {
        assert_eq!(
            retrieval_hmac_message(" Puesto-Rosa "),
            "rb1-retiro:v1:puesto-rosa"
        );
    }

    #[test]
    fn la_prueba_de_retiro_solo_viaja_hasheada() {
        let proof = [7u8; 32];
        let h = retrieval_hash_hex(&proof);
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        // Distinta prueba, distinto hash (obvio, pero es la propiedad que hace
        // que guardar el hash sea suficiente).
        assert_ne!(h, retrieval_hash_hex(&[8u8; 32]));
    }

    #[test]
    fn el_rescate_valida_la_forma_antes_de_tocar_la_base() {
        let ok = RescueBackupRequest {
            tenant_slug: "puesto-rosa".into(),
            retrieval_proof_hex: "ab".repeat(32),
        };
        assert_eq!(validate_rescue_request(&ok).unwrap().len(), 32);

        let sin_slug = RescueBackupRequest {
            tenant_slug: "   ".into(),
            ..ok.clone()
        };
        assert_eq!(
            validate_rescue_request(&sin_slug),
            Err(RescueRejection::BadSlug)
        );

        for malo in ["", "zz".repeat(32).as_str(), "ab".repeat(31).as_str()] {
            let r = RescueBackupRequest {
                tenant_slug: "puesto-rosa".into(),
                retrieval_proof_hex: malo.into(),
            };
            assert_eq!(
                validate_rescue_request(&r),
                Err(RescueRejection::BadProofHex),
                "prueba {malo:?}"
            );
        }
    }

    #[test]
    fn la_cuota_corta_el_sobre_gigante() {
        let q = BackupQuota::default();
        // El techo real medido hoy son 108 KB: tiene que pasar holgado.
        assert!(check_quota(&q, 108_073, 1_000, None).is_ok());
        assert_eq!(
            check_quota(&q, q.max_envelope_bytes + 1, 1_000, None),
            Err(QuotaRejection::TooLarge {
                size: q.max_envelope_bytes + 1,
                max: q.max_envelope_bytes
            })
        );
    }

    #[test]
    fn la_cuota_frena_la_subida_seguida_y_dice_cuanto_falta() {
        let q = BackupQuota::default();
        let last = 10_000i64;
        // Justo después: rechaza y dice el tiempo que falta.
        let e = check_quota(&q, 1_000, last + 60, Some(last)).unwrap_err();
        assert_eq!(
            e,
            QuotaRejection::TooSoon {
                wait_seconds: q.min_seconds_between_uploads - 60
            }
        );
        assert_eq!(e.retry_after_seconds(), Some(840));
        // El mensaje no puede sonar a "perdiste la venta".
        assert!(e.message().contains("sigue guardado en el teléfono"));
        // Pasado el piso: entra.
        assert!(check_quota(&q, 1_000, last + q.min_seconds_between_uploads as i64, Some(last)).is_ok());
    }

    #[test]
    fn un_reloj_del_futuro_no_abre_la_puerta() {
        // El teléfono del feriante puede tener la hora mal. Si el último sobre
        // quedó fechado en el futuro, `elapsed` sería negativo; tratarlo como
        // "pasó mucho tiempo" convertiría un reloj mal puesto en una subida
        // ilimitada.
        let q = BackupQuota::default();
        let r = check_quota(&q, 1_000, 1_000, Some(999_999));
        assert!(matches!(r, Err(QuotaRejection::TooSoon { .. })), "{r:?}");
    }

    #[test]
    fn la_rotacion_deja_las_mas_nuevas() {
        let q = BackupQuota {
            max_versions_per_tenant: 3,
            ..Default::default()
        };
        let ids: Vec<String> = (0..5).map(|i| format!("b{i}")).collect();
        // Entrada ordenada del más nuevo al más viejo.
        assert_eq!(versions_to_evict(&q, &ids), vec!["b3".to_string(), "b4".into()]);
        assert!(versions_to_evict(&q, &ids[..3]).is_empty());
        assert!(versions_to_evict(&q, &[]).is_empty());
    }

    #[test]
    fn rescue_qr_roundtrip() {
        let bloques = vec![
            "AB3K".into(),
            "9F2Q".into(),
            "M7NP".into(),
            "4RST".into(),
            "WXY2".into(),
            "HJKL".into(),
            "QRST".into(),
            "VBNM".into(),
        ];
        let payload = rescue_qr_payload("puesto-rosa", &bloques).expect("payload");
        assert!(payload.starts_with("rutbusiness-rescue:v1:puesto-rosa:"));
        let (slug, blocks) = parse_rescue_qr_payload(&payload).expect("parse");
        assert_eq!(slug, "puesto-rosa");
        assert!(blocks_shape_ok(&blocks));
    }
}
