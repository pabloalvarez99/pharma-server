//! Envío de DTEs al endpoint SII + polling de estado.
//!
//! Subtasks **9.1.d** (upload multipart) y **9.1.e** (polling SOAP).
//!
//! ## Endpoints SII usados
//!
//! - **Upload** `POST /cgi_dte/UPL/DTEUpload` (multipart/form-data) en
//!   `maullin.sii.cl` (sandbox/cert) / `palena.sii.cl` (prod). Campos:
//!   `rutSender`, `dvSender`, `rutCompany`, `dvCompany`, `archivo` (xml).
//!   Respuesta XML `<RECEPCIONDTE>` con `<TRACKID>`, `<STATUS>`, `<TIMESTAMP>`.
//!   Spec: SII "Manual Desarrollador Externo — Envío Automático DTE"
//!   <https://www.sii.cl/factura_electronica/factura_mercado/envio.pdf>.
//! - **Poll** SOAP RPC/encoded `getEstUp` en `/DTEWS/QueryEstUp.jws`. Params
//!   string-en-orden: `RutCompania`, `DvCompania`, `TrackId`, `Token`. WSDL
//!   `targetNamespace=http://DefaultNamespace`, `soapAction` vacío. Respuesta
//!   string serializa XML `<SII:RESPUESTA>` con `<ESTADO>` (códigos: `EPR`
//!   procesado, `RCH` rechazo, `RFR` reparos, `RCT` rechazo content, `RSC`
//!   rechazo schema, `REC` recibido, `SOK` schema ok, `RPR` reparos
//!   pendientes, etc.) y `<GLOSA_ESTADO>`. Spec: SII "Manual Consulta Estado
//!   Envío DTE" <https://www.sii.cl/factura_electronica/factura_mercado/estado_envio.pdf>.
//!
//! ## Auth (lo que NO se hace aquí)
//!
//! La descarga del token SII (autenticación con seed firmado por cert PFX vs
//! `/DTEWS/GetTokenFromSeed.jws`) es la subtask 9.1.i (`cert::*` está stub).
//! Esta fase acepta `cert`+`cert_pass`+`Token` opcional vía API y los pasa
//! through; la firma/seed-token flow se enchufa después sin cambiar las
//! signatures públicas. Los tests usan wiremock + token vacío.
//!
//! ## Lo que es nuestra implementación vs spec verificada
//!
//! - Upload multipart field names + nombre archivo + parsing
//!   `<RECEPCIONDTE><TRACKID>` ↔ verificado en spec SII.
//! - `getEstUp` SOAP envelope (RPC/encoded, params `RutCompania/DvCompania/
//!   TrackId/Token` en ese orden, `xsi:type="xsd:string"`) ↔ verificado vía
//!   WSDL `https://maullin.sii.cl/DTEWS/QueryEstUp.jws?WSDL`.
//! - El XML payload anidado adentro del `<getEstUpReturn>` está documentado
//!   por SII pero su shape exacto (tags `<ESTADO>` vs `<RECEP_DTE>` etc.)
//!   puede variar entre versiones del WS. Ver `TODO(sii-spec)` en `parse_poll_xml`.

use crate::types::SiiEnv;
use crate::DteError;
use chrono::{DateTime, NaiveDateTime, Utc};
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, StatusCode};
use std::time::Duration;

/// Resultado de subir un DTE al SII (subtask 9.1.d).
///
/// `track_id` es el identificador asignado por SII al envío y se usa para
/// `poll_status`. `fecha_recepcion` viene del `<TIMESTAMP>` SII (UTC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadResult {
    pub track_id: i64,
    pub fecha_recepcion: DateTime<Utc>,
}

