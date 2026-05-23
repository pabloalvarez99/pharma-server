//! Subtask 9.1.i — cert encrypt-at-rest roundtrip + aislamiento por tenant /
//! master_key.
//!
//! Verifica:
//! - encrypt + decrypt con el mismo (tenant, master_key) recupera PFX+password.
//! - decrypt con OTRO tenant falla (clave derivada distinta = defensa lateral).
//! - decrypt con OTRA master_key falla.
//!
//! No usa PFX real: bytes arbitrarios bastan, el cifrado es opaco al contenido.

use chrono::{TimeZone, Utc};
use dte::cert::{decrypt_for_sign, encrypt_at_rest};
use dte::CertDigital;
use uuid::Uuid;

const TENANT1: &str = "tenant:farmacia-uno";
const TENANT2: &str = "tenant:farmacia-dos";

fn master(byte: u8) -> [u8; 32] {
    [byte; 32]
}

/// Bytes que simulan un PFX (no es un PKCS#12 válido, pero el cifrado no lo
/// inspecciona — sólo cifra/descifra opaco).
fn fake_pfx() -> Vec<u8> {
    (0u8..=255).cycle().take(2048).collect()
}

fn cert_with(pfx_enc: Vec<u8>, pw_enc: Vec<u8>) -> CertDigital {
    CertDigital {
        id: Uuid::nil(),
        rut_propietario: "76123456-7".to_string(),
        nombre_propietario: None,
        pfx_blob: pfx_enc,
        password_encrypted: pw_enc,
        vigencia_desde: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        vigencia_hasta: Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap(),
        activo: true,
    }
}

#[test]
fn encrypt_decrypt_mismo_tenant_recupera_plaintext() {
    let pfx = fake_pfx();
    let password = "S3cr3t-PFX-Pass!";
    let mk = master(0xAB);

    let (pfx_enc, pw_enc) = encrypt_at_rest(&pfx, password, TENANT1, &mk).expect("encrypt ok");
    // El blob cifrado NO es el plaintext.
    assert_ne!(pfx_enc, pfx, "pfx_blob no debe quedar en claro");
    assert!(pfx_enc.len() > pfx.len(), "nonce + tag agregan overhead");

    let cert = cert_with(pfx_enc, pw_enc);
    let dec = decrypt_for_sign(&cert, TENANT1, &mk).expect("decrypt ok");
    assert_eq!(dec.pfx, pfx, "PFX recuperado debe coincidir");
    assert_eq!(dec.password, password, "password recuperado debe coincidir");
}

#[test]
fn nonce_aleatorio_produce_ciphertext_distinto() {
    let pfx = fake_pfx();
    let mk = master(0x11);
    let (a, _) = encrypt_at_rest(&pfx, "pw", TENANT1, &mk).unwrap();
    let (b, _) = encrypt_at_rest(&pfx, "pw", TENANT1, &mk).unwrap();
    assert_ne!(
        a, b,
        "dos cifrados del mismo plaintext deben diferir (nonce random)"
    );
}

#[test]
fn decrypt_con_otro_tenant_falla() {
    let pfx = fake_pfx();
    let mk = master(0x42);
    let (pfx_enc, pw_enc) = encrypt_at_rest(&pfx, "pw", TENANT1, &mk).unwrap();
    let cert = cert_with(pfx_enc, pw_enc);

    let err = decrypt_for_sign(&cert, TENANT2, &mk)
        .expect_err("descifrar con tenant distinto debe fallar");
    assert!(
        matches!(err, dte::DteError::CertInvalid(_)),
        "esperado CertInvalid, obtuve: {err:?}"
    );
}

#[test]
fn decrypt_con_otra_master_key_falla() {
    let pfx = fake_pfx();
    let (pfx_enc, pw_enc) = encrypt_at_rest(&pfx, "pw", TENANT1, &master(0x01)).unwrap();
    let cert = cert_with(pfx_enc, pw_enc);

    let err = decrypt_for_sign(&cert, TENANT1, &master(0x02))
        .expect_err("descifrar con master_key distinta debe fallar");
    assert!(matches!(err, dte::DteError::CertInvalid(_)));
}

#[test]
fn tenant_id_demasiado_corto_rechazado() {
    let err = encrypt_at_rest(b"x", "pw", "abc", &master(0x00))
        .expect_err("tenant_id < 8 bytes debe rechazarse");
    assert!(matches!(err, dte::DteError::CertInvalid(_)));
}
