//! Subtasks 9.1.d (upload SII) + 9.1.e (polling estado).
//!
//! Tests pure-logic con `wiremock` montando un endpoint local que imita la
//! shape XML del SII — NUNCA tocan maullin/palena.

use dte::sii::{testing, SiiEstado};
use dte::DteError;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CERT_FAKE: &[u8] = b"dummy-pfx-bytes";
const CERT_PASS: &str = "ignored";
const RUT_EMISOR: &str = "76123456-7";
const RUT_ENVIA: &str = "11111111-1";

// ============================================================================
// 9.1.d — upload_dte
// ============================================================================

/// Happy path: SII responde 200 + `<RECEPCIONDTE><STATUS>0</STATUS><TRACKID>123</TRACKID></RECEPCIONDTE>`.
/// Esperamos `UploadResult.track_id == 123` y los campos multipart presentes.
#[tokio::test]
async fn upload_happy_path_devuelve_track_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(|req: &Request| {
            // Espía multipart: confirma que el body trae los field names que SII espera.
            let body = String::from_utf8_lossy(&req.body);
            assert!(body.contains("name=\"rutSender\""), "falta rutSender");
            assert!(body.contains("name=\"dvSender\""), "falta dvSender");
            assert!(body.contains("name=\"rutCompany\""), "falta rutCompany");
            assert!(body.contains("name=\"archivo\""), "falta archivo");
            assert!(body.contains("11111111"), "rut envia separado mal");
            assert!(body.contains("76123456"), "rut emisor separado mal");
            ResponseTemplate::new(200).set_body_string(
                r#"<?xml version="1.0"?>
<RECEPCIONDTE>
  <STATUS>0</STATUS>
  <TRACKID>123</TRACKID>
  <TIMESTAMP>2026-05-23T10:15:23</TIMESTAMP>
  <RUTSENDER>11111111</RUTSENDER>
  <DVSENDER>1</DVSENDER>
  <RUTCOMPANY>76123456</RUTCOMPANY>
  <DVCOMPANY>7</DVCOMPANY>
  <FILE>dte.xml</FILE>
</RECEPCIONDTE>"#,
            )
        })
        .mount(&server)
        .await;

    let url = format!("{}/cgi_dte/UPL/DTEUpload", server.uri());
    let result = testing::upload_dte_to(
        &url,
        "<DTE>fake-signed</DTE>",
        CERT_FAKE,
        CERT_PASS,
        RUT_EMISOR,
        RUT_ENVIA,
    )
    .await
    .expect("upload happy-path debe devolver Ok");

    assert_eq!(result.track_id, 123);
    assert_eq!(
        result
            .fecha_recepcion
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        "2026-05-23T10:15:23"
    );
}

/// HTTP 5xx → `DteError::SiiNetwork`. SII rara vez devuelve 5xx pero cuando
/// ocurre el caller debe reintentar con backoff (responsabilidad del scheduler).
#[tokio::test]
async fn upload_5xx_es_sii_network() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
        .mount(&server)
        .await;

    let url = format!("{}/cgi_dte/UPL/DTEUpload", server.uri());
    let err = testing::upload_dte_to(&url, "<DTE/>", CERT_FAKE, CERT_PASS, RUT_EMISOR, RUT_ENVIA)
        .await
        .expect_err("5xx debe ser error");

    match err {
        DteError::SiiNetwork(msg) => {
            assert!(msg.contains("503"), "msg debe traer status: {msg}");
        }
        other => panic!("esperado SiiNetwork, got {other:?}"),
    }
}

/// SII devuelve `<STATUS>` ≠ 0 + `<ERROR>` → `DteError::SiiRejected` con la
/// glosa pasada through.
#[tokio::test]
async fn upload_status_nonzero_es_sii_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0"?>
<RECEPCIONDTE>
  <STATUS>3</STATUS>
  <ERROR>Empresa no autorizada para enviar boletas electrónicas</ERROR>
  <TIMESTAMP>2026-05-23T10:15:23</TIMESTAMP>
</RECEPCIONDTE>"#,
        ))
        .mount(&server)
        .await;

    let url = format!("{}/cgi_dte/UPL/DTEUpload", server.uri());
    let err = testing::upload_dte_to(&url, "<DTE/>", CERT_FAKE, CERT_PASS, RUT_EMISOR, RUT_ENVIA)
        .await
        .expect_err("STATUS!=0 debe ser rejected");

    match err {
        DteError::SiiRejected { glosa } => {
            assert!(
                glosa.contains("no autorizada"),
                "glosa debe traer mensaje SII: {glosa}"
            );
        }
        other => panic!("esperado SiiRejected, got {other:?}"),
    }
}

/// Respuesta XML mal formada → `SiiNetwork`. Defensa contra middleware que
/// inyecta HTML de error o body vacío.
#[tokio::test]
async fn upload_xml_malformado_es_sii_network() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>Maintenance window</body></html>"),
        )
        .mount(&server)
        .await;

    let url = format!("{}/cgi_dte/UPL/DTEUpload", server.uri());
    let err = testing::upload_dte_to(&url, "<DTE/>", CERT_FAKE, CERT_PASS, RUT_EMISOR, RUT_ENVIA)
        .await
        .expect_err("respuesta sin <STATUS> debe fallar");

    assert!(matches!(err, DteError::SiiNetwork(_)));
}

// ============================================================================
// 9.1.e — poll_status
// ============================================================================

