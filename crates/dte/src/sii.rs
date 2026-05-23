//! Envío de DTEs al endpoint SII (sandbox/prod) y polling de estado.
//!
//! Subtasks 9.1.d (envío) y 9.1.e (polling).
//!
//! Flujo SII (oficial):
//! 1. `GET https://{host}/DTEWS/CrSeed.jws` → semilla XML (`<SEMILLA>`).
//! 2. El cliente envuelve la semilla en `<getToken>` con `<Signature>` XML-DSig
//!    sobre el cert digital y `POST` a `GetTokenFromSeed.jws`. Respuesta:
//!    `<TOKEN>...</TOKEN>` válido 60 min.
//! 3. `POST` multipart a `cgi_dte/UPL/DTEUpload` con campos `rutSender`/`dvSender`/
//!    `rutCompany`/`dvCompany` + archivo `xml` y cookie `TOKEN={token}`. Respuesta
//!    XML contiene `<TRACKID>` numérico.
//! 4. Polling: `POST` a `QueryEstUp.jws` con `getEstUp` SOAP body que incluye
//!    rutEmpresa/dv/trackid/token. Respuesta parseada: `EnProceso` | `Aceptado` |
//!    `Rechazado{glosa}`.
//!
//! ## Testabilidad
//!
//! El "happy path" (multipart + parsing) se prueba con `wiremock` apuntando a
//! `SiiClient::with_base(mock_url)` y llamando `upload_xml`/`query_estado`
//! directamente con un token pre-fabricado. La negociación seed→token requiere
//! XML-DSig real (`crate::sign`) y se ejercita sólo en integración contra SII
//! sandbox real (fuera de CI).

use crate::types::{CertDigital, SiiEnv};
use crate::DteError;
use std::time::Duration;

/// Cliente SII configurable. Para producción se construye desde `SiiEnv` con
/// `SiiClient::new(env)`; tests usan `SiiClient::with_base(url)`.
pub struct SiiClient {
    base: String,
    http: reqwest::Client,
}

impl SiiClient {
    pub fn new(env: SiiEnv) -> Self {
        Self::with_base(env.host())
    }

