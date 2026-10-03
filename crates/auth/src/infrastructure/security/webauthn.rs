use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ciborium::value::Value;
use p256::ecdsa::signature::Verifier;
use sha2::{Digest, Sha256};

use crate::application::{
    CollectedClientData, PasskeyRegistration, PortError, WebAuthnVerifier,
};
use crate::domain::Passkey;

const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_ATTESTED_CREDENTIAL_DATA: u8 = 0x40;

pub struct RustWebAuthnVerifier {
    rp_id: String,
    rp_name: String,
    allowed_origins: Vec<String>,
}

impl RustWebAuthnVerifier {
    pub fn new(
        rp_id: impl Into<String>,
        rp_name: impl Into<String>,
        allowed_origins: Vec<String>,
    ) -> Self {
        Self {
            rp_id: rp_id.into(),
            rp_name: rp_name.into(),
            allowed_origins,
        }
    }

    pub fn with_origins_csv(
        rp_id: impl Into<String>,
        rp_name: impl Into<String>,
        origins_csv: &str,
    ) -> Self {
        let origins = origins_csv
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect();
        Self::new(rp_id, rp_name, origins)
    }

    fn check_origin(&self, origin: &str) -> Result<(), PortError> {
        if self.allowed_origins.iter().any(|allowed| allowed == origin) {
            Ok(())
        } else {
            Err(PortError::Failure("origin not allowed".to_string()))
        }
    }
}

#[derive(serde::Deserialize)]
struct ClientDataJson {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
}

struct AuthenticatorData {
    rp_id_hash: [u8; 32],
    flags: u8,
    sign_count: u32,
    credential_id: Option<Vec<u8>>,
    public_key: Option<Vec<u8>>,
}

fn parse_authenticator_data(
    bytes: &[u8],
    require_attested_credential: bool,
) -> Result<AuthenticatorData, PortError> {
    let invalid = |message: &str| PortError::Failure(message.to_string());
    if bytes.len() < 37 {
        return Err(invalid("authenticator data too short"));
    }

    let mut rp_id_hash = [0u8; 32];
    rp_id_hash.copy_from_slice(&bytes[0..32]);
    let flags = bytes[32];
    let sign_count = u32::from_be_bytes([bytes[33], bytes[34], bytes[35], bytes[36]]);

    if flags & FLAG_USER_PRESENT == 0 {
        return Err(invalid("user presence flag not set"));
    }

    if !require_attested_credential {
        return Ok(AuthenticatorData {
            rp_id_hash,
            flags,
            sign_count,
            credential_id: None,
            public_key: None,
        });
    }

    if flags & FLAG_ATTESTED_CREDENTIAL_DATA == 0 {
        return Err(invalid("attested credential data flag not set"));
    }
    if bytes.len() < 55 {
        return Err(invalid("attested credential data truncated"));
    }

    let credential_length = u16::from_be_bytes([bytes[53], bytes[54]]) as usize;
    let credential_start = 55;
    let credential_end = credential_start + credential_length;
    if bytes.len() < credential_end + 1 {
        return Err(invalid("credential public key missing"));
    }

    Ok(AuthenticatorData {
        rp_id_hash,
        flags,
        sign_count,
        credential_id: Some(bytes[credential_start..credential_end].to_vec()),
        public_key: Some(bytes[credential_end..].to_vec()),
    })
}

fn parse_attestation_object(bytes: &[u8]) -> Result<(String, Vec<u8>), PortError> {
    let invalid = |message: &str| PortError::Failure(message.to_string());
    let value: Value =
        ciborium::de::from_reader(bytes).map_err(|_| invalid("invalid attestation object"))?;
    let map = value.as_map().ok_or_else(|| invalid("attestation is not a map"))?;

    let text = |key: &str| {
        map.iter()
            .find(|(candidate, _)| candidate.as_text() == Some(key))
            .map(|(_, value)| value)
    };

    let fmt = text("fmt")
        .and_then(Value::as_text)
        .ok_or_else(|| invalid("attestation fmt missing"))?
        .to_string();
    let auth_data = text("authData")
        .and_then(Value::as_bytes)
        .ok_or_else(|| invalid("attestation authData missing"))?
        .clone();

    Ok((fmt, auth_data))
}

fn cose_entry<'a>(map: &'a [(Value, Value)], key: i64) -> Option<&'a Value> {
    map.iter()
        .find(|(candidate, _)| candidate.as_integer() == Some(key.into()))
        .map(|(_, value)| value)
}

