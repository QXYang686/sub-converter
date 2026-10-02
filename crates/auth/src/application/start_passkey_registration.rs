use std::sync::Arc;

use user::UserId;

use crate::domain::{
    Challenge, ChallengeKind, ChallengeRepository, CredentialRepository, CHALLENGE_TTL_SECONDS,
};

use super::encoding::encode_base64url;
use super::error::AppError;
use super::ports::{Clock, RandomSource, WebAuthnVerifier};

const CHALLENGE_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct StartPasskeyRegistrationCommand {
    pub user_id: UserId,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyRegistrationOptions {
    pub challenge: String,
    pub rp_id: String,
    pub rp_name: String,
    pub user_handle: String,
    pub username: String,
    pub exclude_credential_ids: Vec<String>,
}

pub struct StartPasskeyRegistrationHandler {
    credentials: Arc<dyn CredentialRepository>,
    challenges: Arc<dyn ChallengeRepository>,
    random: Arc<dyn RandomSource>,
    verifier: Arc<dyn WebAuthnVerifier>,
    clock: Arc<dyn Clock>,
}

impl StartPasskeyRegistrationHandler {
    pub fn new(
        credentials: Arc<dyn CredentialRepository>,
        challenges: Arc<dyn ChallengeRepository>,
        random: Arc<dyn RandomSource>,
        verifier: Arc<dyn WebAuthnVerifier>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            credentials,
            challenges,
            random,
            verifier,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: StartPasskeyRegistrationCommand,
    ) -> Result<PasskeyRegistrationOptions, AppError> {
        let existing = self
            .credentials
            .find_passkeys_by_user_id(&command.user_id)
            .await?;

        let challenge = encode_base64url(
            self.random
                .random_bytes(CHALLENGE_BYTES)
                .map_err(|err| AppError::Internal(err.to_string()))?,
        );
        let now = self.clock.now();
        let record = Challenge::new(
            challenge.clone(),
            Some(command.user_id.clone()),
            ChallengeKind::PasskeyRegister,
            now,
            CHALLENGE_TTL_SECONDS,
        );
        self.challenges.save(&record).await?;

        Ok(PasskeyRegistrationOptions {
            challenge,
            rp_id: self.verifier.relying_party_id(),
            rp_name: self.verifier.relying_party_name(),
            user_handle: encode_base64url(command.user_id.as_uuid().as_bytes()),
            username: command.username,
            exclude_credential_ids: existing
                .iter()
                .filter_map(|credential| {
                    credential
                        .passkey()
                        .map(|passkey| passkey.credential_id().to_string())
                })
                .collect(),
        })
    }
}