    /// Construye un cliente apuntado a `base` (sin trailing slash). Para tests
    /// con wiremock pasar la URL completa del mock.
    pub fn with_base(base: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest builder default ok");
        Self {
            base: trim_slash(base.into()),
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// `GET /DTEWS/CrSeed.jws` — devuelve el XML SOAP con `<SEMILLA>...</SEMILLA>`.
    /// Caller debe extraer la semilla y firmarla (XML-DSig) antes de pedir el token.
    pub async fn fetch_seed(&self) -> Result<String, DteError> {
        let url = self.url("/DTEWS/CrSeed.jws");
        retry_5xx(3, || async {
            let resp = self
                .http
                .get(&url)
                .send()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("GET CrSeed: {e}")))?;
            let resp = check_status(resp)?;
            resp.text()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("leer CrSeed: {e}")))
        })
        .await
    }

    /// `POST /DTEWS/GetTokenFromSeed.jws` con `<getToken>...</getToken>` ya
    /// firmado (XML-DSig). Devuelve el TOKEN extraído.
    pub async fn fetch_token(&self, signed_seed_xml: &str) -> Result<String, DteError> {
        let url = self.url("/DTEWS/GetTokenFromSeed.jws");
        let soap = wrap_get_token_soap(signed_seed_xml);
        let body = retry_5xx(3, || async {
            let resp = self
                .http
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=utf-8")
                .header("SOAPAction", "")
                .body(soap.clone())
                .send()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("POST GetToken: {e}")))?;
            let resp = check_status(resp)?;
            resp.text()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("leer GetToken: {e}")))
        })
        .await?;
        extract_token(&body)
    }

    /// `POST multipart /cgi_dte/UPL/DTEUpload` con el XML firmado y cookie
    /// `TOKEN={token}`. Devuelve `track_id`.
    pub async fn upload_xml(
        &self,
        token: &str,
        xml_firmado: &str,
        rut_emisor: &str,
    ) -> Result<i64, DteError> {
        let (rut_num, dv) = split_rut(rut_emisor)?;
        let url = self.url("/cgi_dte/UPL/DTEUpload");
        let body = retry_5xx(3, || async {
            let form = reqwest::multipart::Form::new()
                .text("rutSender", rut_num.clone())
                .text("dvSender", dv.clone())
                .text("rutCompany", rut_num.clone())
                .text("dvCompany", dv.clone())
                .part(
                    "archivo",
                    reqwest::multipart::Part::bytes(xml_firmado.as_bytes().to_vec())
                        .file_name("envio.xml")
                        .mime_str("text/xml")
                        .map_err(|e| DteError::SiiNetwork(format!("mime archivo: {e}")))?,
                );
            let resp = self
                .http
                .post(&url)
                .header(reqwest::header::COOKIE, format!("TOKEN={token}"))
                .multipart(form)
                .send()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("POST DTEUpload: {e}")))?;
            let resp = check_status(resp)?;
            resp.text()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("leer DTEUpload: {e}")))
        })
        .await?;
        extract_track_id(&body)
    }

    /// `POST /DTEWS/QueryEstUp.jws` con SOAP `getEstUp`. Parsea el estado.
    pub async fn query_estado(
        &self,
        token: &str,
        track_id: i64,
        rut_emisor: &str,
    ) -> Result<SiiEstado, DteError> {
        let (rut_num, dv) = split_rut(rut_emisor)?;
        let url = self.url("/DTEWS/QueryEstUp.jws");
        let soap = wrap_get_est_up_soap(&rut_num, &dv, track_id, token);
        let body = retry_5xx(3, || async {
            let resp = self
                .http
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "text/xml; charset=utf-8")
                .header("SOAPAction", "")
                .body(soap.clone())
                .send()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("POST QueryEstUp: {e}")))?;
            let resp = check_status(resp)?;
            resp.text()
                .await
                .map_err(|e| DteError::SiiNetwork(format!("leer QueryEstUp: {e}")))
        })
        .await?;
        parse_estado(&body)
    }
}

/// Sube un XML DTE firmado al endpoint SII correspondiente al entorno.
/// Devuelve `track_id` cuando SII acepta el envío inicial.
///
/// Orquesta seed → sign seed → token → upload. La firma del seed (XML-DSig)
/// se delega a `crate::sign::sign_seed_xml`.
pub async fn upload(
    env: SiiEnv,
    xml_firmado: &str,
    cert: &CertDigital,
    master_key: &[u8; 32],
    tenant_id: &str,
) -> Result<i64, DteError> {
    let client = SiiClient::new(env);
    let token = negotiate_token(&client, cert, master_key, tenant_id).await?;
    client
        .upload_xml(&token, xml_firmado, &cert.rut_propietario)
        .await
}

/// Consulta el estado del envío `track_id` en SII.
pub async fn estado(
    env: SiiEnv,
    track_id: i64,
    cert: &CertDigital,
    master_key: &[u8; 32],
    tenant_id: &str,
) -> Result<SiiEstado, DteError> {
    let client = SiiClient::new(env);
    let token = negotiate_token(&client, cert, master_key, tenant_id).await?;
    client
        .query_estado(&token, track_id, &cert.rut_propietario)
        .await
}

/// Orquesta el ciclo seed→sign→token. Requiere `crate::sign::sign_seed_xml`
/// operativo (XML-DSig real). Propaga errores de cifra/firma intactos.
async fn negotiate_token(
    client: &SiiClient,
    cert: &CertDigital,
    master_key: &[u8; 32],
    tenant_id: &str,
) -> Result<String, DteError> {
    let seed_xml = client.fetch_seed().await?;
    let semilla = extract_semilla(&seed_xml)?;
    let signed = crate::sign::sign_seed_xml(&semilla, cert, master_key, tenant_id)?;
    client.fetch_token(&signed).await
}

