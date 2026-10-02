use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use wasm_bindgen::prelude::*;

use crate::application::{PortError, SecretGenerator};

const SECRET_BYTES: usize = 32;

#[wasm_bindgen(inline_js = r#"
export function randomBytes(length) {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return bytes;
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn randomBytes(length: usize) -> Result<js_sys::Uint8Array, JsValue>;
}

pub struct OsSecretGenerator;

impl SecretGenerator for OsSecretGenerator {
    fn generate(&self) -> Result<String, PortError> {
        let bytes =
            randomBytes(SECRET_BYTES).map_err(|err| PortError::Failure(format!("{err:?}")))?;
        Ok(URL_SAFE_NO_PAD.encode(bytes.to_vec()))
    }
}
