use pharma_core::config::JwtConfig;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue(cfg: &JwtConfig, sub: &str, tenant_id: &str, roles: Vec<String>) -> anyhow::Result<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: sub.to_string(),
        tenant_id: tenant_id.to_string(),
        roles,
        iss: cfg.issuer.clone(),
        iat: now,
        exp: now + cfg.ttl_seconds as i64,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn verify(cfg: &JwtConfig, token: &str) -> anyhow::Result<Claims> {
    let mut v = Validation::default();
    v.set_issuer(&[cfg.issuer.clone()]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.secret.as_bytes()),
        &v,
    )?;
    Ok(data.claims)
}

pub mod password {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
        Argon2,
    };

    pub fn hash(pw: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(pw.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("argon2: {e}"))?
            .to_string();
        Ok(hash)
    }

    pub fn verify(pw: &str, hash: &str) -> anyhow::Result<bool> {
        let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
        Ok(Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok())
    }
}