/// Estado devuelto por SII al consultar `track_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiiEstado {
    EnProceso,
    Aceptado,
    Rechazado { glosa: String },
}

// ---------- helpers internos ----------

fn trim_slash(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Convierte `reqwest::Response` no-2xx en `DteError::SiiNetwork` preservando
/// el código. 4xx termina el retry inmediatamente; 5xx lo dispara.
fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, DteError> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        Err(DteError::SiiNetwork(format!(
            "HTTP status {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        )))
    }
}

/// Reintenta `f` hasta `attempts` veces ante errores 5xx o de transporte.
/// 4xx propaga inmediatamente. Backoff exponencial corto (50ms, 200ms).
async fn retry_5xx<F, Fut, T>(attempts: usize, mut f: F) -> Result<T, DteError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, DteError>>,
{
    let mut last_err: Option<DteError> = None;
    for intento in 0..attempts {
        match f().await {
            Ok(v) => return Ok(v),
            Err(DteError::SiiNetwork(msg)) => {
                let is_retriable = msg.contains("HTTP status 5") || msg.contains("timeout");
                if !is_retriable {
                    return Err(DteError::SiiNetwork(msg));
                }
                last_err = Some(DteError::SiiNetwork(msg));
                if intento + 1 < attempts {
                    let ms = 50u64 << intento;
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
            }
            Err(other) => return Err(other),
        }
    }
    Err(last_err.unwrap_or_else(|| DteError::SiiNetwork("retry agotado".into())))
}

/// RUT chileno formato `12345678-9` o `12.345.678-9` → `("12345678","9")`.
fn split_rut(rut: &str) -> Result<(String, String), DteError> {
    let clean: String = rut.chars().filter(|c| *c != '.').collect();
    let mut parts = clean.split('-');
    let num = parts
        .next()
        .ok_or_else(|| DteError::SiiNetwork(format!("RUT inválido (sin '-'): {rut}")))?;
    let dv = parts
        .next()
        .ok_or_else(|| DteError::SiiNetwork(format!("RUT inválido (sin DV): {rut}")))?;
    if num.is_empty() || dv.is_empty() || parts.next().is_some() {
        return Err(DteError::SiiNetwork(format!("RUT inválido: {rut}")));
    }
    Ok((num.to_string(), dv.to_string()))
}

fn extract_semilla(xml: &str) -> Result<String, DteError> {
    extract_between(xml, "<SEMILLA>", "</SEMILLA>")
        .ok_or_else(|| DteError::SiiNetwork("SEMILLA no encontrada en respuesta SII".into()))
}

fn extract_token(xml: &str) -> Result<String, DteError> {
    extract_between(xml, "<TOKEN>", "</TOKEN>")
        .ok_or_else(|| DteError::SiiNetwork("TOKEN no encontrado en respuesta SII".into()))
}

fn extract_track_id(xml: &str) -> Result<i64, DteError> {
    let s = extract_between(xml, "<TRACKID>", "</TRACKID>")
        .ok_or_else(|| DteError::SiiNetwork("TRACKID no encontrado en respuesta SII".into()))?;
    s.trim()
        .parse::<i64>()
        .map_err(|e| DteError::SiiNetwork(format!("TRACKID no parseable como i64 ({s}): {e}")))
}

fn extract_between(s: &str, open: &str, close: &str) -> Option<String> {
    let start = s.find(open)? + open.len();
    let end_rel = s[start..].find(close)?;
    Some(s[start..start + end_rel].trim().to_string())
}

fn parse_estado(xml: &str) -> Result<SiiEstado, DteError> {
    // Respuesta típica getEstUp:
    //   <ESTADO>EPR</ESTADO>          → EnProceso
    //   <ESTADO>DOK</ESTADO> o RPR     → Aceptado
    //   <ESTADO>RCH</ESTADO> + <GLOSA> → Rechazado
    // Hay variantes textuales; manejamos las más comunes.
    let estado = extract_between(xml, "<ESTADO>", "</ESTADO>")
        .ok_or_else(|| DteError::SiiNetwork("ESTADO no encontrado en respuesta SII".into()))?;
    match estado.as_str() {
        "EPR" | "SOK" | "EnProceso" => Ok(SiiEstado::EnProceso),
        "DOK" | "RPR" | "RFR" | "Aceptado" => Ok(SiiEstado::Aceptado),
        "RCH" | "RSC" | "FAU" | "Rechazado" => {
            let glosa = extract_between(xml, "<GLOSA>", "</GLOSA>")
                .or_else(|| extract_between(xml, "<GLOSA_ESTADO>", "</GLOSA_ESTADO>"))
                .unwrap_or_else(|| "rechazo sin glosa".to_string());
            Ok(SiiEstado::Rechazado { glosa })
        }
        other => Err(DteError::SiiNetwork(format!(
            "ESTADO SII desconocido: {other}"
        ))),
    }
}

fn wrap_get_token_soap(signed_seed_xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><getToken xmlns="http://DefaultNamespace"><pszXml><![CDATA[{signed_seed_xml}]]></pszXml></getToken></soap:Body></soap:Envelope>"#
    )
}

fn wrap_get_est_up_soap(rut_num: &str, dv: &str, track_id: i64, token: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><getEstUp xmlns="http://DefaultNamespace"><RutEmpresa>{rut_num}</RutEmpresa><DvEmpresa>{dv}</DvEmpresa><TrackId>{track_id}</TrackId><Token>{token}</Token></getEstUp></soap:Body></soap:Envelope>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_rut_acepta_punto_y_sin_punto() {
        assert_eq!(
            split_rut("76123456-7").unwrap(),
            ("76123456".to_string(), "7".to_string())
        );
        assert_eq!(
            split_rut("76.123.456-K").unwrap(),
            ("76123456".to_string(), "K".to_string())
        );
    }

    #[test]
    fn split_rut_rechaza_malformados() {
        assert!(split_rut("76123456").is_err());
        assert!(split_rut("-7").is_err());
        assert!(split_rut("76123456-").is_err());
        assert!(split_rut("a-b-c").is_err());
    }

    #[test]
    fn extract_track_id_parsea() {
        let xml = "<RESP><TRACKID>123456789</TRACKID></RESP>";
        assert_eq!(extract_track_id(xml).unwrap(), 123456789);
    }

    #[test]
    fn extract_track_id_falla_sin_tag() {
        let xml = "<RESP></RESP>";
        assert!(extract_track_id(xml).is_err());
    }

    #[test]
    fn extract_track_id_falla_si_no_es_int() {
        let xml = "<RESP><TRACKID>abc</TRACKID></RESP>";
        assert!(extract_track_id(xml).is_err());
    }

    #[test]
    fn parse_estado_mapea_codigos() {
        assert_eq!(
            parse_estado("<X><ESTADO>EPR</ESTADO></X>").unwrap(),
            SiiEstado::EnProceso
        );
        assert_eq!(
            parse_estado("<X><ESTADO>DOK</ESTADO></X>").unwrap(),
            SiiEstado::Aceptado
        );
        let r = parse_estado("<X><ESTADO>RCH</ESTADO><GLOSA>folio repetido</GLOSA></X>").unwrap();
        assert_eq!(
            r,
            SiiEstado::Rechazado {
                glosa: "folio repetido".into()
            }
        );
    }

    #[test]
    fn parse_estado_codigo_desconocido_error() {
        assert!(parse_estado("<X><ESTADO>ZZZ</ESTADO></X>").is_err());
    }
}
