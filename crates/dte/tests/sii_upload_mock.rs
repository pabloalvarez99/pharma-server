//! Subtask 9.1.d — `SiiClient::upload_xml` contra wiremock.
//!
//! Verifica el happy path del POST multipart `cgi_dte/UPL/DTEUpload` y el
//! parseo del `track_id` de la respuesta SII. La negociación seed→token
//! NO se ejercita aquí (requiere XML-DSig real; ver subtask sign).
//!
//! Tests cubren:
//! - upload OK con respuesta XML que incluye `<TRACKID>`.
//! - upload con server 5xx reintenta y eventualmente devuelve error.
//! - upload con 4xx falla inmediatamente sin reintentar.
//! - upload con respuesta sin `<TRACKID>` devuelve SiiNetwork.

use dte::sii::SiiClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RUT: &str = "76123456-7";
const TOKEN: &str = "test-token-abc123";
const XML: &str = r#"<?xml version="1.0"?><DTE></DTE>"#;

#[tokio::test]
async fn upload_devuelve_track_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .and(header("cookie", format!("TOKEN={TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<?xml version="1.0"?><RECEPCIONDTE><TRACKID>987654321</TRACKID></RECEPCIONDTE>"#,
            "text/xml",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = SiiClient::with_base(server.uri());
    let track = client.upload_xml(TOKEN, XML, RUT).await.expect("upload ok");
    assert_eq!(track, 987654321);
}

#[tokio::test]
async fn upload_5xx_reintenta_y_falla() {
    let server = MockServer::start().await;

    // wiremock por default responde 500 a paths sin mock; mejor ser explícito.
    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3) // 3 reintentos
        .mount(&server)
        .await;

    let client = SiiClient::with_base(server.uri());
    let err = client
        .upload_xml(TOKEN, XML, RUT)
        .await
        .expect_err("5xx debe fallar");
    let msg = format!("{err}");
    assert!(
        msg.contains("status 5") || msg.contains("503"),
        "esperado 5xx en mensaje, fue: {msg}"
    );
}

#[tokio::test]
async fn upload_4xx_falla_sin_reintento() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(ResponseTemplate::new(400))
        .expect(1) // sin reintentos
        .mount(&server)
        .await;

    let client = SiiClient::with_base(server.uri());
    let err = client
        .upload_xml(TOKEN, XML, RUT)
        .await
        .expect_err("4xx debe fallar");
    let msg = format!("{err}");
    assert!(
        msg.contains("status 4") || msg.contains("400"),
        "esperado 4xx en mensaje, fue: {msg}"
    );
}

#[tokio::test]
async fn upload_respuesta_sin_track_id_falla() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/cgi_dte/UPL/DTEUpload"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<?xml version="1.0"?><RECEPCIONDTE><ESTADO>OK</ESTADO></RECEPCIONDTE>"#,
            "text/xml",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = SiiClient::with_base(server.uri());
    let err = client
        .upload_xml(TOKEN, XML, RUT)
        .await
        .expect_err("sin TRACKID debe fallar");
    assert!(format!("{err}").contains("TRACKID"));
}