/// `getEstUp` retorna `EPR` (envío procesado) → `SiiEstado::Aceptado`.
/// El envelope SOAP confirma que pasamos RutCompania/DvCompania/TrackId.
#[tokio::test]
async fn poll_aceptado_devuelve_estado_aceptado() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .and(header("Content-Type", "text/xml; charset=utf-8"))
        .and(body_string_contains("getEstUp"))
        .and(body_string_contains("76123456"))
        .and(body_string_contains(">7<"))
        .and(body_string_contains(">7777<"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/">
  <soapenv:Body>
    <ns:getEstUpResponse xmlns:ns="http://DefaultNamespace">
      <getEstUpReturn xsi:type="xsd:string" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
        &lt;?xml version="1.0"?&gt;
        &lt;SII:RESPUESTA xmlns:SII="http://www.sii.cl/XMLSchema"&gt;
          &lt;SII:RESP_HDR&gt;
            &lt;ESTADO&gt;EPR&lt;/ESTADO&gt;
            &lt;GLOSA_ESTADO&gt;Envio Procesado&lt;/GLOSA_ESTADO&gt;
            &lt;TRACKID&gt;7777&lt;/TRACKID&gt;
            &lt;TIMESTAMP_RECEPCION&gt;2026-05-23T10:20:00&lt;/TIMESTAMP_RECEPCION&gt;
          &lt;/SII:RESP_HDR&gt;
        &lt;/SII:RESPUESTA&gt;
      </getEstUpReturn>
    </ns:getEstUpResponse>
  </soapenv:Body>
</soapenv:Envelope>"#,
        ))
        .mount(&server)
        .await;

    let url = format!("{}/DTEWS/QueryEstUp.jws", server.uri());
    let st = testing::poll_status_at(url, 7777, RUT_EMISOR, "")
        .await
        .expect("poll happy-path");

    assert_eq!(st.estado, SiiEstado::Aceptado);
    assert!(
        st.glosa.contains("Procesado"),
        "glosa debe venir del SII: {}",
        st.glosa
    );
    assert!(st.accepted_at.is_some(), "TIMESTAMP_RECEPCION debe parsear");
}

/// `getEstUp` retorna `RCH` → `SiiEstado::Rechazado`.
#[tokio::test]
async fn poll_rechazado_devuelve_estado_rechazado() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/">
  <soapenv:Body>
    <ns:getEstUpResponse xmlns:ns="http://DefaultNamespace">
      <getEstUpReturn xsi:type="xsd:string" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
        &lt;SII:RESPUESTA xmlns:SII="http://www.sii.cl/XMLSchema"&gt;
          &lt;SII:RESP_HDR&gt;
            &lt;ESTADO&gt;RCH&lt;/ESTADO&gt;
            &lt;GLOSA_ESTADO&gt;Rechazo de Envio&lt;/GLOSA_ESTADO&gt;
            &lt;TRACKID&gt;7777&lt;/TRACKID&gt;
          &lt;/SII:RESP_HDR&gt;
        &lt;/SII:RESPUESTA&gt;
      </getEstUpReturn>
    </ns:getEstUpResponse>
  </soapenv:Body>
</soapenv:Envelope>"#,
        ))
        .mount(&server)
        .await;

    let url = format!("{}/DTEWS/QueryEstUp.jws", server.uri());
    let st = testing::poll_status_at(url, 7777, RUT_EMISOR, "")
        .await
        .expect("poll rechazo");

    assert_eq!(st.estado, SiiEstado::Rechazado);
    assert!(st.glosa.contains("Rechazo"));
    assert!(st.accepted_at.is_none(), "rechazo no trae fecha proc");
}

/// HTTP 5xx en poll → `SiiNetwork`.
#[tokio::test]
async fn poll_5xx_es_sii_network() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .respond_with(ResponseTemplate::new(500).set_body_string("oops"))
        .mount(&server)
        .await;

    let url = format!("{}/DTEWS/QueryEstUp.jws", server.uri());
    let err = testing::poll_status_at(url, 1, RUT_EMISOR, "")
        .await
        .expect_err("5xx en poll debe fallar");
    assert!(matches!(err, DteError::SiiNetwork(_)));
}

// ============================================================================
// extras: envelope shape + endpoint derivation
// ============================================================================

/// Sanity: el SOAP envelope tiene los 4 params SII en orden + namespace
/// `http://DefaultNamespace` literal (sí, así viene en el WSDL).
#[test]
fn soap_envelope_contiene_params_en_orden() {
    let env = testing::build_get_est_up_envelope("76123456", "7", 999, "tok");
    let i_rut = env.find("RutCompania").unwrap();
    let i_dv = env.find("DvCompania").unwrap();
    let i_tk = env.find("TrackId").unwrap();
    let i_token = env.find("Token").unwrap();
    assert!(i_rut < i_dv && i_dv < i_tk && i_tk < i_token, "orden SII");
    assert!(env.contains(">999<"), "track id como string en body");
    assert!(env.contains("http://DefaultNamespace"));
    assert!(env.contains("getEstUp"));
}

#[test]
fn query_endpoint_sandbox_vs_prod() {
    use dte::SiiEnv;
    assert!(testing::query_endpoint(SiiEnv::Sandbox).contains("maullin.sii.cl"));
    assert!(testing::query_endpoint(SiiEnv::Prod).contains("palena.sii.cl"));
}