/// Estado normalizado del envío en SII (subtask 9.1.e).
///
/// Mapea los códigos string del SII a un enum estable. Decisión por código:
/// - `EPR` (Envío procesado) → `Aceptado`.
/// - `SOK` / `REC` (schema ok / recibido aún sin procesar) → `EnProceso`.
/// - `FOK` (firma ok) / `LOK` (lectura ok) / `COK` (cláusulas ok) → `Recibido`.
/// - `RFR` / `RPR` (reparos) → `AceptadoConReparos`.
/// - `RCH` / `RCT` / `RSC` / `RPT` / `RFT` (rechazos) → `Rechazado`.
/// - cualquier otro → `Error` (glosa preserva el código original).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiiEstado {
    Recibido,
    EnProceso,
    Aceptado,
    AceptadoConReparos,
    Rechazado,
    Error,
}

/// Resultado de `poll_status`. Combina estado normalizado + glosa textual
/// SII + timestamp de aceptación si el estado lo trae.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollStatus {
    pub estado: SiiEstado,
    pub glosa: String,
    pub accepted_at: Option<DateTime<Utc>>,
}

/// Sube `signed_xml` al endpoint SII vía multipart/form-data y devuelve el
/// `track_id` asignado.
///
/// Params:
/// - `cert` / `cert_pass`: PFX del certificado digital y password. Hoy se
///   pasan through pero NO se usan para autenticar (token vacío). Ver módulo
///   doc `Auth` arriba — el seed/token flow es subtask 9.1.i.
/// - `rut_emisor`: RUT de la empresa dueña del DTE, formato `12345678-9`.
/// - `rut_envia`: RUT del operador (puede coincidir con emisor para envío
///   propio), mismo formato.
///
/// Errores:
/// - `DteError::SiiNetwork` para fallos de conexión, timeout, body lectura,
///   HTTP 5xx, o response no-XML/no-parseable.
/// - `DteError::SiiRejected { glosa }` si `<STATUS>` ≠ `0` (SII rechazó el
///   envío inicial — schema mal, tamaño, autenticación, etc.).
pub async fn upload_dte(
    env: SiiEnv,
    signed_xml: &str,
    cert: &[u8],
    cert_pass: &str,
    rut_emisor: &str,
    rut_envia: &str,
) -> Result<UploadResult, DteError> {
    upload_dte_to(
        env.upload_endpoint(),
        signed_xml,
        cert,
        cert_pass,
        rut_emisor,
        rut_envia,
    )
    .await
}

/// Variante interna apuntable a una URL arbitraria — la usan los tests con
/// wiremock. Mantiene la lógica HTTP/parsing 100% testeable sin tocar SII real.
async fn upload_dte_to(
    url: &str,
    signed_xml: &str,
    _cert: &[u8],
    _cert_pass: &str,
    rut_emisor: &str,
    rut_envia: &str,
) -> Result<UploadResult, DteError> {
    let (rut_sender, dv_sender) = split_rut(rut_envia)?;
    let (rut_company, dv_company) = split_rut(rut_emisor)?;

    let archivo = Part::bytes(signed_xml.as_bytes().to_vec())
        .file_name("dte.xml")
        .mime_str("text/xml")
        .map_err(|e| DteError::SiiNetwork(format!("multipart mime: {e}")))?;

    let form = Form::new()
        .text("rutSender", rut_sender)
        .text("dvSender", dv_sender)
        .text("rutCompany", rut_company)
        .text("dvCompany", dv_company)
        .part("archivo", archivo);

    let resp = http_client()?
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| DteError::SiiNetwork(format!("upload POST: {e}")))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| DteError::SiiNetwork(format!("upload body: {e}")))?;

    if status.is_server_error() {
        return Err(DteError::SiiNetwork(format!(
            "SII HTTP {}: {}",
            status.as_u16(),
            truncate(&body, 200)
        )));
    }
    if !status.is_success() {
        return Err(DteError::SiiNetwork(format!(
            "SII HTTP {} (cliente): {}",
            status.as_u16(),
            truncate(&body, 200)
        )));
    }

    parse_upload_xml(&body)
}

