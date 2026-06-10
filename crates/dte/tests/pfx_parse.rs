//! Parse nativo PFX/PKCS#12 (subtask 9.1.b.3): `KeyMaterial::from_pkcs12` y
//! la detección PFX-vs-PEM de `from_keystore_bytes`, incluyendo el roundtrip
//! completo con el encrypt-at-rest de `cert::` (misma passphrase para el PFX
//! y para el blob — UX de una sola clave para el operador).
//!
//! Fixture: `tests/assets/test-cert.pfx` — RSA 2048 self-signed generado con
//! `New-SelfSignedCertificate` + `Export-PfxCertificate` (Windows, export
//! legacy TripleDES-SHA1/PBES1 — el mismo esquema que producen los emisores
//! de certs SII y `openssl -legacy`). Password `test1234`. Sólo testing.

use dte::{sign_xml, verify_signature, KeyMaterial};
use rsa::pkcs8::EncodePrivateKey;
use rsa::pkcs8::LineEnding;
use rsa::RsaPrivateKey;

const TEST_PFX: &[u8] = include_bytes!("assets/test-cert.pfx");
const TEST_PFX_PASSWORD: &str = "test1234";

const DOC_XML: &str = r#"<DTE version="1.0"><Documento ID="F1T39"><Encabezado><Folio>1</Folio></Encabezado></Documento></DTE>"#;

#[test]
fn from_pkcs12_parses_and_signs() {
    let km = KeyMaterial::from_pkcs12(TEST_PFX, TEST_PFX_PASSWORD).expect("parse PFX nativo");
    // El material extraído firma y la firma verifica (clave y cert coherentes).
    let signed = sign_xml(DOC_XML, &km).expect("sign con clave del PFX");
    assert!(signed.contains("<Signature"));
    assert!(signed.contains("X509Certificate"));
    verify_signature(&signed).expect("la firma del PFX verifica");
}

#[test]
fn from_pkcs12_rejects_wrong_password() {
    let err = KeyMaterial::from_pkcs12(TEST_PFX, "incorrecta").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("PKCS#12") || msg.contains("passphrase"),
        "error inesperado: {msg}"
    );
}

#[test]
fn from_pkcs12_rejects_garbage() {
    assert!(KeyMaterial::from_pkcs12(b"\x30\x82not-a-pfx", TEST_PFX_PASSWORD).is_err());
}

#[test]
fn from_keystore_bytes_detects_pfx_der() {
    // Mismo contenido que from_pkcs12 — el primer byte 0x30 enruta al parser DER.
    let km = KeyMaterial::from_keystore_bytes(TEST_PFX, TEST_PFX_PASSWORD).expect("detect DER");
    let signed = sign_xml(DOC_XML, &km).expect("sign");
    verify_signature(&signed).expect("verify");
}

#[test]
fn from_keystore_bytes_falls_back_to_pem_bundle() {
    // Back-compat: bundle PEM (openssl pkcs12 -nodes) sigue funcionando; la
    // passphrase se ignora en ese path (clave sin cifrar dentro del bundle).
    let mut rng = rand::thread_rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 1024).unwrap();
    let key_pem = priv_key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let bundle = format!("{key_pem}-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n");
    let km = KeyMaterial::from_keystore_bytes(bundle.as_bytes(), "cualquiera").expect("bundle PEM");
    assert!(sign_xml(DOC_XML, &km).is_ok());
}

#[test]
fn from_keystore_bytes_rejects_non_cert_payload() {
    assert!(KeyMaterial::from_keystore_bytes(b"hola mundo", "x").is_err());
    assert!(KeyMaterial::from_keystore_bytes(&[0xff, 0x00, 0x01], "x").is_err());
}

#[test]
fn encrypt_at_rest_roundtrip_single_passphrase() {
    // Flujo real de onboarding nativo: `pharma cert import cert.pfx` cifra el
    // PFX tal cual con la passphrase del PFX; al emitir, decrypt + parse usan
    // la MISMA passphrase.
    let enc = dte::cert::encrypt_pfx(TEST_PFX, TEST_PFX_PASSWORD).expect("encrypt at rest");
    let plain = dte::cert::decrypt_pfx(&enc, TEST_PFX_PASSWORD).expect("decrypt at rest");
    let km = KeyMaterial::from_keystore_bytes(&plain, TEST_PFX_PASSWORD).expect("parse PFX");
    let signed = sign_xml(DOC_XML, &km).expect("sign");
    verify_signature(&signed).expect("verify");
}
