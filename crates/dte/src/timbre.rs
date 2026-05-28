//! `TimbreElectronico` (TED) — bloque firmado RSA-SHA1 que SII exige inyectado
//! en cada DTE. Subtask 9.1.b.
//!
//! Estructura del TED:
//! ```xml
//! <TED version="1.0">
//!   <DD>
//!     <RE>rut_emisor</RE><TD>tipo</TD><F>folio</F><FE>fecha</FE>
//!     <RR>rut_receptor</RR><RSR>razon_social_receptor</RSR>
//!     <MNT>monto_total</MNT><IT1>primer_item_nombre</IT1>
//!     <CAF version="1.0">...subtree CAF entero del SII (incluye FRMA SII)...</CAF>
//!     <TSTED>YYYY-MM-DDTHH:MM:SS</TSTED>
//!   </DD>
//!   <FRMT algoritmo="SHA1withRSA">base64(RSA-SHA1(canonical(DD), RSASK))</FRMT>
//! </TED>
//! ```
//!
//! ## Por qué SHA1 (y no SHA256)
//!
//! SII Chile especifica SHA1withRSA en la xsd v1.0 del TED. NO es decisión
//! nuestra; SII no acepta firmas con SHA256 sobre el TED. Esta cripto vieja
//! es legalmente válida en CL para boleta electrónica (Resolución SII 45/2003
//! y posteriores). NO usar este patrón fuera del módulo DTE.
//!
//! ## Canonicalización
//!
//! DD se serializa sin whitespace entre tags y en orden estricto de la xsd.
//! El CAF se inserta como subtree literal (incluye su propio FRMA SII firmado
//! por SII; NO re-canonicalizar). La firma cubre los bytes UTF-8 exactos del
//! `<DD>...</DD>` que aparecerá en el XML final.

use crate::types::{Caf, Dte};
use crate::DteError;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::traits::SignatureScheme;
use rsa::RsaPrivateKey;
use sha1::{Digest, Sha1};

/// Genera el bloque `<TED>...</TED>` listo para inyectar en el XML del DTE
/// antes de `<TmstFirma>`. Firma DD con la clave privada RSASK embebida en
/// el CAF.
pub fn generate(dte: &Dte, caf: &Caf) -> Result<String, DteError> {
    if dte.tipo != caf.tipo_dte {
        return Err(DteError::SignFailed(format!(
            "tipo DTE {} no coincide con tipo CAF {}",
            dte.tipo.code(),
            caf.tipo_dte.code()
        )));
    }
    if dte.folio < caf.folio_desde || dte.folio > caf.folio_hasta {
        return Err(DteError::SignFailed(format!(
            "folio {} fuera del rango CAF [{}..{}]",
            dte.folio, caf.folio_desde, caf.folio_hasta
        )));
    }
    let caf_subtree = extract_caf_subtree(&caf.xml)?;
    let rsask = extract_pem(&caf.xml, "RSASK")?;
    let dd = build_dd(dte, &caf_subtree);
    let firma = sign_dd_rsa_sha1(dd.as_bytes(), &rsask)?;
    Ok(format!(
        "<TED version=\"1.0\">{dd}<FRMT algoritmo=\"SHA1withRSA\">{firma}</FRMT></TED>"
    ))
}

fn build_dd(dte: &Dte, caf_subtree: &str) -> String {
    let primer_item = dte.items.first().map(|i| i.nombre.as_str()).unwrap_or("");
    let monto_total = dte.monto_total.trunc();
    let tsted = dte.fecha_emision.format("%Y-%m-%dT%H:%M:%S");
    format!(
        "<DD><RE>{re}</RE><TD>{td}</TD><F>{f}</F><FE>{fe}</FE>\
         <RR>{rr}</RR><RSR>{rsr}</RSR><MNT>{mnt}</MNT><IT1>{it1}</IT1>\
         {caf}<TSTED>{tsted}</TSTED></DD>",
        re = xml_escape(&dte.rut_emisor),
        td = dte.tipo.code(),
        f = dte.folio,
        fe = dte.fecha_emision.format("%Y-%m-%d"),
        rr = xml_escape(&dte.rut_receptor),
        rsr = xml_escape(&dte.razon_social_receptor),
        mnt = monto_total,
        it1 = xml_escape(primer_item),
        caf = caf_subtree,
        tsted = tsted,
    )
}

