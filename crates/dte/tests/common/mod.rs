//! Helpers compartidos por los tests de dte. Genera CAF synthetic con clave
//! RSA real (1024 bits para velocidad — testing only, SII real usa 2048).

#![allow(dead_code)] // cada test binary usa un subset

use chrono::{TimeZone, Utc};
use dte::{Caf, Dte, DteEstado, DteItem, DteTipo, EmisorConfig};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::{EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Genera un CAF XML mínimo válido con clave RSA recién generada. El
/// `<FRMA>` SII embebido es un placeholder (los tests del TED no validan
/// la firma SII; sólo la firma propia del DTE sobre DD).
pub fn caf_xml_synthetic(rut: &str, tipo: DteTipo, desde: i64, hasta: i64) -> (String, Caf) {
    let mut rng = rand::thread_rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 1024).expect("generar RSA 1024");
    let pub_key = RsaPublicKey::from(&priv_key);
    let rsask_pem = priv_key
        .to_pkcs1_pem(LineEnding::LF)
        .expect("PEM RSASK")
        .to_string();
    let rsapubk_pem = pub_key
        .to_public_key_pem(LineEnding::LF)
        .expect("PEM RSAPUBK");
    let xml = format!(
        r#"<AUTORIZACION>
<CAF version="1.0">
<DA>
<RE>{rut}</RE>
<RS>FARMACIA TEST SPA</RS>
<TD>{td}</TD>
<RNG><D>{d}</D><H>{h}</H></RNG>
<FA>2026-01-01</FA>
<RSAPK><M>placeholder</M><E>Aw==</E></RSAPK>
<IDK>100</IDK>
</DA>
<FRMA algoritmo="SHA1withRSA">PLACEHOLDER_SII_SIGNATURE</FRMA>
</CAF>
<RSASK>{rsask_pem}</RSASK>
<RSAPUBK>{rsapubk_pem}</RSAPUBK>
</AUTORIZACION>"#,
        rut = rut,
        td = tipo.code(),
        d = desde,
        h = hasta,
        rsask_pem = rsask_pem,
        rsapubk_pem = rsapubk_pem,
    );
    let caf = Caf {
        id: Uuid::new_v4(),
        tipo_dte: tipo,
        folio_desde: desde,
        folio_hasta: hasta,
        next_folio: desde,
        fecha_autorizacion: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        rut_emisor: rut.to_string(),
        xml: xml.clone(),
        activo: true,
    };
    (xml, caf)
}

pub fn dte_boleta_minimal(folio: i64) -> Dte {
    Dte {
        id: Uuid::nil(),
        tipo: DteTipo::BoletaElectronica,
        folio,
        fecha_emision: Utc.with_ymd_and_hms(2026, 5, 21, 10, 30, 0).unwrap(),
        rut_emisor: "76123456-7".to_string(),
        rut_receptor: "66666666-6".to_string(),
        razon_social_receptor: "SIN INFORMACION".to_string(),
        giro_receptor: None,
        direccion_receptor: None,
        comuna_receptor: None,
        ind_traslado: None,
        referencias: vec![],
        descuentos_globales: vec![],
        monto_neto: Decimal::ZERO,
        iva: Decimal::ZERO,
        monto_exento: Decimal::ZERO,
        monto_total: Decimal::from(1000),
        items: vec![DteItem {
            nro_linea: 1,
            nombre: "Paracetamol 500mg".to_string(),
            cantidad: Decimal::from(2),
            precio_unitario: Decimal::from(500),
            descuento_pct: None,
            monto_item: Decimal::from(1000),
            codigo_sku: None,
            unidad_medida: None,
            exento: false,
        }],
        estado: DteEstado::Draft,
        xml_firmado: None,
        timbre: None,
        track_id: None,
        sii_glosa: None,
        metadata: None,
    }
}

pub fn emisor_test() -> EmisorConfig {
    EmisorConfig {
        rut: "76123456-7".to_string(),
        razon_social: "FARMACIA TEST SPA".to_string(),
        giro: "FARMACIA".to_string(),
        direccion: "CALLE 123".to_string(),
        comuna: "COQUIMBO".to_string(),
        ciudad: Some("COQUIMBO".to_string()),
        acteco: Some(477310),
    }
}