/// Consulta el estado de `track_id` para `rut_consulta` (RUT empresa dueña
/// del envío, ej. `76123456-7`).
///
/// SII expone esto como SOAP 1.1 RPC/encoded en `/DTEWS/QueryEstUp.jws`,
/// método `getEstUp(RutCompania, DvCompania, TrackId, Token)`. El response
/// `getEstUpReturn` es un string que contiene XML serializado con el estado.
pub async fn poll_status(
    env: SiiEnv,
    track_id: i64,
    rut_consulta: &str,
) -> Result<PollStatus, DteError> {
    poll_status_at(query_endpoint(env), track_id, rut_consulta, "").await
}

/// Variante interna parametrizable por URL y token — la usan los tests.
async fn poll_status_at(
    url: String,
    track_id: i64,
    rut_consulta: &str,
    token: &str,
) -> Result<PollStatus, DteError> {
    let (rut, dv) = split_rut(rut_consulta)?;
    let envelope = build_get_est_up_envelope(&rut, &dv, track_id, token);

    let resp = http_client()?
        .post(&url)
        // SOAP 1.1: `Content-Type: text/xml; charset=utf-8` y SOAPAction
        // (vacío según el WSDL de QueryEstUp.jws).
        .header("Content-Type", "text/xml; charset=utf-8")
        .header("SOAPAction", "\"\"")
        .body(envelope)
        .send()
        .await
        .map_err(|e| DteError::SiiNetwork(format!("poll POST: {e}")))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| DteError::SiiNetwork(format!("poll body: {e}")))?;

    if status.is_server_error() {
        return Err(DteError::SiiNetwork(format!(
            "SII HTTP {}: {}",
            status.as_u16(),
            truncate(&body, 200)
        )));
    }
    if !status.is_success() && status != StatusCode::OK {
        return Err(DteError::SiiNetwork(format!(
            "SII HTTP {} (cliente): {}",
            status.as_u16(),
            truncate(&body, 200)
        )));
    }

    parse_poll_envelope(&body)
}

// ---------- helpers HTTP ----------

fn http_client() -> Result<Client, DteError> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("pharma-server-dte/0.1")
        .build()
        .map_err(|e| DteError::SiiNetwork(format!("http client: {e}")))
}

/// Deriva `/DTEWS/QueryEstUp.jws` del host del `upload_endpoint`.
fn query_endpoint(env: SiiEnv) -> String {
    let host = match env {
        SiiEnv::Sandbox => "maullin.sii.cl",
        SiiEnv::Prod => "palena.sii.cl",
    };
    format!("https://{host}/DTEWS/QueryEstUp.jws")
}

