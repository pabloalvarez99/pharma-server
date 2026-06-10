//! Firma XML-DSig del `<Documento>` del DTE con el cert digital de la empresa
//! — subtask **9.1.b.2** (enveloped signature, RSA-SHA1).
//!
//! ## Qué firma esto (vs el TED de `timbre.rs`)
//!
//! Un DTE despachable al SII lleva DOS firmas RSA-SHA1 distintas:
//!
//! 1. **TED** (`timbre::generate`): firma el bloque `<DD>` con la clave RSASK
//!    embebida en el CAF. Cubre folio + montos + receptor. Ya implementado.
//! 2. **`<Signature>` XML-DSig** (este módulo): firma enveloped sobre el
//!    `<Documento ID="...">` completo (que ya incluye el TED inyectado) con
//!    la clave privada del **cert digital de la empresa** (el PFX que el SII
//!    le exige al contribuyente). Es la firma que autentica al emisor ante el
//!    SII y va como hermana de `<Documento>` dentro de `<DTE>`.
//!
//! ## Por qué RSA-SHA1 (y no SHA256)
//!
//! Igual que el TED: la xsd 1.0 del SII (`DTE_v10.xsd` + `xmldsignature_v10.xsd`)
//! especifica `rsa-sha1` + `sha1` para la firma del documento. NO es decisión
//! nuestra; el validador del SII rechaza firmas SHA256 sobre el DTE. Cripto
//! legal en CL (Resolución SII 45/2003 y posteriores). NO replicar este patrón
//! fuera del módulo DTE.
//!
//! ## Estructura emitida (W3C XML-DSig, perfil SII)
//!
//! ```xml
//! <Signature xmlns="http://www.w3.org/2000/09/xmldsig#">
//!   <SignedInfo>
//!     <CanonicalizationMethod Algorithm=".../REC-xml-c14n-20010315"/>
//!     <SignatureMethod Algorithm=".../xmldsig#rsa-sha1"/>
//!     <Reference URI="#F123T39">
//!       <Transforms>
//!         <Transform Algorithm=".../xmldsig#enveloped-signature"/>
//!       </Transforms>
//!       <DigestMethod Algorithm=".../xmldsig#sha1"/>
//!       <DigestValue>base64(sha1(Documento))</DigestValue>
//!     </Reference>
//!   </SignedInfo>
//!   <SignatureValue>base64(rsa-sha1(SignedInfo))</SignatureValue>
//!   <KeyInfo>
//!     <KeyValue><RSAKeyValue><Modulus/><Exponent/></RSAKeyValue></KeyValue>
//!     <X509Data><X509Certificate>base64(DER)</X509Certificate></X509Data>
//!   </KeyInfo>
//! </Signature>
//! ```
//!
//! ## Canonicalización (alcance de esta subtask)
//!
//! El digest cubre los **bytes UTF-8 exactos del subtree `<Documento>...
//! </Documento>`** tal como los emite `xml::writer` (serializador
//! determinístico, sin pretty-print, sin comentarios, orden de atributos
//! estable). La `SignatureValue` cubre los bytes exactos del `<SignedInfo>`
//! emitido (con su `xmlns` heredado materializado, como exige C14N al
//! canonicalizar el nodo de forma aislada). Esto es **consistente entre firma
//! y verificación** y coincide con el enfoque ya usado para el `<DD>` del TED.
//!
//! Una pasada de **C14N 1.0 completa** (normalización de namespaces heredados
//! para validadores detached, expansión de elementos vacíos, reordenamiento de
//! atributos) se difiere a subtask **9.1.b.4**, gated por el resultado real del
//! sandbox `maullin.sii.cl`: si SII acepta el documento determinístico tal
//! cual, no se agrega complejidad de C14N; si lo rechaza, se refina ahí con la
//! respuesta real como fixture. No firmamos a ciegas un C14N que no podemos
//! validar contra el SII todavía.
//!
//! ## Material de clave (PFX → `KeyMaterial`)
//!
//! El cert digital SII es un PFX/PKCS#12. El parse nativo del PKCS#12 cifrado
//! (PBES1 SHA1-3DES + MAC HMAC-SHA1 que usa E-CertChile y emisores CL) es
//! subtask **9.1.b.3**: es cripto legacy que NO debe shippearse sin un PFX real
//! del SII como fixture de validación (un parser sutilmente incorrecto = el
//! cliente no puede firmar = multa SII). Hasta entonces, este módulo acepta el
//! material de clave ya decodificado vía [`KeyMaterial::from_pem`] (clave
//! PKCS#8/PKCS#1 + cert X.509, ambos PEM) — el formato que produce
//! `openssl pkcs12 -in cert.pfx -nodes -out cert.pem` en el onboarding del
//! cliente. La capa que decifra el PFX at-rest (`cert::decrypt_pfx`, subtask
//! 9.1.i, ya implementada) se enchufa a `KeyMaterial` cuando aterrice 9.1.b.3,
//! sin tocar la firma.

