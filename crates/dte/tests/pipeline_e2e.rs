//! Subtask 9.1.l — pipeline end-to-end de una boleta electrónica (39).
//!
//! Ejercita el flujo completo de armado de un DTE tal como lo correría el
//! servidor en producción, sin tocar red SII:
//!
//! 1. **Folio**: `caf::assign_next` asigna atómicamente el próximo folio del
//!    CAF activo del tenant (kv-mem, mismo setup que `caf_folio_atomic.rs`).
//! 2. **XML**: `xml::render_unsigned` renderiza el `<DTE>` schema SII xsd 1.0.
//! 3. **TED**: `timbre::generate` produce el `<TED>` firmado RSA-SHA1 con la
//!    RSASK embebida en el CAF.
//! 4. **Ensamblado**: el TED se inyecta en el XML antes de `<TmstFirma>` (el
//!    punto donde SII lo exige) y se verifica que el documento resultante
//!    tenga el folio en rango, el TED presente y el timbre dentro del XML.
//!
//! El paso de envío a SII (`sii::upload_dte`) tiene cobertura propia en
//! `sii_upload.rs` (wiremock); aquí se prueba sólo el armado offline, que es
//! el flujo del **tier Free** (genera local + el operador sube manual al
//! portal SII). El gating Pro/Business (`integrations.sii_dte_auto`) vive en
//! `crates/license::gate` y se documenta en `docs/dte/onboarding-cliente.md`.

mod common;

use common::{caf_xml_synthetic, dte_boleta_minimal, emisor_test};
use dte::caf::assign_next;
use dte::timbre::{generate, verify};
use dte::xml::render_unsigned;
use dte::{Caf, DteTipo};
use std::sync::Arc;
use surrealdb::engine::local::{Db, Mem};
use surrealdb::sql::Thing;
use surrealdb::Surreal;

/// Levanta una DB kv-mem con un tenant y un CAF activo cuyo `xml` es un CAF
/// synthetic real (con par RSA). Devuelve el handle, el `Thing` del tenant y
/// la struct `Caf` in-memory equivalente (misma RSASK/RSAPUBK que el record),
/// para poder generar y verificar el TED contra el mismo material de clave.
async fn setup_db_con_caf(
    rut: &str,
    tipo: DteTipo,
    desde: i64,
    hasta: i64,
) -> (Arc<Surreal<Db>>, Thing, Caf) {
    let (caf_xml, caf) = caf_xml_synthetic(rut, tipo, desde, hasta);
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    db.query("CREATE tenant:t1 SET name = 'farmacia test';")
        .await
        .unwrap()
        .check()
        .unwrap();
    // El record CAF persiste el XML entero (igual que `caf::parse_xml` →
    // persistencia real); `assign_next` sólo toca el puntero `next_folio`.
    db.query(
        "CREATE caf:c1 SET \
            tenant = tenant:t1, \
            tipo_dte = $tipo, \
            folio_desde = $desde, \
            folio_hasta = $hasta, \
            next_folio = $desde, \
            fecha_autorizacion = time::now(), \
            rut_emisor = $rut, \
            xml = $xml, \
            activo = true;",
    )
    .bind(("tipo", tipo.code()))
    .bind(("desde", desde))
    .bind(("hasta", hasta))
    .bind(("rut", rut.to_string()))
    .bind(("xml", caf_xml))
    .await
    .unwrap()
    .check()
    .unwrap();
    (Arc::new(db), Thing::from(("tenant", "t1")), caf)
}

