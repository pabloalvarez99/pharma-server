//! Firma AWS Signature V4 para los tres verbos que usa el respaldo.
//!
//! ## Por qué a mano y no `aws-sdk-s3`
//!
//! Se usan exactamente tres operaciones (`PUT`, `GET`, `DELETE` de un objeto),
//! sin multipart, sin streaming, sin presign, sin descubrimiento de región.
//! Todo lo que la firma necesita —`hmac`, `sha2`, `hex`, `reqwest`, `chrono`—
//! ya está en `crates/api/Cargo.toml`: esto no agrega **ninguna** dependencia.
//! `aws-sdk-s3` traería del orden de 60 crates a un workspace que además se
//! compila para Android, a cambio de nada que acá se use.
//!
//! El riesgo de escribir la firma uno mismo es acotado y ruidoso: si está mal,
//! S3 contesta 403 y no se sube nada. No hay forma de que una firma mal hecha
//! filtre un respaldo — el ciphertext ya viaja cifrado desde el teléfono y el
//! secreto de la cuenta nunca sale del server.
//!
//! Los dos eslabones que se pueden pinear contra números **oficiales** de AWS
//! —la derivación de la llave y el formato del canonical request— están
//! pineados contra ellos en los tests del final. El HMAC final se fija contra
//! regresión, verificado contra una implementación independiente.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Credenciales del bucket. Vive sólo en memoria del server.
///
/// `Debug` está escrito a mano: el derive imprimiría el secreto en cualquier
/// `tracing::error!(?creds)` y de ahí a un log en disco hay un paso.
#[derive(Clone)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

impl std::fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Credentials")
            .field("access_key_id", &"<oculto>")
            .field("secret_access_key", &"<oculto>")
            .field("region", &self.region)
            .finish()
    }
}

/// Lo que hay que firmar de un request.
pub struct SignRequest<'a> {
    pub method: &'a str,
    /// Path absoluto ya codificado (`/bucket/key`).
    pub canonical_uri: &'a str,
    /// Host del endpoint, sin esquema.
    pub host: &'a str,
    /// `sha256` hex del cuerpo (`EMPTY_PAYLOAD_SHA256` si no hay).
    pub payload_sha256_hex: &'a str,
    /// `YYYYMMDDTHHMMSSZ` en UTC.
    pub amz_date: &'a str,
}

/// Cabeceras a mandar, ya firmadas: `(nombre, valor)`.
pub fn sign(req: &SignRequest<'_>, creds: &S3Credentials) -> Vec<(String, String)> {
    let date_stamp = &req.amz_date[..8];
    let scope = format!("{}/{}/s3/aws4_request", date_stamp, creds.region);

    // Sólo se firman las tres cabeceras que siempre mandamos. Firmar de menos
    // es seguro (S3 valida las firmadas); firmar de más sin mandarlas, no.
    let canonical_headers = format!(
        "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        req.host, req.payload_sha256_hex, req.amz_date
    );
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        req.method,
        req.canonical_uri,
        "", // sin query string en ninguna de las tres operaciones
        canonical_headers,
        signed_headers,
        req.payload_sha256_hex
    );

    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        req.amz_date,
        scope,
        hex_sha256(canonical_request.as_bytes())
    );

    let signature = hex_lower(&hmac(
        &signing_key(&creds.secret_access_key, date_stamp, &creds.region),
        string_to_sign.as_bytes(),
    ));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        creds.access_key_id, scope, signed_headers, signature
    );

    vec![
        ("x-amz-date".into(), req.amz_date.into()),
        (
            "x-amz-content-sha256".into(),
            req.payload_sha256_hex.into(),
        ),
        ("authorization".into(), authorization),
    ]
}

