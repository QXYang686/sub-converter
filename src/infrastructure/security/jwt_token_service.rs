use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::application::config::{ACCESS_TOKEN_TTL_SECONDS, REFRESH_TOKEN_TTL_SECONDS};
use crate::application::{
    AccessToken, IssuedRefreshToken, PortError, StoredRefreshToken, TokenService,
};
use crate::domain::user::UserId;

use super::random::random_bytes;

type HmacSha256 = Hmac<Sha256>;

const REFRESH_TOKEN_LENGTH: usize = 32;
const JWT_HEADER: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iat: i64,
    exp: i64,
    jti: String,
}

pub struct JwtTokenService {
    secret: Vec<u8>,
}

impl JwtTokenService {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into().into_bytes(),
        }
    }

    fn sign(&self, signing_input: &str) -> Result<String, PortError> {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .map_err(|err| PortError::Failure(err.to_string()))?;
        mac.update(signing_input.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    fn split_token(token: &str) -> Result<(&str, &str, &str), PortError> {
        let mut parts = token.split('.');
        let header = parts.next().ok_or(PortError::InvalidToken)?;
        let payload = parts.next().ok_or(PortError::InvalidToken)?;
        let signature = parts.next().ok_or(PortError::InvalidToken)?;
        if parts.next().is_some() {
            return Err(PortError::InvalidToken);
        }
        Ok((header, payload, signature))
    }
}

#[async_trait]
impl TokenService for JwtTokenService {
    async fn issue_access_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<AccessToken, PortError> {
        let claims = Claims {
            sub: user_id.to_string(),
            iat: now,
            exp: now + ACCESS_TOKEN_TTL_SECONDS,
            jti: Uuid::new_v4().to_string(),
        };
        let payload = serde_json::to_vec(&claims)
            .map_err(|err| PortError::Failure(err.to_string()))?;
        let signing_input = format!("{JWT_HEADER}.{}", URL_SAFE_NO_PAD.encode(payload));
        let signature = self.sign(&signing_input)?;
        Ok(AccessToken {
            token: format!("{signing_input}.{signature}"),
            expires_at: claims.exp,
        })
    }

    async fn verify_access_token(&self, token: &str, now: i64) -> Result<UserId, PortError> {
        let (header, payload, signature) = Self::split_token(token)?;
        if header != JWT_HEADER {
            return Err(PortError::InvalidToken);
        }

        let expected = self.sign(&format!("{header}.{payload}"))?;
        if !bool::from(expected.as_bytes().ct_eq(signature.as_bytes())) {
            return Err(PortError::InvalidToken);
        }

        let decoded = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| PortError::InvalidToken)?;
        let claims: Claims =
            serde_json::from_slice(&decoded).map_err(|_| PortError::InvalidToken)?;

        if claims.exp <= now {
            return Err(PortError::InvalidToken);
        }

        UserId::parse(&claims.sub).map_err(|_| PortError::InvalidToken)
    }

    async fn issue_refresh_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<IssuedRefreshToken, PortError> {
        let bytes = random_bytes(REFRESH_TOKEN_LENGTH).map_err(PortError::Failure)?;
        let raw = URL_SAFE_NO_PAD.encode(&bytes);
        Ok(IssuedRefreshToken {
            record: StoredRefreshToken {
                id: Uuid::new_v4(),
                user_id: user_id.clone(),
                token_hash: self.hash_refresh_token(&raw),
                created_at: now,
                expires_at: now + REFRESH_TOKEN_TTL_SECONDS,
                revoked_at: None,
            },
            raw,
        })
    }

    fn hash_refresh_token(&self, raw: &str) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(raw.as_bytes()))
    }
}
