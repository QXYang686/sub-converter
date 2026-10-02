use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use js_sys::Uint8Array;
use subtle::ConstantTimeEq;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use worker::send::SendFuture;

use crate::application::{PasswordHasher, PortError};
use crate::domain::PasswordHash;

use super::random::random_bytes;

pub const PBKDF2_ITERATIONS: u32 = 600_000;
const PBKDF2_ALGORITHM: &str = "pbkdf2-sha256";
const SALT_LENGTH: usize = 16;
const KEY_LENGTH: usize = 32;

#[wasm_bindgen(inline_js = r#"
export function derivePbkdf2Sha256(password, salt, iterations, length) {
  const encoder = new TextEncoder();
  return crypto.subtle
    .importKey("raw", encoder.encode(password), "PBKDF2", false, ["deriveBits"])
    .then((key) =>
      crypto.subtle.deriveBits(
        { name: "PBKDF2", hash: "SHA-256", salt, iterations },
        key,
        length * 8,
      ),
    )
    .then((bits) => new Uint8Array(bits));
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn derivePbkdf2Sha256(
        password: &str,
        salt: &[u8],
        iterations: u32,
        length: usize,
    ) -> Result<js_sys::Promise, JsValue>;
}

pub struct Pbkdf2PasswordHasher;

impl Default for Pbkdf2PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Pbkdf2PasswordHasher {
    pub fn new() -> Self {
        Self
    }

    async fn derive(
        password: String,
        salt: Vec<u8>,
        iterations: u32,
    ) -> Result<Vec<u8>, PortError> {
        SendFuture::new(async move {
            let promise = derivePbkdf2Sha256(&password, &salt, iterations, KEY_LENGTH)
                .map_err(|err| PortError::Failure(format!("{err:?}")))?;
            let value = JsFuture::from(promise)
                .await
                .map_err(|err| PortError::Failure(format!("{err:?}")))?;
            let bytes = value
                .dyn_into::<Uint8Array>()
                .map_err(|err| PortError::Failure(format!("{err:?}")))?;
            Ok(bytes.to_vec())
        })
        .await
    }

    fn encode(iterations: u32, salt: &[u8], derived: &[u8]) -> String {
        format!(
            "${PBKDF2_ALGORITHM}$i={iterations}${}${}",
            STANDARD_NO_PAD.encode(salt),
            STANDARD_NO_PAD.encode(derived)
        )
    }

    fn decode(hash: &str) -> Result<(u32, Vec<u8>, Vec<u8>), PortError> {
        let invalid = || PortError::Failure("malformed password hash".to_string());

        let mut parts = hash.split('$');
        if parts.next() != Some("") || parts.next() != Some(PBKDF2_ALGORITHM) {
            return Err(invalid());
        }

        let iterations = parts
            .next()
            .and_then(|params| params.strip_prefix("i="))
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(invalid)?;
        let salt = STANDARD_NO_PAD
            .decode(parts.next().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let derived = STANDARD_NO_PAD
            .decode(parts.next().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        if parts.next().is_some() {
            return Err(invalid());
        }

        Ok((iterations, salt, derived))
    }
}

#[async_trait]
impl PasswordHasher for Pbkdf2PasswordHasher {
    async fn hash(&self, password: &str) -> Result<PasswordHash, PortError> {
        let salt = random_bytes(SALT_LENGTH).map_err(PortError::Failure)?;
        let derived = Self::derive(password.to_string(), salt.clone(), PBKDF2_ITERATIONS).await?;
        PasswordHash::new(Self::encode(PBKDF2_ITERATIONS, &salt, &derived))
            .map_err(|err| PortError::Failure(err.to_string()))
    }

    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, PortError> {
        let (iterations, salt, expected) = Self::decode(hash.as_str())?;
        let derived = Self::derive(password.to_string(), salt, iterations).await?;
        if derived.len() != expected.len() {
            return Ok(false);
        }
        Ok(derived.ct_eq(&expected).into())
    }
}