/// Cadena de derivación: `AWS4<secret>` → fecha → región → servicio → request.
fn signing_key(secret: &str, date_stamp: &str, region: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date_stamp.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, b"s3");
    hmac(&k_service, b"aws4_request")
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC acepta cualquier largo");
    m.update(msg);
    m.finalize().into_bytes().to_vec()
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_lower(&h.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Codifica un segmento de path para la URI canónica de S3.
///
/// S3 exige que la URI canónica esté codificada **igual** que la del request,
/// y que `/` quede sin codificar. Los caracteres no reservados (RFC 3986)
/// pasan tal cual; el resto va en `%XX` con hex en mayúsculas.
pub fn uri_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La cadena de derivación de la llave, contra el vector que **publica
    /// AWS** en "Examples of how to derive a signing key for Signature Version
    /// 4". Es el eslabón más fácil de romper (un orden distinto, un `AWS4`
    /// olvidado) y el único con un número oficial que se puede citar sin
    /// ambigüedad, así que va solo en su propio test.
    ///
    /// El servicio es `iam` y no `s3` porque así lo publica AWS; la función
    /// toma el servicio fijo en `s3`, por eso acá se reconstruye a mano.
    #[test]
    fn la_derivacion_de_la_llave_calza_con_el_vector_de_aws() {
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let k_date = hmac(format!("AWS4{secret}").as_bytes(), b"20120215");
        let k_region = hmac(&k_date, b"us-east-1");
        let k_service = hmac(&k_region, b"iam");
        let k_signing = hmac(&k_service, b"aws4_request");
        assert_eq!(
            hex_lower(&k_signing),
            "f4780e2d9f65fa895f9c67b32ce1baf0b0d8a43505a000a1a9e090d414db404d",
            "la cadena AWS4·secreto → fecha → región → servicio → aws4_request está mal"
        );
    }

    /// El canonical request, contra el hash que **publica AWS** para su
    /// ejemplo "GET Object" (*Signature Version 4 signing process*).
    ///
    /// Este número fija el formato exacto: el orden de las líneas, el query
    /// vacío, el `\n` de más después de las cabeceras canónicas, y que los
    /// nombres de cabecera vayan en minúscula y ordenados. Es donde se comete
    /// el 90% de los errores de SigV4 y S3 los devuelve como un 403 mudo.
    ///
    /// El ejemplo de AWS firma también `range`; se reconstruye con sus mismas
    /// cabeceras para poder comparar contra su número.
    #[test]
    fn el_canonical_request_calza_con_el_vector_de_aws() {
        let canonical_headers = concat!(
            "host:examplebucket.s3.amazonaws.com\n",
            "range:bytes=0-9\n",
            "x-amz-content-sha256:",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n",
            "x-amz-date:20130524T000000Z\n"
        );
        let canonical_request = format!(
            "GET\n/test.txt\n\n{canonical_headers}\nhost;range;x-amz-content-sha256;x-amz-date\n{EMPTY_PAYLOAD_SHA256}"
        );
        assert_eq!(
            hex_sha256(canonical_request.as_bytes()),
            "7344ae5b7ee6c3e7e6b0fe0640412a37625d1fbfff95c48bbb2dc43964946972",
            "el canonical request no tiene el formato que espera S3"
        );
    }

    /// El último eslabón: `HMAC(llave_de_firma, string_to_sign)`.
    ///
    /// **Este número no es de AWS**: es el resultado de correr la cadena
    /// completa, verificado contra una implementación independiente (Python
    /// `hmac`/`hashlib`) el 2026-08-10. Los dos eslabones de arriba sí están
    /// pineados contra vectores oficiales, así que lo que queda por cubrir acá
    /// es una sola llamada a HMAC — y esto la fija contra regresiones.
    ///
    /// Se deja anotado el origen a propósito: un número mágico sin decir de
    /// dónde salió es el que después nadie se anima a tocar.
    #[test]
    fn la_firma_final_no_cambia_sin_que_nadie_se_entere() {
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let string_to_sign = concat!(
            "AWS4-HMAC-SHA256\n",
            "20130524T000000Z\n",
            "20130524/us-east-1/s3/aws4_request\n",
            "7344ae5b7ee6c3e7e6b0fe0640412a37625d1fbfff95c48bbb2dc43964946972"
        );
        let sig = hex_lower(&hmac(
            &signing_key(secret, "20130524", "us-east-1"),
            string_to_sign.as_bytes(),
        ));
        assert_eq!(
            sig, "67fe34c8530db585abddc51067328adfedb6e42487d2566dc7d927d6e2722900",
            "cambió la firma para una entrada fija"
        );
    }

    #[test]
    fn sha256_del_cuerpo_vacio_es_la_constante_conocida() {
        assert_eq!(hex_sha256(b""), EMPTY_PAYLOAD_SHA256);
    }

    #[test]
    fn arma_las_tres_cabeceras_y_no_filtra_el_secreto() {
        let creds = S3Credentials {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            region: "auto".into(),
        };
        let headers = sign(
            &SignRequest {
                method: "PUT",
                canonical_uri: "/respaldos/user-backup/t/abc",
                host: "acct.r2.cloudflarestorage.com",
                payload_sha256_hex: EMPTY_PAYLOAD_SHA256,
                amz_date: "20260809T120000Z",
            },
            &creds,
        );
        let names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            vec!["x-amz-date", "x-amz-content-sha256", "authorization"]
        );
        let auth = &headers[2].1;
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20260809/auto/s3/aws4_request"));
        assert!(
            !auth.contains("wJalrXUtnFEMI"),
            "el secreto no puede aparecer en la cabecera"
        );
        // El Debug tampoco puede imprimirlo: va a los logs.
        let dbg = format!("{creds:?}");
        assert!(!dbg.contains("wJalrXUtnFEMI") && !dbg.contains("AKIAIOSFODNN7EXAMPLE"), "{dbg}");
    }

    #[test]
    fn el_path_se_codifica_dejando_las_barras() {
        assert_eq!(uri_encode_path("/a/b-c_d.e~f"), "/a/b-c_d.e~f");
        assert_eq!(uri_encode_path("/a b"), "/a%20b");
        assert_eq!(uri_encode_path("/a+b?c"), "/a%2Bb%3Fc");
        // `.` y `/` son legales en un path S3, así que codificar NO es lo que
        // frena un `../`. Eso lo frena la construcción de la clave
        // (`store::object_key`), donde está el test que lo prueba.
        assert_eq!(uri_encode_path("/x/../y"), "/x/../y");
    }
}
