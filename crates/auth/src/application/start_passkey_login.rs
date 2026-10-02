use std::sync::Arc;

use user::{UserRepository, Username};

use crate::domain::{Challenge, ChallengeKind, ChallengeRepository, CredentialRepository, CHALLENGE_TTL_SECONDS};

use super::encoding::encode_base64url;
use super::error::AppError;
use super::ports::{Clock, RandomSource, WebAuthnVerifier};

const CHALLENGE_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct StartPasskeyLoginCommand {
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyLoginOptions {
    pub challenge: String,
    pub rp_id: String,
    pub allow_credential_ids: Vec<String>,
    pub user_verification: bool,
}

pub struct StartPasskeyLoginHandler {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    challenges: Arc<dyn ChallengeRepository>,
    random: Arc<dyn RandomSource>,
    verifier: Arc<dyn WebAuthnVerifier>,
    clock: Arc<dyn Clock>,
}

impl StartPasskeyLoginHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        challenges: Arc<dyn ChallengeRepository>,
        random: Arc<dyn RandomSource>,
        verifier: Arc<dyn WebAuthnVerifier>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            credentials,
            challenges,
            random,
            verifier,
            clock,
        }
    }

    pub async fn handle(
        &self,
        command: StartPasskeyLoginCommand,
    ) -> Result<PasskeyLoginOptions, AppError> {
        let bound_user = match command.username.as_deref().map(Username::new) {
            Some(Ok(username)) => self.users.find_by_username(&username).await?,
            _ => None,
        };

        let (user_id, allow_credential_ids) = match bound_user {
            Some(user) => {
                let passkeys = self.credentials.find_passkeys_by_user_id(user.id()).await?;
                (
                    Some(user.id().clone()),
                    passkeys
                        .iter()
                        .filter_map(|credential| {
                            credential
                                .passkey()
                                .map(|passkey| passkey.credential_id().to_string())
                        })
                        .collect(),
                )
            }
            None => (None, Vec::new()),
        };

        let challenge = encode_base64url(
            self.random
                .random_bytes(CHALLENGE_BYTES)
                .map_err(|err| AppError::Internal(err.to_string()))?,
        );
        let require_user_verification = user_id.is_none();
        let record = Challenge::new(
            challenge.clone(),
            user_id,
            ChallengeKind::PasskeyLogin,
            self.clock.now(),
            CHALLENGE_TTL_SECONDS,
        );
        self.challenges.save(&record).await?;

        Ok(PasskeyLoginOptions {
            challenge,
            rp_id: self.verifier.relying_party_id(),
            allow_credential_ids,
            user_verification: require_user_verification,
        })
    }
}
