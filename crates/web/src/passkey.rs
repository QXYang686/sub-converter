use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use contract::passkey::{
    PasskeyLoginFinishRequest, PublicKeyCredentialCreationOptions,
    PublicKeyCredentialRequestOptions, RegisterPasskeyFinishRequest,
};

#[wasm_bindgen(inline_js = r#"
function b64urlToBuf(value) {
  const pad = value.length % 4 === 0 ? "" : "=".repeat(4 - (value.length % 4));
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/") + pad;
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function bufToB64url(buffer) {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export async function createPasskeyCredential(optionsJson) {
  const options = JSON.parse(optionsJson);
  options.challenge = b64urlToBuf(options.challenge);
  options.user.id = b64urlToBuf(options.user.id);
  options.excludeCredentials = (options.excludeCredentials || []).map((credential) => ({
    ...credential,
    id: b64urlToBuf(credential.id),
  }));
  const credential = await navigator.credentials.create({ publicKey: options });
  if (!credential) throw new Error("passkey creation was cancelled");
  const response = credential.response;
  return JSON.stringify({
    credentialId: bufToB64url(credential.rawId),
    clientDataJSON: bufToB64url(response.clientDataJSON),
    attestationObject: bufToB64url(response.attestationObject),
    transports: typeof response.getTransports === "function" ? response.getTransports() : null,
  });
}

export async function getPasskeyAssertion(optionsJson) {
  const options = JSON.parse(optionsJson);
  options.challenge = b64urlToBuf(options.challenge);
  options.allowCredentials = (options.allowCredentials || []).map((credential) => ({
    ...credential,
    id: b64urlToBuf(credential.id),
  }));
  const credential = await navigator.credentials.get({ publicKey: options });
  if (!credential) throw new Error("passkey login was cancelled");
  const response = credential.response;
  return JSON.stringify({
    credentialId: bufToB64url(credential.rawId),
    clientDataJSON: bufToB64url(response.clientDataJSON),
    authenticatorData: bufToB64url(response.authenticatorData),
    signature: bufToB64url(response.signature),
  });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = createPasskeyCredential)]
    fn create_passkey_credential(options_json: &str) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch, js_name = getPasskeyAssertion)]
    fn get_passkey_assertion(options_json: &str) -> Result<js_sys::Promise, JsValue>;
}

pub async fn create_credential(
    options: &PublicKeyCredentialCreationOptions,
) -> Result<RegisterPasskeyFinishRequest, String> {
    let options_json = serde_json::to_string(options).map_err(|err| err.to_string())?;
    let promise = create_passkey_credential(&options_json).map_err(js_error)?;
    let value = JsFuture::from(promise).await.map_err(js_error)?;
    let text = value
        .as_string()
        .ok_or_else(|| "unexpected passkey result".to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub async fn get_assertion(
    options: &PublicKeyCredentialRequestOptions,
) -> Result<PasskeyLoginFinishRequest, String> {
    let options_json = serde_json::to_string(options).map_err(|err| err.to_string())?;
    let promise = get_passkey_assertion(&options_json).map_err(js_error)?;
    let value = JsFuture::from(promise).await.map_err(js_error)?;
    let text = value
        .as_string()
        .ok_or_else(|| "unexpected passkey result".to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|error| String::from(error.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}
