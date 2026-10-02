use std::sync::Arc;

use user::UserId;

use crate::domain::CredentialRepository;

use super::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyView {
    pub id: String,
    pub label: Option<String>,
    pub transports: Option<String>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

pub struct ListPasskeysHandler {
    credentials: Arc<dyn CredentialRepository>,
}

impl ListPasskeysHandler {
    pub fn new(credentials: Arc<dyn CredentialRepository>) -> Self {
        Self { credentials }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, user_id: &UserId) -> Result<Vec<PasskeyView>, AppError> {
        let passkeys = self.credentials.find_passkeys_by_user_id(user_id).await?;
        Ok(passkeys
            .into_iter()
            .map(|credential| PasskeyView {
                id: credential.id().to_string(),
                label: credential.label().map(str::to_string),
                transports: credential
                    .passkey()
                    .and_then(|passkey| passkey.transports().map(str::to_string)),
                created_at: credential.created_at(),
                last_used_at: credential.last_used_at(),
            })
            .collect())
    }
}