/// Happy path completo: folio → XML → TED → ensamblado verificado.
#[tokio::test]
async fn boleta_pipeline_happy_path() {
    let rut = "76123456-7";
    let (db, tenant, caf) = setup_db_con_caf(rut, DteTipo::BoletaElectronica, 1, 100).await;

    // 1. Folio atómico desde el CAF activo.
    let (caf_rec, folio) = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
        .await
        .expect("assign_next ok");
    assert_eq!(folio, 1, "primer folio del rango");
    assert!(
        folio >= caf_rec.folio_desde && folio <= caf_rec.folio_hasta,
        "folio {folio} dentro de rango [{}..{}]",
        caf_rec.folio_desde,
        caf_rec.folio_hasta
    );
    assert_eq!(caf_rec.next_folio, 2, "puntero avanza tras asignar");

    // El DTE toma el folio recién asignado.
    let mut dte = dte_boleta_minimal(folio);
    dte.folio = folio;

    // 2. Render XML sin firma (schema SII).
    let xml = render_unsigned(&dte, &emisor_test()).expect("render XML");
    assert!(xml.contains(&format!("<Folio>{folio}</Folio>")));
    assert!(xml.contains("<TmstFirma>"));

    // 3. TED firmado con la RSASK del CAF.
    let ted = generate(&dte, &caf).expect("generar TED");
    assert!(ted.contains(r#"<TED version="1.0">"#));
    assert!(ted.contains(r#"<FRMT algoritmo="SHA1withRSA">"#));
    // El folio firmado en el DD del TED es el mismo que el asignado.
    assert!(
        ted.contains(&format!("<F>{folio}</F>")),
        "folio en DD del TED"
    );

    // 4. Ensamblado: inyectar el TED antes de <TmstFirma> (posición SII).
    let dte_xml = inyectar_ted(&xml, &ted);
    assert!(
        dte_xml.contains(r#"<TED version="1.0">"#),
        "timbre dentro del XML"
    );
    let pos_ted = dte_xml.find("<TED").unwrap();
    let pos_tmst = dte_xml.find("<TmstFirma>").unwrap();
    assert!(pos_ted < pos_tmst, "TED debe preceder a TmstFirma");

    // El TED ensamblado sigue verificando (firma íntegra dentro del XML).
    verify(&dte, &caf, &ted).expect("verify TED ensamblado");
}

/// CAF agotado: tras consumir todos los folios, el siguiente intento corta el
/// pipeline en el paso 1 con `FolioExhausted` (no se renderiza XML).
#[tokio::test]
async fn boleta_pipeline_folio_agotado() {
    let (db, tenant, _caf) = setup_db_con_caf("76123456-7", DteTipo::BoletaElectronica, 1, 2).await;
    // Consumir los 2 folios disponibles.
    for esperado in 1..=2 {
        let (_, folio) = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
            .await
            .unwrap();
        assert_eq!(folio, esperado);
    }
    let err = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
        .await
        .expect_err("CAF agotado debe fallar");
    assert!(
        matches!(err, dte::DteError::FolioExhausted { .. }),
        "esperado FolioExhausted, obtuve: {err:?}"
    );
}

/// Tipo no soportado por el renderer: el pipeline corta en el paso 2. La
/// factura (33) tiene folio asignable pero `render_unsigned` aún no la
/// soporta (subtask 9.1.f) y devuelve `XmlInvalid`.
#[tokio::test]
async fn boleta_pipeline_tipo_no_soportado_en_render() {
    let (db, tenant, _caf) =
        setup_db_con_caf("76123456-7", DteTipo::FacturaElectronica, 1, 50).await;
    let (_, folio) = assign_next(&db, &tenant, DteTipo::FacturaElectronica)
        .await
        .expect("folio factura asignado");

    let mut dte = dte_boleta_minimal(folio);
    dte.folio = folio;
    dte.tipo = DteTipo::FacturaElectronica;

    let err = render_unsigned(&dte, &emisor_test()).expect_err("factura aún no soportada");
    let msg = format!("{err}");
    assert!(
        msg.contains("33") || msg.contains("9.1.f"),
        "esperado tipo no soportado, msg: {msg}"
    );
}

/// Tamper: modificar el XML/TED tras firmar invalida la verificación. Simula
/// un atacante (o corrupción) que altera el monto en el DD ya ensamblado.
#[tokio::test]
async fn boleta_pipeline_tamper_post_firma_falla_verify() {
    let (db, tenant, caf) =
        setup_db_con_caf("76123456-7", DteTipo::BoletaElectronica, 1, 100).await;
    let (_, folio) = assign_next(&db, &tenant, DteTipo::BoletaElectronica)
        .await
        .unwrap();
    let mut dte = dte_boleta_minimal(folio);
    dte.folio = folio;

    let xml = render_unsigned(&dte, &emisor_test()).unwrap();
    let ted = generate(&dte, &caf).unwrap();
    let dte_xml = inyectar_ted(&xml, &ted);

    // Tamper dentro del DD firmado: cambiar el monto total <MNT>.
    let tampered_xml = dte_xml.replace("<MNT>1000</MNT>", "<MNT>1</MNT>");
    assert_ne!(tampered_xml, dte_xml, "el tamper debe modificar el XML");
    // Re-extraer el TED tampered y verificarlo contra el DTE original: la
    // firma RSA-SHA1 ya no cubre el DD alterado.
    let ted_tampered = extraer_ted(&tampered_xml);
    let err = verify(&dte, &caf, &ted_tampered).expect_err("verify debe fallar tras tamper");
    let msg = format!("{err}");
    assert!(
        msg.contains("no coincide") || msg.contains("verificar"),
        "esperado fallo de firma por DD alterado, msg: {msg}"
    );
}

// ---------- helpers de ensamblado (sólo para estos tests) ----------

/// Inyecta el bloque `<TED>` inmediatamente antes de `<TmstFirma>`, que es la
/// posición que exige la xsd SII dentro de `<Documento>`.
fn inyectar_ted(xml: &str, ted: &str) -> String {
    let marker = "<TmstFirma>";
    let pos = xml.find(marker).expect("XML debe tener <TmstFirma>");
    let mut out = String::with_capacity(xml.len() + ted.len());
    out.push_str(&xml[..pos]);
    out.push_str(ted);
    out.push_str(&xml[pos..]);
    out
}

/// Extrae el subtree `<TED>...</TED>` de un XML ensamblado.
fn extraer_ted(xml: &str) -> String {
    let s = xml.find("<TED").expect("TED presente");
    let close = "</TED>";
    let e = xml[s..].find(close).expect("cierre </TED>");
    xml[s..s + e + close.len()].to_string()
}
