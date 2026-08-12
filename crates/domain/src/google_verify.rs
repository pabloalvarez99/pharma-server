//! Verificación del `id_token` de Google (ADR-0022) — **sin red y sin secretos**.
//!
//! Las formas de wire están congeladas en [`crate::google_identity`]. Acá vive
//! la única pregunta que importa: *¿este string es un token que Google emitió
//! para nosotros, y de quién?*
//!
//! **Es una función pura de `(token, llaves, aud esperado, ahora)`.** No baja
//! JWKS, no toca base de datos, no lee config. Quien la llama trae las llaves
//! (el caché vive en `api`) y el reloj. Por eso la suite entera corre sin red y
//! sin esperar a que existan las credenciales de la consola de Google.
//!
//! ## Qué se chequea, y por qué cada uno
//!
//! | Chequeo | Si falta |
//! |---------|----------|
//! | `alg` es RS256 | Un token `alg: none` entra sin firma. |
//! | Firma contra la llave del `kid` | Cualquiera emite tokens. |
//! | `iss` ∈ las dos formas de Google | — |
//! | **`aud` == nuestro client id** | Un token emitido para *otra* app de Google entra a la nuestra. Es el que se olvida. |
//! | `exp` no vencido | Un token viejo sirve para siempre. |
//! | `sub` presente y no vacío | No hay a quién ligar. |
//!
//! ## La identidad es `sub`, nunca el email
//!
//! [`IdentidadGoogleVerificada::sub`] es lo que se liga a un usuario. El email
//! viaja **sólo para mostrar**: un correo puede cambiar de dueño —alguien deja
//! la empresa, el dominio reasigna la casilla— y quien lo reciba después entraría
//! a un negocio que no es suyo. `sub` es estable y no se reasigna.

use serde::{Deserialize, Serialize};

/// Los dos `iss` que Google emite. Las dos formas son válidas y aparecen en
/// tokens reales; aceptar sólo una deja afuera a la mitad de los aparatos.
pub const GOOGLE_ISSUERS: [&str; 2] = ["accounts.google.com", "https://accounts.google.com"];

/// Una llave pública RSA del JWKS de Google.
///
/// `n` y `e` quedan como los manda Google (base64url, sin decodificar): es lo
/// que consume `jsonwebtoken` y evita un round-trip de decode/encode que sólo
/// serviría para introducir bugs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleJwk {
    pub kid: String,
    #[serde(default)]
    pub kty: String,
    #[serde(default)]
    pub alg: String,
    /// Módulo RSA, base64url.
    pub n: String,
    /// Exponente público, base64url.
    pub e: String,
}

/// El set de llaves públicas de Google (`https://www.googleapis.com/oauth2/v3/certs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoogleJwks {
    #[serde(default)]
    pub keys: Vec<GoogleJwk>,
}

impl GoogleJwks {
    pub fn buscar(&self, kid: &str) -> Option<&GoogleJwk> {
        self.keys.iter().find(|k| k.kid == kid)
    }