fn sign_dd_rsa_sha1(dd_bytes: &[u8], rsask_pem: &str) -> Result<String, DteError> {
    let key = RsaPrivateKey::from_pkcs1_pem(rsask_pem)
        .map_err(|e| DteError::SignFailed(format!("cargar RSASK PKCS#1: {e}")))?;
    let mut hasher = Sha1::new();
    hasher.update(dd_bytes);
    let hash = hasher.finalize();
    let scheme = Pkcs1v15Sign::new::<Sha1>();
    let sig = key
        .sign(scheme, &hash)
        .map_err(|e| DteError::SignFailed(format!("firmar TED: {e}")))?;
    Ok(B64.encode(sig))
}

/// Extrae literal el subtree `<CAF ...>...</CAF>` del XML AUTORIZACION
/// entregado por SII. Conserva bytes exactos: el FRMA SII embebido sigue
/// siendo válido.
fn extract_caf_subtree(xml: &str) -> Result<String, DteError> {
    let start = xml
        .find("<CAF ")
        .or_else(|| xml.find("<CAF>"))
        .ok_or_else(|| DteError::CafInvalid("CAF subtree no encontrado".into()))?;
    let end_marker = "</CAF>";
    let rel_end = xml[start..]
        .find(end_marker)
        .ok_or_else(|| DteError::CafInvalid("cierre </CAF> no encontrado".into()))?;
    Ok(xml[start..start + rel_end + end_marker.len()].to_string())
}

fn extract_pem(xml: &str, tag: &str) -> Result<String, DteError> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = xml
        .find(&open)
        .ok_or_else(|| DteError::CafInvalid(format!("tag <{tag}> no encontrado")))?;
    let e = xml
        .find(&close)
        .ok_or_else(|| DteError::CafInvalid(format!("cierre </{tag}> no encontrado")))?;
    Ok(xml[s + open.len()..e].trim().to_string())
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Verifica un TED previamente generado. Usado por tests (roundtrip) y por
/// consumidores que quieran validar TEDs propios antes de enviar a SII.
///
/// Re-construye DD desde `dte` + CAF subtree, extrae la firma del TED y
/// verifica con la pubkey RSAPUBK del CAF.
pub fn verify(dte: &Dte, caf: &Caf, ted_xml: &str) -> Result<(), DteError> {
    use rsa::pkcs8::DecodePublicKey;
    use rsa::RsaPublicKey;

    let caf_subtree = extract_caf_subtree(&caf.xml)?;
    let dd_esperado = build_dd(dte, &caf_subtree);
    let dd_real = extract_dd(ted_xml)?;
    if dd_esperado != dd_real {
        return Err(DteError::SignFailed(
            "DD del TED no coincide con DTE provisto".to_string(),
        ));
    }
    let firma_b64 = extract_frmt(ted_xml)?;
    let firma = B64
        .decode(firma_b64.as_bytes())
        .map_err(|e| DteError::SignFailed(format!("base64 FRMT: {e}")))?;
    let rsapubk_pem = extract_pem(&caf.xml, "RSAPUBK")?;
    let pubkey = RsaPublicKey::from_public_key_pem(&rsapubk_pem)
        .map_err(|e| DteError::SignFailed(format!("cargar RSAPUBK: {e}")))?;
    let mut hasher = Sha1::new();
    hasher.update(dd_real.as_bytes());
    let hash = hasher.finalize();
    let scheme = Pkcs1v15Sign::new::<Sha1>();
    scheme
        .verify(&pubkey, &hash, &firma)
        .map_err(|e| DteError::SignFailed(format!("verificar firma TED: {e}")))?;
    Ok(())
}

fn extract_dd(ted_xml: &str) -> Result<String, DteError> {
    let s = ted_xml
        .find("<DD>")
        .ok_or_else(|| DteError::SignFailed("DD no encontrado en TED".into()))?;
    let e = ted_xml
        .find("</DD>")
        .ok_or_else(|| DteError::SignFailed("cierre </DD> no encontrado".into()))?;
    Ok(ted_xml[s..e + "</DD>".len()].to_string())
}

fn extract_frmt(ted_xml: &str) -> Result<String, DteError> {
    let marker = "<FRMT algoritmo=\"SHA1withRSA\">";
    let s = ted_xml
        .find(marker)
        .ok_or_else(|| DteError::SignFailed("FRMT no encontrado en TED".into()))?;
    let close = "</FRMT>";
    let e = ted_xml[s..]
        .find(close)
        .ok_or_else(|| DteError::SignFailed("cierre </FRMT> no encontrado".into()))?;
    Ok(ted_xml[s + marker.len()..s + e].trim().to_string())
}