use crate::types::{Caf, Dte, EmisorConfig};
use crate::DteError;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};

const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const SIG_METHOD_RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
const TRANSFORM_ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const DIGEST_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#sha1";
const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";

/// Material de clave de firma: clave privada RSA del cert digital de la
/// empresa + el certificado X.509 (DER) que la respalda. Se obtiene del PFX
/// SII (ver nota PFX en el módulo).
#[derive(Clone)]
pub struct KeyMaterial {
    priv_key: RsaPrivateKey,
    /// Certificado X.509 en DER. Se emite base64 dentro de `<X509Certificate>`.
    cert_der: Vec<u8>,
}

impl std::fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NUNCA volcar la clave privada en logs/debug.
        f.debug_struct("KeyMaterial")
            .field("cert_der_len", &self.cert_der.len())
            .finish_non_exhaustive()
    }
}

impl KeyMaterial {
    /// Construye desde PEMs: clave privada (`-----BEGIN PRIVATE KEY-----`
    /// PKCS#8 o `-----BEGIN RSA PRIVATE KEY-----` PKCS#1) y certificado
    /// (`-----BEGIN CERTIFICATE-----`). Este es el output de
    /// `openssl pkcs12 -in cert.pfx -nodes -out cert.pem` que el cliente
    /// genera una vez en onboarding.
    pub fn from_pem(key_pem: &str, cert_pem: &str) -> Result<Self, DteError> {
        // Probar PKCS#8 primero (formato moderno), caer a PKCS#1 (RSA legacy).
        let priv_key = RsaPrivateKey::from_pkcs8_pem(key_pem)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(key_pem))
            .map_err(|e| {
                DteError::CertInvalid(format!(
                    "clave privada PEM inválida (ni PKCS#8 ni PKCS#1): {e}"
                ))
            })?;
        let cert_der = pem_body_der(cert_pem, "CERTIFICATE")?;
        if cert_der.is_empty() {
            return Err(DteError::CertInvalid(
                "certificado PEM vacío o sin bloque CERTIFICATE".to_string(),
            ));
        }
        Ok(Self { priv_key, cert_der })
    }

    /// Construye directamente desde una clave RSA en memoria + cert DER. Lo
    /// usa el wiring de 9.1.b.3 (PFX decifrado) y los tests.
    pub fn from_parts(priv_key: RsaPrivateKey, cert_der: Vec<u8>) -> Self {
        Self { priv_key, cert_der }
    }

    /// Parse nativo de un PFX/PKCS#12 (subtask 9.1.b.3): extrae la clave RSA
    /// y el certificado de entidad directamente del DER, sin `openssl` externo.
    /// Soporta los esquemas legacy de los emisores de certs SII (PBE-SHA1 con
    /// 3DES/RC2, lo que produce OpenSSL `-legacy`) y PBES2 AES (OpenSSL 3
    /// default), vía `p12-keystore` (pure Rust).
    ///
    /// `passphrase` es la del PFX (la misma que la farmacia teclea al emitir;
    /// el flujo encrypt-at-rest de `cert::` usa la misma passphrase, así que
    /// el operador maneja UNA sola clave).
    pub fn from_pkcs12(pfx_der: &[u8], passphrase: &str) -> Result<Self, DteError> {
        use p12_keystore::{KeyStore, Pkcs12ImportPolicy};
        let ks = KeyStore::from_pkcs12(pfx_der, passphrase, Pkcs12ImportPolicy::Strict).map_err(
            |e| DteError::CertInvalid(format!("PKCS#12 inválido o passphrase incorrecta: {e}")),
        )?;
        let (_alias, chain) = ks.private_key_chain().ok_or_else(|| {
            DteError::CertInvalid(
                "el PKCS#12 no contiene una clave privada con su certificado".to_string(),
            )
        })?;
        let priv_key = RsaPrivateKey::from_pkcs8_der(chain.key().as_der()).map_err(|e| {
            DteError::CertInvalid(format!(
                "la clave del PKCS#12 no es una RSA PKCS#8 válida (SII requiere RSA): {e}"
            ))
        })?;
        // El primer cert de la cadena es el de entidad (convención PKCS#12 que
        // p12-keystore garantiza en Strict); el resto son intermedios/raíz que
        // la firma XML-DSig del SII no necesita.
        let cert = chain.certs().first().ok_or_else(|| {
            DteError::CertInvalid(
                "el PKCS#12 no contiene certificado asociado a la clave".to_string(),
            )
        })?;
        Ok(Self {
            priv_key,
            cert_der: cert.as_der().to_vec(),
        })
    }

    /// Decodifica material de firma desde los bytes tal-como-importados por
    /// `pharma cert import` (post `cert::decrypt_pfx`): un PFX/PKCS#12 binario
    /// (path nativo 9.1.b.3) o, back-compat, un bundle PEM texto (clave +
    /// cert concatenados — el workaround `openssl pkcs12 -nodes` previo).
    ///
    /// Detección: DER PKCS#12 siempre abre con SEQUENCE (`0x30`); un bundle
    /// PEM es texto que parte con `-----BEGIN`.
    pub fn from_keystore_bytes(bytes: &[u8], passphrase: &str) -> Result<Self, DteError> {
        if bytes.first() == Some(&0x30) {
            return Self::from_pkcs12(bytes, passphrase);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| {
            DteError::CertInvalid(
                "el certificado almacenado no es PKCS#12 (DER) ni un bundle PEM UTF-8".to_string(),
            )
        })?;
        let key_pem = extract_pem_block(text, "PRIVATE KEY").ok_or_else(|| {
            DteError::CertInvalid("el bundle PEM no contiene un bloque PRIVATE KEY".to_string())
        })?;
        let cert_pem = extract_pem_block(text, "CERTIFICATE").ok_or_else(|| {
            DteError::CertInvalid("el bundle PEM no contiene un bloque CERTIFICATE".to_string())
        })?;
        Self::from_pem(&key_pem, &cert_pem)
    }
}

