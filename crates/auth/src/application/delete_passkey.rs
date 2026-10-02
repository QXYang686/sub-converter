use std::sync::Arc;

use user::UserId;

use crate::domain::{CredentialId, CredentialRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct DeletePasskeyCommand {
    pub user_id: UserId,
    pub credential_id: String,
}

pub struct DeletePasskeyHandler {
    credentials: Arc<dyn CredentialRepository>,
}

impl DeletePasskeyHandler {
    pub fn new(credentials: Arc<dyn CredentialRepository>) -> Self {
        Self { credentials }
    }

    pub async fn handle(&self, command: DeletePasskeyCommand) -> Result<(), AppError> {
        let credential_id =
            CredentialId::parse(&command.credential_id).map_err(|_| AppError::NotFound)?;

        let passkeys = self
            .credentials
            .find_passkeys_by_user_id(&command.user_id)
            .await?;
        if !passkeys.iter().any(|credential| credential.id() == &credential_id) {
            return Err(AppError::NotFound);
        }

        self.credentials.delete(&credential_id).await?;
        Ok(())
    }
}
