use std::sync::Arc;

use user::UserId;

use crate::domain::{
    ChallengeKind, ChallengeRepository, Credential, CredentialId, CredentialRepository, Passkey,
};

use super::encoding::decode_base64url;
use super::error::AppError;
use super::ports::{Clock, WebAuthnVerifier};

#[derive(Debug, Clone)]
pub struct FinishPasskeyRegistrationCommand {
    pub user_id: UserId,
    pub credential_id: String,
    pub client_data_json: String,
    pub attestation_object: String,
    pub transports: Option<Vec<String>>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPasskey {
    pub id: String,
    pub label: Option<String>,
    pub transports: Option<String>,
    pub created_at: i64,
}

pub struct FinishPasskeyRegistrationHandler {
    credentials: Arc<dyn CredentialRepository>,
    challenges: Arc<dyn ChallengeRepository>,
    verifier: Arc<dyn WebAuthnVerifier>,
    clock: Arc<dyn Clock>,
}

impl FinishPasskeyRegistrationHandler {
    pub fn new(
        credentials: Arc<dyn CredentialRepository>,
        challenges: Arc<dyn ChallengeRepository>,
        verifier: Arc<dyn WebAuthnVerifier>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            credentials,
            challenges,
            verifier,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: FinishPasskeyRegistrationCommand,
    ) -> Result<RegisteredPasskey, AppError> {
        let client_data_bytes = decode_base64url(&command.client_data_json)
            .map_err(|_| AppError::Passkey("invalid client data encoding".to_string()))?;
        let attestation_bytes = decode_base64url(&command.attestation_object)
            .map_err(|_| AppError::Passkey("invalid attestation encoding".to_string()))?;

        let client_data = self
            .verifier
            .parse_client_data(&client_data_bytes)
            .map_err(|_| AppError::Passkey("invalid client data".to_string()))?;

        let now = self.clock.now();
        let record = self
            .challenges
            .consume(&client_data.challenge)
            .await?
            .ok_or_else(|| AppError::Passkey("unknown challenge".to_string()))?;
        if record.kind() != ChallengeKind::PasskeyRegister
            || record.user_id() != Some(&command.user_id)
            || record.is_expired(now)
        {
            return Err(AppError::Passkey("challenge mismatch".to_string()));
        }

        let registration = self
            .verifier
            .verify_registration(&client_data.challenge, &client_data_bytes, &attestation_bytes)
            .await
            .map_err(|err| AppError::Passkey(err.to_string()))?;
        if registration.credential_id != command.credential_id {
            return Err(AppError::Passkey(
                "credential id does not match attestation".to_string(),
            ));
        }

        let transports = command.transports.map(|values| values.join(","));
        let passkey = Passkey::new(
            registration.credential_id,
            registration.public_key,
            registration.sign_count,
            transports.clone(),
        );
        let credential = Credential::new_passkey(
            CredentialId::new(),
            command.user_id,
            passkey,
            command.label,
            now,
        );
        self.credentials.save(&credential).await?;

        Ok(RegisteredPasskey {
            id: credential.id().to_string(),
            label: credential.label().map(str::to_string),
            transports,
            created_at: credential.created_at(),
        })
    }
}