/// Extrae el primer bloque PEM completo (BEGIN..END) cuyo tag contiene
/// `needle` — "PRIVATE KEY" matchea PKCS#8 (`PRIVATE KEY`) y PKCS#1
/// (`RSA PRIVATE KEY`). Movido desde el wiring HTTP al crate para que CLI y
/// API compartan el mismo parser de bundle.
fn extract_pem_block(bundle: &str, needle: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(rel) = bundle[search_from..].find("-----BEGIN ") {
        let start = search_from + rel;
        let tag_start = start + "-----BEGIN ".len();
        let tag_end = bundle[tag_start..].find("-----")? + tag_start;
        let tag = &bundle[tag_start..tag_end];
        let end_marker = format!("-----END {tag}-----");
        let end = bundle[start..].find(&end_marker)? + start + end_marker.len();
        if tag.contains(needle) {
            return Some(bundle[start..end].to_string());
        }
        search_from = end;
    }
    None
}

/// Decodifica el cuerpo base64 de un bloque PEM `-----BEGIN <tag>-----` a DER.
fn pem_body_der(pem: &str, tag: &str) -> Result<Vec<u8>, DteError> {
    let begin = format!("-----BEGIN {tag}-----");
    let end = format!("-----END {tag}-----");
    let start = pem
        .find(&begin)
        .ok_or_else(|| DteError::CertInvalid(format!("PEM sin bloque {begin}")))?
        + begin.len();
    let stop = pem
        .find(&end)
        .ok_or_else(|| DteError::CertInvalid(format!("PEM sin cierre {end}")))?;
    if stop < start {
        return Err(DteError::CertInvalid(format!("PEM {tag} mal formado")));
    }
    let body: String = pem[start..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    B64.decode(body.as_bytes())
        .map_err(|e| DteError::CertInvalid(format!("base64 PEM {tag}: {e}")))
}

/// Firma `xml_with_ted` (un `<DTE>` cuyo `<Documento>` ya tiene el TED
/// inyectado) y devuelve el XML con el `<Signature>` enveloped agregado como
/// hermano de `<Documento>`, antes de `</DTE>`.
///
/// Errores:
/// - `SignFailed` si no encuentra `<Documento>` / su `ID`, o si la firma RSA
///   falla.
pub fn sign_xml(xml_with_ted: &str, key: &KeyMaterial) -> Result<String, DteError> {
    sign_enveloped(xml_with_ted, "Documento", "</DTE>", key)
}

/// Firma el Libro de Ventas (`LibroCompraVenta` de `xml::libro`): enveloped
/// signature sobre el `<EnvioLibro ID="...">`, con el `<Signature>` insertado
/// como hermano antes de `</LibroCompraVenta>` — mismo perfil XML-DSig
/// RSA-SHA1 que el DTE (la xsd `LibroCV_v10.xsd` del SII referencia la misma
/// `xmldsignature_v10.xsd`).
pub fn sign_libro(xml_libro: &str, key: &KeyMaterial) -> Result<String, DteError> {
    sign_enveloped(xml_libro, "EnvioLibro", "</LibroCompraVenta>", key)
}

/// Núcleo de la firma enveloped: extrae el subtree `<{tag} ID="...">`, lo
/// digiere, firma el `SignedInfo` y agrega el `<Signature>` justo antes de
/// `close_marker` (el cierre del elemento raíz).
fn sign_enveloped(
    xml: &str,
    tag: &str,
    close_marker: &str,
    key: &KeyMaterial,
) -> Result<String, DteError> {
    let (subtree, id) = extract_subtree_with_id(xml, tag)?;

    // 1. DigestValue = base64(sha1(<{tag}>...</{tag}>)).
    let digest_value = B64.encode(sha1_bytes(subtree.as_bytes()));

    // 2. SignedInfo canónico (con xmlns materializado para firmar el nodo
    //    aislado, como hace C14N al heredarlo de <Signature>).
    let signed_info = build_signed_info(&id, &digest_value);

    // 3. SignatureValue = base64(rsa-sha1(SignedInfo)).
    let signature_value = sign_rsa_sha1(signed_info.as_bytes(), &key.priv_key)?;

    // 4. KeyInfo: RSAKeyValue (modulus/exponent) + X509Certificate.
    let pub_key = RsaPublicKey::from(&key.priv_key);
    let key_info = build_key_info(&pub_key, &key.cert_der);

    let signature = format!(
        "<Signature xmlns=\"{DSIG_NS}\">{signed_info}\
         <SignatureValue>{signature_value}</SignatureValue>{key_info}</Signature>"
    );

    // 5. Insertar <Signature> tras el subtree firmado, antes del cierre raíz.
    inject_signature(xml, close_marker, &signature)
}

/// Wrapper end-to-end: renderiza el DTE sin firma, inyecta el TED del CAF,
/// firma con el cert empresa y devuelve el XML listo para `sii::upload_dte`.
pub fn build_signed_dte(
    dte: &Dte,
    emisor: &EmisorConfig,
    caf: &Caf,
    key: &KeyMaterial,
) -> Result<String, DteError> {
    let xml = crate::xml::render_unsigned(dte, emisor)?;
    let ted = crate::timbre::generate(dte, caf)?;
    let with_ted = inject_ted(&xml, &ted)?;
    sign_xml(&with_ted, key)
}

/// Inyecta el `<TED>` inmediatamente antes de `<TmstFirma>` — la posición que
/// exige la xsd SII dentro de `<Documento>`.
fn inject_ted(xml: &str, ted: &str) -> Result<String, DteError> {
    let marker = "<TmstFirma>";
    let pos = xml.find(marker).ok_or_else(|| {
        DteError::XmlInvalid("XML sin <TmstFirma> para inyectar el TED".to_string())
    })?;
    let mut out = String::with_capacity(xml.len() + ted.len());
    out.push_str(&xml[..pos]);
    out.push_str(ted);
    out.push_str(&xml[pos..]);
    Ok(out)
}

/// Extrae el subtree `<{tag} ...>...</{tag}>` completo y el valor de su
/// atributo `ID` (usado en `Reference URI="#ID"`).
fn extract_subtree_with_id(xml: &str, tag: &str) -> Result<(String, String), DteError> {
    let open = format!("<{tag}");
    let start = xml
        .find(&open)
        .ok_or_else(|| DteError::SignFailed(format!("XML sin <{tag}> para firmar")))?;
    let close = format!("</{tag}>");
    let rel_end = xml[start..]
        .find(&close)
        .ok_or_else(|| DteError::SignFailed(format!("XML sin cierre </{tag}>")))?;
    let subtree = xml[start..start + rel_end + close.len()].to_string();

    // Atributo ID dentro del start-tag (hasta el primer '>').
    let tag_end = subtree
        .find('>')
        .ok_or_else(|| DteError::SignFailed(format!("start-tag <{tag}> sin cierre '>'")))?;
    let start_tag = &subtree[..tag_end];
    let id = extract_attr(start_tag, "ID").ok_or_else(|| {
        DteError::SignFailed(format!("<{tag}> sin atributo ID (requerido por Reference)"))
    })?;
    Ok((subtree, id))
}

/// Extrae `attr="valor"` de un start-tag. Acepta comillas dobles (las que
/// emite quick-xml). Match por nombre exacto precedido de espacio para no
/// confundir `ID` con otros atributos terminados en `ID`.
fn extract_attr(start_tag: &str, attr: &str) -> Option<String> {
    let needle = format!(" {attr}=\"");
    let s = start_tag.find(&needle)? + needle.len();
    let rest = &start_tag[s..];
    let e = rest.find('"')?;
    Some(rest[..e].to_string())
}

/// `<SignedInfo>` canónico, sin whitespace, con `xmlns` materializado.
fn build_signed_info(reference_id: &str, digest_value: &str) -> String {
    format!(
        "<SignedInfo xmlns=\"{DSIG_NS}\">\
         <CanonicalizationMethod Algorithm=\"{C14N}\"></CanonicalizationMethod>\
         <SignatureMethod Algorithm=\"{SIG_METHOD_RSA_SHA1}\"></SignatureMethod>\
         <Reference URI=\"#{reference_id}\">\
         <Transforms>\
         <Transform Algorithm=\"{TRANSFORM_ENVELOPED}\"></Transform>\
         </Transforms>\
         <DigestMethod Algorithm=\"{DIGEST_SHA1}\"></DigestMethod>\
         <DigestValue>{digest_value}</DigestValue>\
         </Reference>\
         </SignedInfo>"
    )
}

/// `<KeyInfo>` con `RSAKeyValue` (modulus/exponent base64 big-endian) y el
/// certificado X.509 en base64(DER).
fn build_key_info(pub_key: &RsaPublicKey, cert_der: &[u8]) -> String {
    let modulus = B64.encode(pub_key.n().to_bytes_be());
    let exponent = B64.encode(pub_key.e().to_bytes_be());
    let cert_b64 = B64.encode(cert_der);
    format!(
        "<KeyInfo>\
         <KeyValue>\
         <RSAKeyValue>\
         <Modulus>{modulus}</Modulus>\
         <Exponent>{exponent}</Exponent>\
         </RSAKeyValue>\
         </KeyValue>\
         <X509Data>\
         <X509Certificate>{cert_b64}</X509Certificate>\
         </X509Data>\
         </KeyInfo>"
    )
}

/// Inserta el `<Signature>` justo antes de `marker` (el cierre del elemento
/// raíz: `</DTE>` o `</LibroCompraVenta>`).
fn inject_signature(xml: &str, marker: &str, signature: &str) -> Result<String, DteError> {
    let pos = xml.find(marker).ok_or_else(|| {
        DteError::SignFailed(format!("XML sin {marker} para insertar <Signature>"))
    })?;
    let mut out = String::with_capacity(xml.len() + signature.len());
    out.push_str(&xml[..pos]);
    out.push_str(signature);
    out.push_str(&xml[pos..]);
    Ok(out)
}

fn sha1_bytes(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn sign_rsa_sha1(data: &[u8], key: &RsaPrivateKey) -> Result<String, DteError> {
    let hash = sha1_bytes(data);
    let scheme = Pkcs1v15Sign::new::<Sha1>();
    let sig = key
        .sign(scheme, &hash)
        .map_err(|e| DteError::SignFailed(format!("firmar SignedInfo RSA-SHA1: {e}")))?;
    Ok(B64.encode(sig))
}

/// Verifica un DTE firmado por `sign_xml`: recomputa el digest del
/// `<Documento>` y lo compara contra `<DigestValue>`, luego verifica la
/// `<SignatureValue>` sobre el `<SignedInfo>` con la clave pública embebida en
/// `<RSAKeyValue>`. Usado por tests de roundtrip y por consumidores que quieran
/// validar firmas propias antes de despachar al SII.
pub fn verify_signature(signed_xml: &str) -> Result<(), DteError> {
    verify_enveloped(signed_xml, "Documento")
}

/// Verifica un Libro de Ventas firmado por `sign_libro` (digest sobre el
/// `<EnvioLibro>`).
pub fn verify_libro_signature(signed_xml: &str) -> Result<(), DteError> {
    verify_enveloped(signed_xml, "EnvioLibro")
}

fn verify_enveloped(signed_xml: &str, tag: &str) -> Result<(), DteError> {
    use rsa::traits::SignatureScheme;

    let (documento, _id) = extract_subtree_with_id(signed_xml, tag)?;
    let digest_calc = B64.encode(sha1_bytes(documento.as_bytes()));
    let digest_xml = extract_element(signed_xml, "DigestValue")
        .ok_or_else(|| DteError::SignFailed("firma sin <DigestValue>".to_string()))?;
    if digest_calc != digest_xml {
        return Err(DteError::SignFailed(format!(
            "DigestValue no coincide con el <{tag}> (documento alterado tras firmar)"
        )));
    }

    let signed_info = extract_subtree(signed_xml, "SignedInfo")
        .ok_or_else(|| DteError::SignFailed("firma sin <SignedInfo>".to_string()))?;
    let sig_b64 = extract_element(signed_xml, "SignatureValue")
        .ok_or_else(|| DteError::SignFailed("firma sin <SignatureValue>".to_string()))?;
    let sig = B64
        .decode(sig_b64.as_bytes())
        .map_err(|e| DteError::SignFailed(format!("base64 SignatureValue: {e}")))?;

    let modulus = extract_element(signed_xml, "Modulus")
        .ok_or_else(|| DteError::SignFailed("firma sin <Modulus>".to_string()))?;
    let exponent = extract_element(signed_xml, "Exponent")
        .ok_or_else(|| DteError::SignFailed("firma sin <Exponent>".to_string()))?;
    let n = BigUint::from_bytes_be(
        &B64.decode(modulus.as_bytes())
            .map_err(|e| DteError::SignFailed(format!("base64 Modulus: {e}")))?,
    );
    let e = BigUint::from_bytes_be(
        &B64.decode(exponent.as_bytes())
            .map_err(|e| DteError::SignFailed(format!("base64 Exponent: {e}")))?,
    );
    let pub_key = RsaPublicKey::new(n, e)
        .map_err(|e| DteError::SignFailed(format!("RSAKeyValue inválido: {e}")))?;

    let hash = sha1_bytes(signed_info.as_bytes());
    let scheme = Pkcs1v15Sign::new::<Sha1>();
    scheme
        .verify(&pub_key, &hash, &sig)
        .map_err(|e| DteError::SignFailed(format!("verificar SignatureValue: {e}")))?;
    Ok(())
}

/// Texto entre `<tag>` y `</tag>` (primer match). Para elementos hoja.
fn extract_element(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = xml.find(&open)? + open.len();
    let rest = &xml[s..];
    let e = rest.find(&close)?;
    Some(rest[..e].to_string())
}

/// Subtree `<tag ...>...</tag>` completo (incluye los tags), primer match.
fn extract_subtree(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let s = xml.find(&open)?;
    let rel = xml[s..].find(&close)? + close.len();
    Some(xml[s..s + rel].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};

    fn test_key() -> KeyMaterial {
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 1024).expect("RSA 1024 test");
        // Cert DER placeholder (los tests no validan X.509, solo el roundtrip
        // de firma sobre SignedInfo y digest sobre Documento).
        KeyMaterial::from_parts(priv_key, b"\x30\x82test-cert-der".to_vec())
    }

    const SAMPLE_DTE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<DTE version=\"1.0\"><Documento ID=\"F123T39\">\
<Encabezado><IdDoc><TipoDTE>39</TipoDTE><Folio>123</Folio></IdDoc></Encabezado>\
<TED version=\"1.0\"><DD></DD><FRMT>x</FRMT></TED>\
<TmstFirma>2026-05-21T10:30:00</TmstFirma></Documento></DTE>";

    #[test]
    fn pem_body_der_decodes() {
        let pem = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
        assert_eq!(pem_body_der(pem, "CERTIFICATE").unwrap(), b"ABC");
    }

    #[test]
    fn pem_body_der_missing_block() {
        assert!(pem_body_der("garbage", "CERTIFICATE").is_err());
    }

    #[test]
    fn extract_attr_exact_match() {
        assert_eq!(
            extract_attr("<Documento ID=\"F1T39\"", "ID"),
            Some("F1T39".to_string())
        );
        // No confundir un atributo terminado en "ID".
        assert_eq!(
            extract_attr("<Documento OtroID=\"x\" ID=\"good\"", "ID"),
            Some("good".to_string())
        );
    }

    #[test]
    fn extract_documento_gets_id() {
        let (doc, id) = extract_subtree_with_id(SAMPLE_DTE, "Documento").unwrap();
        assert_eq!(id, "F123T39");
        assert!(doc.starts_with("<Documento ID=\"F123T39\">"));
        assert!(doc.ends_with("</Documento>"));
        // El TED queda dentro del documento firmado.
        assert!(doc.contains("<TED"));
    }

    #[test]
    fn extract_documento_no_id_fails() {
        let xml = "<DTE><Documento>x</Documento></DTE>";
        assert!(extract_subtree_with_id(xml, "Documento").is_err());
    }

    #[test]
    fn sign_libro_roundtrip() {
        let key = test_key();
        let libro = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<LibroCompraVenta><EnvioLibro ID=\"LV202605\">\
<Caratula><RutEmisorLibro>76123456-7</RutEmisorLibro>\
<PeriodoTributario>2026-05</PeriodoTributario></Caratula>\
<ResumenPeriodo/><DocumentosLista/></EnvioLibro></LibroCompraVenta>";
        let signed = sign_libro(libro, &key).expect("firmar libro");
        assert!(signed.contains("<Reference URI=\"#LV202605\">"));
        // <Signature> tras </EnvioLibro>, antes de </LibroCompraVenta>.
        let pos_envio = signed.find("</EnvioLibro>").unwrap();
        let pos_sig = signed.find("<Signature").unwrap();
        let pos_root = signed.find("</LibroCompraVenta>").unwrap();
        assert!(pos_envio < pos_sig && pos_sig < pos_root);
        verify_libro_signature(&signed).expect("verify libro roundtrip");

        // Tamper dentro del EnvioLibro firmado → digest mismatch.
        let tampered = signed.replace("2026-05", "2026-06");
        let err = verify_libro_signature(&tampered).expect_err("tamper detectado");
        assert!(format!("{err}").contains("DigestValue no coincide"));
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let key = test_key();
        let signed = sign_xml(SAMPLE_DTE, &key).expect("firmar");
        // Estructura esperada.
        assert!(signed.contains("<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\">"));
        assert!(signed.contains("<Reference URI=\"#F123T39\">"));
        assert!(signed.contains("<SignatureValue>"));
        assert!(signed.contains("<X509Certificate>"));
        // <Signature> va tras </Documento> y antes de </DTE>.
        let pos_doc_close = signed.find("</Documento>").unwrap();
        let pos_sig = signed.find("<Signature").unwrap();
        let pos_dte_close = signed.find("</DTE>").unwrap();
        assert!(pos_doc_close < pos_sig && pos_sig < pos_dte_close);
        // Verifica.
        verify_signature(&signed).expect("verify roundtrip");
    }

    #[test]
    fn verify_fails_on_tampered_documento() {
        let key = test_key();
        let signed = sign_xml(SAMPLE_DTE, &key).unwrap();
        // Alterar el folio dentro del <Documento> firmado invalida el digest.
        let tampered = signed.replace("<Folio>123</Folio>", "<Folio>999</Folio>");
        assert_ne!(tampered, signed);
        let err = verify_signature(&tampered).expect_err("digest mismatch");
        assert!(format!("{err}").contains("DigestValue no coincide"));
    }

    #[test]
    fn verify_fails_on_tampered_signature_value() {
        let key = test_key();
        let signed = sign_xml(SAMPLE_DTE, &key).unwrap();
        // Corromper la SignatureValue mantiene el digest pero rompe RSA verify.
        let val = extract_element(&signed, "SignatureValue").unwrap();
        let bad = format!("{}A", &val[..val.len() - 1]);
        let tampered = signed.replace(&val, &bad);
        assert!(verify_signature(&tampered).is_err());
    }

    #[test]
    fn from_pem_roundtrips_key() {
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 1024).unwrap();
        let key_pem = priv_key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let cert_pem = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
        let km = KeyMaterial::from_pem(&key_pem, cert_pem).expect("from_pem PKCS#8");
        assert_eq!(km.cert_der, b"ABC");
        // También PKCS#1.
        let key_pem1 = priv_key.to_pkcs1_pem(LineEnding::LF).unwrap().to_string();
        assert!(KeyMaterial::from_pem(&key_pem1, cert_pem).is_ok());
    }

    #[test]
    fn from_pem_rejects_bad_key() {
        let cert_pem = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
        assert!(KeyMaterial::from_pem("not a key", cert_pem).is_err());
    }
}