fn verify_signature(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), PortError> {
    let invalid = |message: &str| PortError::Failure(message.to_string());
    let value: Value =
        ciborium::de::from_reader(public_key).map_err(|_| invalid("invalid COSE key"))?;
    let map = value.as_map().ok_or_else(|| invalid("COSE key is not a map"))?;
    let algorithm = cose_entry(map, 3)
        .and_then(Value::as_integer)
        .map(i128::from)
        .ok_or_else(|| invalid("COSE algorithm missing"))?;

    match algorithm {
        -7 => {
            let x = cose_entry(map, -2)
                .and_then(Value::as_bytes)
                .ok_or_else(|| invalid("COSE x coordinate missing"))?;
            let y = cose_entry(map, -3)
                .and_then(Value::as_bytes)
                .ok_or_else(|| invalid("COSE y coordinate missing"))?;
            if x.len() != 32 || y.len() != 32 {
                return Err(invalid("invalid P-256 coordinate length"));
            }

            let mut sec1 = Vec::with_capacity(65);
            sec1.push(0x04);
            sec1.extend_from_slice(x);
            sec1.extend_from_slice(y);
            let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(&sec1)
                .map_err(|_| invalid("invalid P-256 public key"))?;
            let signature = p256::ecdsa::Signature::from_der(signature)
                .map_err(|_| invalid("invalid ECDSA signature"))?;
            key.verify(message, &signature)
                .map_err(|_| invalid("invalid signature"))
        }
        -8 => {
            let x = cose_entry(map, -2)
                .and_then(Value::as_bytes)
                .ok_or_else(|| invalid("COSE x coordinate missing"))?;
            let key_bytes: [u8; 32] = x
                .as_slice()
                .try_into()
                .map_err(|_| invalid("invalid Ed25519 key length"))?;
            let key = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| invalid("invalid Ed25519 public key"))?;
            let signature = ed25519_dalek::Signature::from_slice(signature)
                .map_err(|_| invalid("invalid Ed25519 signature"))?;
            key.verify_strict(message, &signature)
                .map_err(|_| invalid("invalid signature"))
        }
        _ => Err(invalid("unsupported COSE algorithm")),
    }
}

#[async_trait]
impl WebAuthnVerifier for RustWebAuthnVerifier {
    fn relying_party_id(&self) -> String {
        self.rp_id.clone()
    }

    fn relying_party_name(&self) -> String {
        self.rp_name.clone()
    }

    fn parse_client_data(&self, client_data_json: &[u8]) -> Result<CollectedClientData, PortError> {
        let parsed: ClientDataJson = serde_json::from_slice(client_data_json)
            .map_err(|_| PortError::Failure("invalid client data json".to_string()))?;
        Ok(CollectedClientData {
            kind: parsed.kind,
            challenge: parsed.challenge,
            origin: parsed.origin,
        })
    }

    async fn verify_registration(
        &self,
        expected_challenge: &str,
        client_data_json: &[u8],
        attestation_object: &[u8],
    ) -> Result<PasskeyRegistration, PortError> {
        let client_data = self.parse_client_data(client_data_json)?;
        if client_data.kind != "webauthn.create" {
            return Err(PortError::Failure(
                "unexpected client data type".to_string(),
            ));
        }
        if client_data.challenge != expected_challenge {
            return Err(PortError::Failure("challenge mismatch".to_string()));
        }
        self.check_origin(&client_data.origin)?;

        let (fmt, auth_data) = parse_attestation_object(attestation_object)?;
        if fmt != "none" {
            return Err(PortError::Failure(
                "unsupported attestation format".to_string(),
            ));
        }

        let parsed = parse_authenticator_data(&auth_data, true)?;
        if parsed.rp_id_hash != Sha256::digest(self.rp_id.as_bytes()).as_slice() {
            return Err(PortError::Failure("rp id hash mismatch".to_string()));
        }

        let credential_id = parsed
            .credential_id
            .ok_or_else(|| PortError::Failure("credential id missing".to_string()))?;
        let public_key = parsed
            .public_key
            .ok_or_else(|| PortError::Failure("public key missing".to_string()))?;

        Ok(PasskeyRegistration {
            credential_id: URL_SAFE_NO_PAD.encode(&credential_id),
            public_key: URL_SAFE_NO_PAD.encode(&public_key),
            sign_count: parsed.sign_count,
        })
    }