/// Divide `12345678-9` en `("12345678", "9")`. SII espera RUT y DV separados
/// en ambos endpoints. Acepta DV `K`/`k`.
fn split_rut(rut: &str) -> Result<(String, String), DteError> {
    let cleaned = rut.trim().replace('.', "");
    let mut parts = cleaned.split('-');
    let num = parts.next().unwrap_or("").trim();
    let dv = parts.next().unwrap_or("").trim();
    if num.is_empty() || dv.is_empty() || parts.next().is_some() {
        return Err(DteError::SiiNetwork(format!(
            "RUT inválido '{rut}', se espera NNNNNNNN-D"
        )));
    }
    if !num.chars().all(|c| c.is_ascii_digit()) {
        return Err(DteError::SiiNetwork(format!(
            "RUT inválido '{rut}', parte numérica no es dígitos"
        )));
    }
    Ok((num.to_string(), dv.to_ascii_uppercase()))
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

// ---------- parsing upload response ----------

/// Parsea la respuesta de `DTEUpload`:
///
/// ```xml
/// <?xml version="1.0"?>
/// <RECEPCIONDTE>
///   <STATUS>0</STATUS>
///   <TRACKID>123456</TRACKID>
///   <TIMESTAMP>2026-05-23T10:15:23</TIMESTAMP>
///   <RUTSENDER>...</RUTSENDER>
///   <DVSENDER>...</DVSENDER>
///   <RUTCOMPANY>...</RUTCOMPANY>
///   <DVCOMPANY>...</DVCOMPANY>
///   <FILE>dte.xml</FILE>
///   <ERROR>opcional, presente cuando STATUS≠0</ERROR>
/// </RECEPCIONDTE>
/// ```
fn parse_upload_xml(body: &str) -> Result<UploadResult, DteError> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut status: Option<String> = None;
    let mut track_id: Option<String> = None;
    let mut timestamp: Option<String> = None;
    let mut error_msg: Option<String> = None;
    let mut current: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(DteError::SiiNetwork(format!(
                    "respuesta upload no-XML: {e}"
                )))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current = Some(name.to_ascii_uppercase());
            }
            Ok(Event::End(_)) => current = None,
            Ok(Event::Text(t)) => {
                if let Some(tag) = &current {
                    let val = t
                        .unescape()
                        .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()))
                        .into_owned();
                    match tag.as_str() {
                        "STATUS" => status = Some(val),
                        "TRACKID" => track_id = Some(val),
                        "TIMESTAMP" => timestamp = Some(val),
                        "ERROR" | "GLOSA" | "DETAIL" => error_msg = Some(val),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }

    let status = status.ok_or_else(|| {
        DteError::SiiNetwork(format!("respuesta sin <STATUS>: {}", truncate(body, 200)))
    })?;

    // STATUS = "0" = envío aceptado para procesar. Cualquier otro = rechazo.
    if status.trim() != "0" {
        let glosa = error_msg.unwrap_or_else(|| format!("STATUS={status}"));
        return Err(DteError::SiiRejected { glosa });
    }

    let track_id_raw =
        track_id.ok_or_else(|| DteError::SiiNetwork("respuesta sin <TRACKID>".to_string()))?;
    let track_id = track_id_raw.trim().parse::<i64>().map_err(|e| {
        DteError::SiiNetwork(format!("<TRACKID>='{track_id_raw}' no numérico: {e}"))
    })?;

    let fecha_recepcion = timestamp
        .as_deref()
        .and_then(parse_sii_timestamp)
        .unwrap_or_else(Utc::now);

    Ok(UploadResult {
        track_id,
        fecha_recepcion,
    })
}

/// SII timestamps: `YYYY-MM-DDTHH:MM:SS` (sin zona). Se interpretan como
/// hora oficial CL y se mantienen como UTC naive — el caller persiste tal cual.
fn parse_sii_timestamp(s: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

// ---------- SOAP envelope poll ----------

/// SOAP 1.1 RPC/encoded envelope para `getEstUp(RutCompania, DvCompania,
/// TrackId, Token)`. WSDL targetNamespace = `http://DefaultNamespace` (sí,
/// literalmente). Params son `xsd:string` por contrato — TrackId como string
/// para evitar problemas con int32 vs int64 en JAX-WS legacy.
fn build_get_est_up_envelope(rut: &str, dv: &str, track_id: i64, token: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\" \
xmlns:def=\"http://DefaultNamespace\" \
xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<soapenv:Body>\
<def:getEstUp soapenv:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<RutCompania xsi:type=\"xsd:string\">{rut}</RutCompania>\
<DvCompania xsi:type=\"xsd:string\">{dv}</DvCompania>\
<TrackId xsi:type=\"xsd:string\">{track_id}</TrackId>\
<Token xsi:type=\"xsd:string\">{token}</Token>\
</def:getEstUp>\
</soapenv:Body>\
</soapenv:Envelope>"
    )
}

