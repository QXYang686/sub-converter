use std::sync::Arc;

use user::{RepositoryError, User, UserId, UserRepository, Username};

use crate::domain::{validate_password, Credential, CredentialId, CredentialRepository};

use super::dto::UserView;
use super::error::AppError;
use super::ports::{Clock, PasswordHasher};

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub username: String,
    pub password: String,
}

pub struct RegisterHandler {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    clock: Arc<dyn Clock>,
}

impl RegisterHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            credentials,
            password_hasher,
            clock,
        }
    }

    pub async fn handle(&self, command: RegisterCommand) -> Result<UserView, AppError> {
        let username = Username::new(&command.username).map_err(|_| AppError::InvalidUsername)?;
        validate_password(&command.password).map_err(|_| AppError::InvalidPassword)?;

        if self.users.find_by_username(&username).await?.is_some() {
            return Err(AppError::UsernameTaken);
        }

        let now = self.clock.now();
        let user = User::register(UserId::new(), username, now);
        match self.users.save(&user).await {
            Ok(()) => {}
            Err(RepositoryError::UsernameConflict) => return Err(AppError::UsernameTaken),
            Err(err) => return Err(err.into()),
        }

        let password_hash = self.password_hasher.hash(&command.password).await?;
        let credential = Credential::password(
            CredentialId::new(),
            user.id().clone(),
            password_hash,
            now,
        );
        if let Err(err) = self.credentials.save(&credential).await {
            let _ = self.users.delete(user.id()).await;
            return Err(AppError::Internal(format!(
                "failed to store credential: {err}"
            )));
        }

        Ok(UserView::from(&user))
    }
}
