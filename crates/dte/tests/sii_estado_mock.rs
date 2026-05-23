//! Subtask 9.1.e — `SiiClient::query_estado` contra wiremock.
//!
//! Verifica el POST SOAP a `/DTEWS/QueryEstUp.jws` y el parseo de los
//! tres estados terminales SII: EnProceso, Aceptado, Rechazado{glosa}.

use dte::sii::{SiiClient, SiiEstado};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RUT: &str = "76123456-7";
const TOKEN: &str = "test-token";

async fn mock_with(status_xml: &str) -> (MockServer, SiiClient) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(status_xml.to_string(), "text/xml"))
        .expect(1)
        .mount(&server)
        .await;
    let client = SiiClient::with_base(server.uri());
    (server, client)
}

#[tokio::test]
async fn estado_en_proceso() {
    let body = r#"<?xml version="1.0"?><RESP><ESTADO>EPR</ESTADO></RESP>"#;
    let (_server, client) = mock_with(body).await;
    let est = client.query_estado(TOKEN, 1, RUT).await.expect("query ok");
    assert_eq!(est, SiiEstado::EnProceso);
}

#[tokio::test]
async fn estado_aceptado() {
    let body = r#"<?xml version="1.0"?><RESP><ESTADO>DOK</ESTADO></RESP>"#;
    let (_server, client) = mock_with(body).await;
    let est = client.query_estado(TOKEN, 1, RUT).await.unwrap();
    assert_eq!(est, SiiEstado::Aceptado);
}

#[tokio::test]
async fn estado_rechazado_con_glosa() {
    let body =
        r#"<?xml version="1.0"?><RESP><ESTADO>RCH</ESTADO><GLOSA>folio duplicado</GLOSA></RESP>"#;
    let (_server, client) = mock_with(body).await;
    let est = client.query_estado(TOKEN, 1, RUT).await.unwrap();
    assert_eq!(
        est,
        SiiEstado::Rechazado {
            glosa: "folio duplicado".into(),
        }
    );
}

#[tokio::test]
async fn estado_5xx_reintenta_y_falla() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .respond_with(ResponseTemplate::new(500))
        .expect(3)
        .mount(&server)
        .await;
    let client = SiiClient::with_base(server.uri());
    let err = client
        .query_estado(TOKEN, 1, RUT)
        .await
        .expect_err("5xx debe fallar");
    let msg = format!("{err}");
    assert!(
        msg.contains("status 5") || msg.contains("500"),
        "esperado 5xx en mensaje: {msg}"
    );
}

#[tokio::test]
async fn estado_4xx_falla_sin_reintento() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/DTEWS/QueryEstUp.jws"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let client = SiiClient::with_base(server.uri());
    let err = client
        .query_estado(TOKEN, 1, RUT)
        .await
        .expect_err("4xx debe fallar");
    let msg = format!("{err}");
    assert!(
        msg.contains("status 4") || msg.contains("401"),
        "esperado 4xx en mensaje: {msg}"
    );
}
