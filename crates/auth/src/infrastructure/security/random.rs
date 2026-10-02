use wasm_bindgen::prelude::*;

use crate::application::{PortError, RandomSource};

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

pub fn random_bytes(length: usize) -> Result<Vec<u8>, String> {
    randomBytes(length)
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("{err:?}"))
}

pub struct OsRandomSource;

impl RandomSource for OsRandomSource {
    fn random_bytes(&self, length: usize) -> Result<Vec<u8>, PortError> {
        random_bytes(length).map_err(PortError::Failure)
    }
}