/// Extrae `<getEstUpReturn>` del SOAP envelope y delega al parser interno.
/// El response value es un string que SII codifica como XML escapado (el
/// servidor SII JAX-WS rpc/encoded retorna `&lt;SII:RESPUESTA&gt;...` etc).
fn parse_poll_envelope(body: &str) -> Result<PollStatus, DteError> {
    let inner = extract_get_est_up_return(body).ok_or_else(|| {
        DteError::SiiNetwork(format!(
            "respuesta poll sin <getEstUpReturn>: {}",
            truncate(body, 200)
        ))
    })?;
    parse_poll_xml(&inner)
}

/// Saca el text node de `<getEstUpReturn>` (escapado o CDATA). El SOAP server
/// lo envuelve en namespace prefix arbitrario — buscamos por suffix de nombre.
fn extract_get_est_up_return(body: &str) -> Option<String> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut inside = false;
    let mut acc = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => {
                let n = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if n.ends_with("getEstUpReturn") || n.ends_with(":getEstUpReturn") || n == "return"
                {
                    inside = true;
                }
            }
            Ok(Event::End(e)) => {
                let n = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if n.ends_with("getEstUpReturn") || n.ends_with(":getEstUpReturn") || n == "return"
                {
                    break;
                }
            }
            Ok(Event::Text(t)) if inside => {
                let txt = t
                    .unescape()
                    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()))
                    .into_owned();
                acc.push_str(&txt);
            }
            Ok(Event::CData(c)) if inside => {
                acc.push_str(&String::from_utf8_lossy(c.as_ref()));
            }
            _ => {}
        }
        buf.clear();
    }
    if acc.is_empty() {
        None
    } else {
        Some(acc)
    }
}

