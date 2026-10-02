use std::sync::Arc;

use user::UserRepository;

use crate::domain::{ChallengeKind, ChallengeRepository, CredentialRepository};

use super::dto::UserView;
use super::encoding::decode_base64url;
use super::error::AppError;
use super::login::LoginResult;
use super::ports::{Clock, RefreshTokenRepository, TokenService, WebAuthnVerifier};

#[derive(Debug, Clone)]
pub struct FinishPasskeyLoginCommand {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
}

pub struct FinishPasskeyLoginHandler {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    challenges: Arc<dyn ChallengeRepository>,
    token_service: Arc<dyn TokenService>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    verifier: Arc<dyn WebAuthnVerifier>,
    clock: Arc<dyn Clock>,
}

impl FinishPasskeyLoginHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        challenges: Arc<dyn ChallengeRepository>,
        token_service: Arc<dyn TokenService>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        verifier: Arc<dyn WebAuthnVerifier>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            credentials,
            challenges,
            token_service,
            refresh_tokens,
            verifier,
            clock,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: FinishPasskeyLoginCommand,
    ) -> Result<LoginResult, AppError> {
        let credential = self
            .credentials
            .find_passkey_by_credential_id(&command.credential_id)
            .await?
            .ok_or(AppError::InvalidCredentials)?;
        let passkey = credential.passkey().ok_or(AppError::InvalidCredentials)?;

        let client_data_bytes = decode_base64url(&command.client_data_json)
            .map_err(|_| AppError::Passkey("invalid client data encoding".to_string()))?;
        let authenticator_bytes = decode_base64url(&command.authenticator_data)
            .map_err(|_| AppError::Passkey("invalid authenticator data encoding".to_string()))?;
        let signature_bytes = decode_base64url(&command.signature)
            .map_err(|_| AppError::Passkey("invalid signature encoding".to_string()))?;

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
        if record.kind() != ChallengeKind::PasskeyLogin || record.is_expired(now) {
            return Err(AppError::Passkey("challenge mismatch".to_string()));
        }
        if let Some(bound_user) = record.user_id() {
            if bound_user != credential.user_id() {
                return Err(AppError::InvalidCredentials);
            }
        }
        let require_user_verification = record.user_id().is_none();

        let new_sign_count = self
            .verifier
            .verify_assertion(
                passkey,
                &client_data.challenge,
                &client_data_bytes,
                &authenticator_bytes,
                &signature_bytes,
                require_user_verification,
            )
            .await
            .map_err(|err| AppError::Passkey(err.to_string()))?;
        if passkey.sign_count() > 0 && new_sign_count > 0 && new_sign_count <= passkey.sign_count()
        {
            return Err(AppError::InvalidToken);
        }

        self.credentials
            .update_sign_count(credential.id(), new_sign_count)
            .await?;
        let _ = self.credentials.mark_used(credential.id(), now).await;

        let user = self
            .users
            .find_by_id(credential.user_id())
            .await?
            .ok_or(AppError::InvalidCredentials)?;

        let access = self.token_service.issue_access_token(user.id(), now).await?;
        let refresh = self.token_service.issue_refresh_token(user.id(), now).await?;
        self.refresh_tokens.save(&refresh.record).await?;

        Ok(LoginResult {
            user: UserView::from(&user),
            access_token: access.token,
            access_token_expires_at: access.expires_at,
            refresh_token: refresh.raw,
            refresh_token_expires_at: refresh.record.expires_at,
        })
    }
}