    async fn verify_assertion(
        &self,
        passkey: &Passkey,
        expected_challenge: &str,
        client_data_json: &[u8],
        authenticator_data: &[u8],
        signature: &[u8],
        require_user_verification: bool,
    ) -> Result<u32, PortError> {
        let client_data = self.parse_client_data(client_data_json)?;
        if client_data.kind != "webauthn.get" {
            return Err(PortError::Failure(
                "unexpected client data type".to_string(),
            ));
        }
        if client_data.challenge != expected_challenge {
            return Err(PortError::Failure("challenge mismatch".to_string()));
        }
        self.check_origin(&client_data.origin)?;

        let parsed = parse_authenticator_data(authenticator_data, false)?;
        if parsed.rp_id_hash != Sha256::digest(self.rp_id.as_bytes()).as_slice() {
            return Err(PortError::Failure("rp id hash mismatch".to_string()));
        }
        if require_user_verification && parsed.flags & FLAG_USER_VERIFIED == 0 {
            return Err(PortError::Failure(
                "user verification required".to_string(),
            ));
        }

        let public_key = URL_SAFE_NO_PAD
            .decode(passkey.public_key())
            .map_err(|_| PortError::Failure("stored public key is invalid".to_string()))?;
        let mut signed_data = authenticator_data.to_vec();
        signed_data.extend_from_slice(&Sha256::digest(client_data_json));
        verify_signature(&public_key, &signed_data, signature)?;

        Ok(parsed.sign_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    const RP_ID: &str = "example.com";
    const ORIGIN: &str = "https://example.com";
    const CHALLENGE: &str = "test-challenge";

    fn verifier() -> RustWebAuthnVerifier {
        RustWebAuthnVerifier::new(RP_ID, "Test", vec![ORIGIN.to_string()])
    }

    fn ec2_cose_key(key: &p256::ecdsa::VerifyingKey) -> Vec<u8> {
        let point = key.to_encoded_point(false);
        let map = Value::Map(vec![
            (Value::Integer(1i64.into()), Value::Integer(2i64.into())),
            (Value::Integer(3i64.into()), Value::Integer((-7i64).into())),
            (Value::Integer((-1i64).into()), Value::Integer(1i64.into())),
            (
                Value::Integer((-2i64).into()),
                Value::Bytes(point.x().unwrap().to_vec()),
            ),
            (
                Value::Integer((-3i64).into()),
                Value::Bytes(point.y().unwrap().to_vec()),
            ),
        ]);
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&map, &mut encoded).unwrap();
        encoded
    }

    fn okp_cose_key(key: &ed25519_dalek::VerifyingKey) -> Vec<u8> {
        let map = Value::Map(vec![
            (Value::Integer(1i64.into()), Value::Integer(1i64.into())),
            (Value::Integer(3i64.into()), Value::Integer((-8i64).into())),
            (Value::Integer((-1i64).into()), Value::Integer(6i64.into())),
            (
                Value::Integer((-2i64).into()),
                Value::Bytes(key.to_bytes().to_vec()),
            ),
        ]);
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&map, &mut encoded).unwrap();
        encoded
    }

    fn registration_auth_data(credential_id: &[u8], cose_key: &[u8], flags: u8) -> Vec<u8> {
        let mut auth_data = Vec::new();
        auth_data.extend_from_slice(&Sha256::digest(RP_ID.as_bytes()));
        auth_data.push(flags);
        auth_data.extend_from_slice(&0u32.to_be_bytes());
        auth_data.extend_from_slice(&[0u8; 16]);
        auth_data.extend_from_slice(&(credential_id.len() as u16).to_be_bytes());
        auth_data.extend_from_slice(credential_id);
        auth_data.extend_from_slice(cose_key);
        auth_data
    }

    fn assertion_auth_data(sign_count: u32, flags: u8) -> Vec<u8> {
        let mut auth_data = Vec::new();
        auth_data.extend_from_slice(&Sha256::digest(RP_ID.as_bytes()));
        auth_data.push(flags);
        auth_data.extend_from_slice(&sign_count.to_be_bytes());
        auth_data
    }