/// Parsea el XML interno que SII retorna dentro de `<getEstUpReturn>`. La
/// shape canónica documentada SII es:
///
/// ```xml
/// <SII:RESPUESTA xmlns:SII="http://www.sii.cl/XMLSchema">
///   <SII:RESP_HDR>
///     <ESTADO>EPR</ESTADO>
///     <GLOSA_ESTADO>Envio Procesado</GLOSA_ESTADO>
///     <TRACKID>123</TRACKID>
///   </SII:RESP_HDR>
///   <SII:RESP_BODY>...</SII:RESP_BODY>
/// </SII:RESPUESTA>
/// ```
///
/// TODO(sii-spec): la posición exacta de `<TIMESTAMP_RECEPCION>` /
/// `<FECHA_PROC>` varía entre versiones del servicio SII. Aquí busco "primer
/// timestamp parseable que aparezca" como heurística — un dev futuro debe
/// validar contra response real sandbox y refinar a tags exactos cuando esté
/// disponible el manual completo SII OI2004_CEDTE.
fn parse_poll_xml(inner: &str) -> Result<PollStatus, DteError> {
    let mut reader = Reader::from_str(inner);
    reader.config_mut().trim_text(true);

    let mut estado_code: Option<String> = None;
    let mut glosa: Option<String> = None;
    let mut accepted_at: Option<DateTime<Utc>> = None;
    let mut current: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(DteError::SiiNetwork(format!("respuesta poll no-XML: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let raw = String::from_utf8_lossy(e.name().as_ref()).to_string();
                // strip namespace prefix `SII:` etc.
                let local = raw.rsplit(':').next().unwrap_or(&raw).to_ascii_uppercase();
                current = Some(local);
            }
            Ok(Event::End(_)) => current = None,
            Ok(Event::Text(t)) => {
                if let Some(tag) = &current {
                    let val = t
                        .unescape()
                        .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()))
                        .into_owned();
                    match tag.as_str() {
                        "ESTADO" => estado_code = Some(val),
                        // primer hit gana — `GLOSA_ESTADO` es el header.
                        "GLOSA_ESTADO" | "GLOSA" | "GLOSA_ERR" if glosa.is_none() => {
                            glosa = Some(val);
                        }
                        // TODO(sii-spec): refinar tag exacto cuando se tenga
                        // respuesta real sandbox.
                        "TIMESTAMP_RECEPCION" | "FECHA_PROC" | "TIMESTAMP"
                            if accepted_at.is_none() =>
                        {
                            accepted_at = parse_sii_timestamp(&val);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }

    let code = estado_code.ok_or_else(|| {
        DteError::SiiNetwork(format!(
            "respuesta poll sin <ESTADO>: {}",
            truncate(inner, 200)
        ))
    })?;
    let estado = SiiEstado::from_code(&code);
    let glosa = glosa.unwrap_or_else(|| code.clone());

    Ok(PollStatus {
        estado,
        glosa,
        accepted_at,
    })
}

impl SiiEstado {
    /// Mapa código SII string → enum normalizado. Ver doc del enum para la
    /// tabla.
    fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_uppercase().as_str() {
            "EPR" => SiiEstado::Aceptado,
            "SOK" | "REC" => SiiEstado::EnProceso,
            "FOK" | "LOK" | "COK" => SiiEstado::Recibido,
            "RFR" | "RPR" => SiiEstado::AceptadoConReparos,
            "RCH" | "RCT" | "RSC" | "RPT" | "RFT" => SiiEstado::Rechazado,
            _ => SiiEstado::Error,
        }
    }
}

// ---------- tests-only API export ----------
//
// Re-exports privados para que los tests `tests/sii_upload.rs` puedan apuntar
// las funciones a una URL arbitraria (wiremock) sin tocar la API pública.
#[doc(hidden)]
pub mod testing {
    use super::*;

    pub async fn upload_dte_to(
        url: &str,
        signed_xml: &str,
        cert: &[u8],
        cert_pass: &str,
        rut_emisor: &str,
        rut_envia: &str,
    ) -> Result<UploadResult, DteError> {
        super::upload_dte_to(url, signed_xml, cert, cert_pass, rut_emisor, rut_envia).await
    }

    pub async fn poll_status_at(
        url: String,
        track_id: i64,
        rut_consulta: &str,
        token: &str,
    ) -> Result<PollStatus, DteError> {
        super::poll_status_at(url, track_id, rut_consulta, token).await
    }

    pub fn build_get_est_up_envelope(rut: &str, dv: &str, track_id: i64, token: &str) -> String {
        super::build_get_est_up_envelope(rut, dv, track_id, token)
    }

    pub fn query_endpoint(env: SiiEnv) -> String {
        super::query_endpoint(env)
    }
}

// ---------- unit tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_rut_ok() {
        assert_eq!(
            split_rut("76123456-7").unwrap(),
            ("76123456".to_string(), "7".to_string())
        );
        assert_eq!(
            split_rut("76.123.456-K").unwrap(),
            ("76123456".to_string(), "K".to_string())
        );
        assert_eq!(
            split_rut("76123456-k").unwrap(),
            ("76123456".to_string(), "K".to_string())
        );
    }

    #[test]
    fn split_rut_invalido() {
        assert!(split_rut("76123456").is_err());
        assert!(split_rut("foo-7").is_err());
        assert!(split_rut("").is_err());
    }

    #[test]
    fn estado_codes_map() {
        assert_eq!(SiiEstado::from_code("EPR"), SiiEstado::Aceptado);
        assert_eq!(SiiEstado::from_code("epr"), SiiEstado::Aceptado);
        assert_eq!(SiiEstado::from_code("RCH"), SiiEstado::Rechazado);
        assert_eq!(SiiEstado::from_code("REC"), SiiEstado::EnProceso);
        assert_eq!(SiiEstado::from_code("RFR"), SiiEstado::AceptadoConReparos);
        assert_eq!(SiiEstado::from_code("???"), SiiEstado::Error);
    }

    #[test]
    fn query_endpoint_matches_env() {
        assert!(query_endpoint(SiiEnv::Sandbox).contains("maullin.sii.cl"));
        assert!(query_endpoint(SiiEnv::Prod).contains("palena.sii.cl"));
    }
}
