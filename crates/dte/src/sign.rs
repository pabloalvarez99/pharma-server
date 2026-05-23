//! Firma del XML completo del DTE con el cert digital de la empresa
//! (XML-DSig: SignedInfo C14N + SignatureValue RSA-SHA1 + KeyInfo X509).
//!
//! ## Estado actual
//!
//! Subtask 9.1.b (sign XML DSig completo) **deferida** dentro de la sesión
//! 9.1.d/e/i — el sello byte-exacto de C14N (`http://www.w3.org/TR/2001/REC-xml-
//! c14n-20010315`) + PKCS#12 → RSA priv decode + KeyInfo embed requiere
//! tiempo dedicado adicional. La cripto está disponible (`rsa`, `sha1`,
//! `p12`, `x509-cert` ya en `Cargo.toml`); falta el ensamble del XML DSig
//! `<SignedInfo>` con C14N exacto que SII validará en el servidor.
//!
//! Las funciones expuestas devuelven `DteError::SignFailed` con mensaje
//! explícito hasta que el subtask sign cierre. `sii::upload`/`estado`
//! propagan ese error con contexto cuando se invoca el flujo end-to-end
//! (las pruebas unitarias de `sii` con wiremock NO ejercitan este camino;
//! testan `SiiClient::upload_xml`/`query_estado` directamente con token
//! pre-fabricado).

use crate::types::{CertDigital, Dte};
use crate::DteError;

const PENDING_MSG: &str =
    "sign XML-DSig RSA-SHA1: deferido (cripto disponible, falta C14N byte-exact)";

/// Firma `xml_unsigned` con `cert` (PFX decifrado en memoria con
/// `cert::decrypt_for_sign(cert, tenant_id, master_key)`) y devuelve el XML
/// con `<Signature>` enmendado al final del `<Documento>`.
///
/// Implementación pendiente: parse PFX (p12 0.6) → RSA priv key + X509 →
/// SignedInfo (C14N exclusiva, SignatureMethod rsa-sha1, Reference URI=""
/// enveloped + c14n, DigestMethod sha1, DigestValue base64(SHA1(c14n))) →
/// SignatureValue base64(RSA-PKCS1v15-SHA1(c14n(SignedInfo))) → KeyInfo
/// X509Certificate.
pub fn sign_xml(
    _xml_unsigned: &str,
    _cert: &CertDigital,
    _master_key: &[u8; 32],
    _tenant_id: &str,
) -> Result<String, DteError> {
    Err(DteError::SignFailed(format!(
        "sign::sign_xml: {PENDING_MSG}"
    )))
}

/// Firma la semilla SII (`<getToken><item><Semilla>{seed}</Semilla></item>
/// <Signature>...</Signature></getToken>`) con el cert digital. Usada por
/// `sii::negotiate_token` para autenticarse y obtener el TOKEN de 60 min.
pub fn sign_seed_xml(
    _semilla: &str,
    _cert: &CertDigital,
    _master_key: &[u8; 32],
    _tenant_id: &str,
) -> Result<String, DteError> {
    Err(DteError::SignFailed(format!(
        "sign::sign_seed_xml: {PENDING_MSG}"
    )))
}

/// Wrapper end-to-end: renderiza unsigned, inyecta TED, firma, devuelve XML
/// listo para envío SII.
pub fn build_signed_dte(
    _dte: &Dte,
    _cert: &CertDigital,
    _master_key: &[u8; 32],
    _tenant_id: &str,
) -> Result<String, DteError> {
    Err(DteError::SignFailed(format!(
        "sign::build_signed_dte: {PENDING_MSG}"
    )))
}
