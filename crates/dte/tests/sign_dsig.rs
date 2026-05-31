//! Subtask 9.1.b.2 — firma XML-DSig end-to-end de una boleta (39).
//!
//! Ejercita `build_signed_dte`: render boleta sin firma → inyección del TED
//! (firmado con la RSASK del CAF) → firma `<Signature>` enveloped del
//! `<Documento>` con la clave del cert empresa → verificación de ambas firmas.
//!
//! No toca red SII; es el armado offline completo (tier Free genera local +
//! el operador despacha). El envío real (`sii::upload_dte`) tiene cobertura en
//! `sii_upload.rs`.

mod common;

use common::{caf_xml_synthetic, dte_boleta_minimal, emisor_test};
use dte::timbre::verify as verify_ted;
use dte::{build_signed_dte, verify_signature, DteTipo, KeyMaterial};
use rsa::RsaPrivateKey;

/// Genera material de clave del cert empresa (RSA fresca + cert DER dummy).
/// El cert DER no se valida como X.509 en esta subtask; solo se emite base64
/// dentro de `<X509Certificate>`.
fn company_key() -> KeyMaterial {
    let mut rng = rand::thread_rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 1024).expect("RSA empresa 1024");
    KeyMaterial::from_parts(priv_key, b"\x30\x82dummy-x509-der".to_vec())
}

#[test]
fn boleta_firmada_e2e_verifica() {
    let rut = "76123456-7";
    let (_caf_xml, caf) = caf_xml_synthetic(rut, DteTipo::BoletaElectronica, 1, 100);
    let mut dte = dte_boleta_minimal(1);
    dte.folio = 1;

    let key = company_key();
    let signed = build_signed_dte(&dte, &emisor_test(), &caf, &key).expect("build_signed_dte");

    // 1. Estructura: TED dentro del Documento + Signature hermana antes de </DTE>.
    assert!(signed.contains(r#"<TED version="1.0">"#), "TED inyectado");
    assert!(
        signed.contains(r#"<Signature xmlns="http://www.w3.org/2000/09/xmldsig#">"#),
        "Signature presente"
    );
    let pos_ted = signed.find("<TED").unwrap();
    let pos_tmst = signed.find("<TmstFirma>").unwrap();
    let pos_doc_close = signed.find("</Documento>").unwrap();
    let pos_sig = signed.find("<Signature").unwrap();
    assert!(pos_ted < pos_tmst, "TED precede a TmstFirma");
    assert!(pos_doc_close < pos_sig, "Signature va tras </Documento>");

    // 2. La firma XML-DSig del cert empresa verifica.
    verify_signature(&signed).expect("verify firma cert empresa");

    // 3. El TED (firma RSASK del CAF) sigue verificando dentro del XML firmado.
    let ted = extraer_ted(&signed);
    verify_ted(&dte, &caf, &ted).expect("verify TED dentro del documento firmado");

    // 4. La Reference apunta al ID del Documento (F{folio}T{tipo}).
    assert!(
        signed.contains("<Reference URI=\"#F1T39\">"),
        "Reference URI"
    );
}

#[test]
fn tamper_monto_post_firma_invalida_dsig() {
    let (_caf_xml, caf) = caf_xml_synthetic("76123456-7", DteTipo::BoletaElectronica, 1, 100);
    let mut dte = dte_boleta_minimal(1);
    dte.folio = 1;
    let signed = build_signed_dte(&dte, &emisor_test(), &caf, &company_key()).unwrap();

    // Alterar el monto total dentro del <Documento> rompe el DigestValue.
    let tampered = signed.replace("<MntTotal>1000</MntTotal>", "<MntTotal>1</MntTotal>");
    assert_ne!(tampered, signed, "el tamper debe modificar el XML");
    let err = verify_signature(&tampered).expect_err("digest mismatch tras tamper");
    assert!(
        format!("{err}").contains("DigestValue no coincide"),
        "esperado fallo de digest, msg: {err}"
    );
}

fn extraer_ted(xml: &str) -> String {
    let s = xml.find("<TED").expect("TED presente");
    let close = "</TED>";
    let e = xml[s..].find(close).expect("cierre </TED>");
    xml[s..s + e + close.len()].to_string()
}