    fn attestation_object(auth_data: &[u8]) -> Vec<u8> {
        let map = Value::Map(vec![
            (Value::Text("fmt".to_string()), Value::Text("none".to_string())),
            (Value::Text("attStmt".to_string()), Value::Map(vec![])),
            (
                Value::Text("authData".to_string()),
                Value::Bytes(auth_data.to_vec()),
            ),
        ]);
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&map, &mut encoded).unwrap();
        encoded
    }

    fn client_data(kind: &str, challenge: &str, origin: &str) -> Vec<u8> {
        format!(
            r#"{{"type":"{kind}","challenge":"{challenge}","origin":"{origin}"}}"#
        )
        .into_bytes()
    }

    #[test]
    fn registration_accepts_none_attestation_with_es256_key() {
        let signing_key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let cose = ec2_cose_key(&signing_key.verifying_key());
        let credential_id = vec![1u8, 2, 3, 4];
        let auth_data = registration_auth_data(&credential_id, &cose, 0x41);
        let attestation = attestation_object(&auth_data);
        let client = client_data("webauthn.create", CHALLENGE, ORIGIN);

        let registration = block_on(verifier().verify_registration(
            CHALLENGE,
            &client,
            &attestation,
        ))
        .unwrap();

        assert_eq!(
            registration.credential_id,
            URL_SAFE_NO_PAD.encode(&credential_id)
        );
        assert_eq!(registration.sign_count, 0);
        assert_eq!(
            URL_SAFE_NO_PAD.decode(&registration.public_key).unwrap(),
            cose
        );
    }

    #[test]
    fn registration_rejects_wrong_origin_and_challenge() {
        let signing_key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let cose = ec2_cose_key(&signing_key.verifying_key());
        let auth_data = registration_auth_data(&[1, 2, 3], &cose, 0x41);
        let attestation = attestation_object(&auth_data);

        let bad_origin = client_data("webauthn.create", CHALLENGE, "https://evil.example");
        assert!(block_on(verifier().verify_registration(CHALLENGE, &bad_origin, &attestation)).is_err());

        let bad_challenge = client_data("webauthn.create", "other", ORIGIN);
        assert!(
            block_on(verifier().verify_registration(CHALLENGE, &bad_challenge, &attestation))
                .is_err()
        );
    }

    #[test]
    fn assertion_accepts_valid_es256_signature() {
        use p256::ecdsa::signature::Signer;

        let signing_key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let cose = ec2_cose_key(&signing_key.verifying_key());
        let passkey = Passkey::new(
            URL_SAFE_NO_PAD.encode([1, 2, 3]),
            URL_SAFE_NO_PAD.encode(&cose),
            0,
            None,
        );

        let auth_data = assertion_auth_data(1, 0x05);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let mut signed_data = auth_data.clone();
        signed_data.extend_from_slice(&Sha256::digest(&client));
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_data);
        let signature = signature.to_der();

        let sign_count = block_on(verifier().verify_assertion(
            &passkey,
            CHALLENGE,
            &client,
            &auth_data,
            signature.as_bytes(),
            true,
        ))
        .unwrap();
        assert_eq!(sign_count, 1);
    }

    #[test]
    fn assertion_accepts_valid_ed25519_signature() {
        use ed25519_dalek::Signer;

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let cose = okp_cose_key(&signing_key.verifying_key());
        let passkey = Passkey::new(
            URL_SAFE_NO_PAD.encode([4, 5, 6]),
            URL_SAFE_NO_PAD.encode(&cose),
            1,
            None,
        );

        let auth_data = assertion_auth_data(2, 0x05);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let mut signed_data = auth_data.clone();
        signed_data.extend_from_slice(&Sha256::digest(&client));
        let signature = signing_key.sign(&signed_data);

        let sign_count = block_on(verifier().verify_assertion(
            &passkey,
            CHALLENGE,
            &client,
            &auth_data,
            &signature.to_bytes(),
            true,
        ))
        .unwrap();
        assert_eq!(sign_count, 2);
    }

    #[test]
    fn assertion_rejects_tampered_signature() {
        use p256::ecdsa::signature::Signer;

        let signing_key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let cose = ec2_cose_key(&signing_key.verifying_key());
        let passkey = Passkey::new(
            URL_SAFE_NO_PAD.encode([1, 2, 3]),
            URL_SAFE_NO_PAD.encode(&cose),
            0,
            None,
        );

        let auth_data = assertion_auth_data(1, 0x05);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let mut signed_data = auth_data.clone();
        signed_data.extend_from_slice(&Sha256::digest(&client));
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_data);
        let signature = signature.to_der();

        let tampered = client_data("webauthn.get", "tampered", ORIGIN);
        assert!(block_on(verifier().verify_assertion(
            &passkey,
            CHALLENGE,
            &tampered,
            &auth_data,
            signature.as_bytes(),
            true,
        ))
        .is_err());
    }

    #[test]
    fn assertion_requires_user_verification_when_requested() {
        use p256::ecdsa::signature::Signer;

        let signing_key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).unwrap();
        let cose = ec2_cose_key(&signing_key.verifying_key());
        let passkey = Passkey::new(
            URL_SAFE_NO_PAD.encode([1, 2, 3]),
            URL_SAFE_NO_PAD.encode(&cose),
            0,
            None,
        );

        let auth_data = assertion_auth_data(1, 0x01);
        let client = client_data("webauthn.get", CHALLENGE, ORIGIN);
        let mut signed_data = auth_data.clone();
        signed_data.extend_from_slice(&Sha256::digest(&client));
        let signature: p256::ecdsa::Signature = signing_key.sign(&signed_data);
        let signature = signature.to_der();

        assert!(block_on(verifier().verify_assertion(
            &passkey,
            CHALLENGE,
            &client,
            &auth_data,
            signature.as_bytes(),
            true,
        ))
        .is_err());
    }
}