    pub fn vacio(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Quién entró, según Google, después de verificar la firma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentidadGoogleVerificada {
    /// **La identidad estable.** Es lo que se liga a un usuario nuestro.
    pub sub: String,
    /// Sólo para mostrar y para el primer vínculo. Nunca para buscar sesión.
    pub email: Option<String>,
    /// Google dice que verificó ese correo. Un email sin verificar no sirve
    /// ni siquiera para el primer vínculo.
    pub email_verificado: bool,
    pub nombre: Option<String>,
    /// `exp` del token de Google (no el de nuestra sesión).
    pub expira_en: i64,
}

/// Por qué no entró. Cada variante es un chequeo distinto a propósito: el
/// handler responde distinto a "llave desconocida" (refrescar JWKS y
/// reintentar) que a "aud ajeno" (rechazar y no reintentar nunca).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorVerificacionGoogle {
    #[error("token mal formado")]
    MalFormado,

    /// El `kid` no está en el JWKS que tenemos. **No es fatal**: Google rota
    /// llaves y el caché puede estar viejo. Quien llama refresca y reintenta
    /// una vez. Si vuelve a pasar con JWKS fresco, ahí sí el token es basura.
    #[error("kid desconocido")]
    LlaveDesconocida { kid: String },

    /// Sólo RS256. Cerrar esto es lo que impide el ataque de `alg: none` y el
    /// de confusión de algoritmo (firmar HS256 con la clave pública RSA).
    #[error("algoritmo no soportado")]
    AlgoritmoNoSoportado { alg: String },

    #[error("firma inválida")]
    FirmaInvalida,

    #[error("emisor inesperado")]
    EmisorInvalido { iss: String },

    /// El token era para otra aplicación de Google.
    #[error("audiencia inesperada")]
    AudienciaInvalida,

    #[error("token vencido")]
    Vencido,

    #[error("token sin sub")]
    SinSujeto,

    /// El JWKS que nos pasaron no tiene llaves. Es un problema nuestro (red o
    /// caché), no del token: se responde 503, no 401.
    #[error("sin llaves de Google")]
    SinLlaves,
}

/// Claims que nos interesan del `id_token`. Google manda más; se ignoran.
#[derive(Debug, Deserialize)]
struct ClaimsGoogle {
    sub: Option<String>,
    iss: String,
    aud: String,
    exp: i64,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    name: Option<String>,
}

/// Margen de reloj. Un teléfono de feria con la hora corrida por segundos no
/// puede quedarse afuera del negocio; un minuto no le sirve a nadie para robar
/// nada, porque el token igual está firmado.
const MARGEN_RELOJ_SEGS: i64 = 60;

/// Verifica un `id_token` de Google.
///
/// `ahora` es epoch en segundos, inyectado para que los tests no dependan del
/// reloj de la máquina. `aud_esperado` es **nuestro** client id Web.
///
/// El orden importa: primero la firma, después los claims. Chequear claims de
/// un token no verificado es leer datos que puso el atacante.
pub fn verificar_id_token(
    id_token: &str,
    jwks: &GoogleJwks,
    aud_esperado: &str,
    ahora: i64,
) -> Result<IdentidadGoogleVerificada, ErrorVerificacionGoogle> {
    if jwks.vacio() {
        return Err(ErrorVerificacionGoogle::SinLlaves);
    }

    let header = jsonwebtoken::decode_header(id_token)
        .map_err(|_| ErrorVerificacionGoogle::MalFormado)?;

    // Antes de mirar el kid: si el algoritmo no es RS256 no hay nada que
    // discutir. `jsonwebtoken` también lo valida, pero rechazarlo acá deja el
    // error específico y evita depender de la config de `Validation` para una
    // garantía de seguridad.
    if header.alg != jsonwebtoken::Algorithm::RS256 {
        return Err(ErrorVerificacionGoogle::AlgoritmoNoSoportado {
            alg: format!("{:?}", header.alg),
        });
    }

    let kid = header.kid.ok_or(ErrorVerificacionGoogle::MalFormado)?;
    let jwk = jwks
        .buscar(&kid)
        .ok_or(ErrorVerificacionGoogle::LlaveDesconocida { kid: kid.clone() })?;

    let clave = jsonwebtoken::DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|_| ErrorVerificacionGoogle::FirmaInvalida)?;

    // `jsonwebtoken` valida **la firma y nada más**. Emisor, audiencia y
    // vencimiento los chequea este módulo: así cada falla tiene su variante
    // (el handler trata distinto "kid viejo" que "aud ajeno") y el `exp` se
    // mide contra el `ahora` inyectado en vez del reloj del proceso.
    let mut val = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    val.validate_exp = false;
    val.validate_aud = false;
    val.required_spec_claims.clear();

    let datos =
        jsonwebtoken::decode::<ClaimsGoogle>(id_token, &clave, &val).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                ErrorVerificacionGoogle::FirmaInvalida
            }
            _ => ErrorVerificacionGoogle::MalFormado,
        })?;
    let claims = datos.claims;

    if !GOOGLE_ISSUERS.contains(&claims.iss.as_str()) {
        return Err(ErrorVerificacionGoogle::EmisorInvalido { iss: claims.iss });
    }

    // El chequeo que la gente olvida. Sin esto, un `id_token` que Google emitió
    // para cualquier otra app —una que el atacante registró hace cinco minutos—
    // está perfectamente firmado por Google y entra a nuestro negocio.
    if claims.aud != aud_esperado {
        return Err(ErrorVerificacionGoogle::AudienciaInvalida);
    }

    if claims.exp + MARGEN_RELOJ_SEGS < ahora {
        return Err(ErrorVerificacionGoogle::Vencido);
    }

    let sub = claims
        .sub
        .filter(|s| !s.trim().is_empty())
        .ok_or(ErrorVerificacionGoogle::SinSujeto)?;

    Ok(IdentidadGoogleVerificada {
        sub,
        email: claims.email.map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty()),
        email_verificado: claims.email_verified.unwrap_or(false),
        nombre: claims.name,
        expira_en: claims.exp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::traits::PublicKeyParts;
    use serde_json::json;
    use std::sync::OnceLock;

    const AUD_NUESTRO: &str = "client-id-de-test.apps.googleusercontent.com";
    const AHORA: i64 = 1_800_000_000;

    /// Par de llaves de test, generado en memoria una sola vez por binario.
    ///
    /// **Se genera, no se commitea** (Regla 3): una clave privada en el repo es
    /// una clave privada en el repo, aunque diga "de test". Generar RSA-2048
    /// cuesta ~1 s y el `OnceLock` lo paga una vez para toda la suite.
    struct ParDeLlaves {
        pem: String,
        n: String,
        e: String,
    }

    fn llaves() -> &'static ParDeLlaves {
        static LLAVES: OnceLock<ParDeLlaves> = OnceLock::new();
        LLAVES.get_or_init(|| {
            let mut rng = rand::thread_rng();
            let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("generar rsa");
            let pub_key = priv_key.to_public_key();
            ParDeLlaves {
                pem: priv_key
                    .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
                    .expect("pem")
                    .to_string(),
                n: b64url(&pub_key.n().to_bytes_be()),
                e: b64url(&pub_key.e().to_bytes_be()),
            }
        })
    }

    fn b64url(bytes: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn jwks_bueno() -> GoogleJwks {
        GoogleJwks {
            keys: vec![GoogleJwk {
                kid: "llave-1".into(),
                kty: "RSA".into(),
                alg: "RS256".into(),
                n: llaves().n.clone(),
                e: llaves().e.clone(),
            }],
        }
    }

    /// Firma un token con la llave de test. `kid` se pasa aparte para poder
    /// simular rotación (token firmado con una llave que el JWKS no tiene).
    fn firmar(kid: &str, claims: serde_json::Value) -> String {
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(kid.to_string());
        let key = EncodingKey::from_rsa_pem(llaves().pem.as_bytes()).expect("encoding key");
        encode(&header, &claims, &key).expect("firmar")
    }

    fn claims_validos() -> serde_json::Value {
        json!({
            "sub": "104729...estable",
            "iss": "https://accounts.google.com",
            "aud": AUD_NUESTRO,
            "exp": AHORA + 3600,
            "iat": AHORA,
            "email": "Feriante@Gmail.com",
            "email_verified": true,
            "name": "Rosa del puesto",
        })
    }

    #[test]
    fn token_valido_entra_y_la_identidad_es_el_sub() {
        let t = firmar("llave-1", claims_validos());
        let id = verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA).expect("debe entrar");
        assert_eq!(id.sub, "104729...estable");
        assert!(id.email_verificado);
        // El email se normaliza, pero no es la identidad.
        assert_eq!(id.email.as_deref(), Some("feriante@gmail.com"));
    }

    #[test]
    fn las_dos_formas_de_iss_valen() {
        for iss in GOOGLE_ISSUERS {
            let mut c = claims_validos();
            c["iss"] = json!(iss);
            let t = firmar("llave-1", c);
            assert!(
                verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA).is_ok(),
                "iss {iss} debería valer",
            );
        }
    }

    /// El chequeo que la gente olvida: token perfectamente firmado por Google,
    /// pero emitido para otra app. No entra.
    #[test]
    fn token_de_otra_app_no_entra_aunque_google_lo_haya_firmado() {
        let mut c = claims_validos();
        c["aud"] = json!("otra-app.apps.googleusercontent.com");
        let t = firmar("llave-1", c);
        assert_eq!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::AudienciaInvalida),
        );
    }

    #[test]
    fn emisor_ajeno_no_entra() {
        let mut c = claims_validos();
        c["iss"] = json!("https://accounts.evil.example");
        let t = firmar("llave-1", c);
        assert!(matches!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::EmisorInvalido { .. }),
        ));
    }

    #[test]
    fn token_vencido_no_entra_pero_el_margen_de_reloj_perdona_segundos() {
        let mut c = claims_validos();
        c["exp"] = json!(AHORA - 10);
        let t = firmar("llave-1", c.clone());
        // Vencido hace 10 s: entra, el teléfono puede tener la hora corrida.
        assert!(verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA).is_ok());

        c["exp"] = json!(AHORA - 3600);
        let viejo = firmar("llave-1", c);
        assert_eq!(
            verificar_id_token(&viejo, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::Vencido),
        );
    }

    /// Rotación: Google firmó con una llave que nuestro caché no tiene todavía.
    /// El error tiene que ser distinguible para que el handler refresque y
    /// reintente en vez de rechazar a alguien que hizo todo bien.
    #[test]
    fn kid_desconocido_se_distingue_de_firma_invalida() {
        let t = firmar("llave-nueva-de-google", claims_validos());
        assert_eq!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::LlaveDesconocida {
                kid: "llave-nueva-de-google".into()
            }),
        );
    }

    #[test]
    fn firma_de_otra_llave_no_entra() {
        let t = firmar("llave-1", claims_validos());
        // Mismo kid, módulo de otra llave: la firma no cierra.
        let otra = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).expect("rsa");
        let pubk = otra.to_public_key();
        let jwks = GoogleJwks {
            keys: vec![GoogleJwk {
                kid: "llave-1".into(),
                kty: "RSA".into(),
                alg: "RS256".into(),
                n: b64url(&pubk.n().to_bytes_be()),
                e: b64url(&pubk.e().to_bytes_be()),
            }],
        };
        assert_eq!(
            verificar_id_token(&t, &jwks, AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::FirmaInvalida),
        );
    }

    /// `alg: none` es el ataque de manual: sacar la firma y decir que no hacía
    /// falta. Tiene que morir antes de mirar un solo claim.
    #[test]
    fn alg_none_no_entra() {
        // Header `{"alg":"none","kid":"llave-1"}` + claims válidos + firma vacía.
        let header = b64url(br#"{"alg":"none","kid":"llave-1"}"#);
        let payload = b64url(claims_validos().to_string().as_bytes());
        let t = format!("{header}.{payload}.");
        assert!(matches!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::MalFormado
                | ErrorVerificacionGoogle::AlgoritmoNoSoportado { .. }),
        ));
    }

    /// Confusión de algoritmo: firmar HS256 usando la clave *pública* RSA como
    /// secreto compartido. Si el verificador confía en el `alg` del header, el
    /// atacante firma sus propios tokens con datos que son públicos.
    #[test]
    fn hs256_firmado_con_la_clave_publica_no_entra() {
        let mut header = Header::new(jsonwebtoken::Algorithm::HS256);
        header.kid = Some("llave-1".into());
        let key = EncodingKey::from_secret(llaves().n.as_bytes());
        let t = encode(&header, &claims_validos(), &key).expect("firmar hs256");
        assert!(matches!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::AlgoritmoNoSoportado { .. }),
        ));
    }

    #[test]
    fn sin_sub_no_hay_a_quien_ligar() {
        let mut c = claims_validos();
        c["sub"] = json!("   ");
        let t = firmar("llave-1", c);
        assert_eq!(
            verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::SinSujeto),
        );
    }

    /// Sin llaves es problema nuestro (red/caché), no del token: el handler
    /// responde 503 y no acusa a la persona de traer un token malo.
    #[test]
    fn jwks_vacio_es_falla_nuestra() {
        let t = firmar("llave-1", claims_validos());
        assert_eq!(
            verificar_id_token(&t, &GoogleJwks::default(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::SinLlaves),
        );
    }

    #[test]
    fn basura_no_es_token() {
        assert_eq!(
            verificar_id_token("no-soy-un-jwt", &jwks_bueno(), AUD_NUESTRO, AHORA),
            Err(ErrorVerificacionGoogle::MalFormado),
        );
    }

    #[test]
    fn email_sin_verificar_llega_marcado() {
        let mut c = claims_validos();
        c["email_verified"] = json!(false);
        let t = firmar("llave-1", c);
        let id = verificar_id_token(&t, &jwks_bueno(), AUD_NUESTRO, AHORA).expect("entra");
        assert!(!id.email_verificado, "no se puede ligar por un email sin verificar");
    }
}
