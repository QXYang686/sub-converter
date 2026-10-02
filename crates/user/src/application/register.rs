use std::sync::Arc;

use crate::application::error::AppError;
use crate::application::ports::{Clock, PasswordHasher};
use crate::domain::{validate_password, RepositoryError, User, UserId, UserRepository, Username};

use super::UserView;

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub username: String,
    pub password: String,
}

pub struct RegisterHandler {
    users: Arc<dyn UserRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    clock: Arc<dyn Clock>,
}

impl RegisterHandler {
    pub fn new(
        users: Arc<dyn UserRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            password_hasher,
            clock,
        }
    }

    pub async fn handle(&self, command: RegisterCommand) -> Result<UserView, AppError> {
        let username = Username::new(&command.username)?;
        validate_password(&command.password)?;

        if self.users.find_by_username(&username).await?.is_some() {
            return Err(AppError::UsernameTaken);
        }

        let password_hash = self.password_hasher.hash(&command.password).await?;
        let user = User::register(UserId::new(), username, password_hash, self.clock.now());

        match self.users.save(&user).await {
            Ok(()) => Ok(UserView::from(&user)),
            Err(RepositoryError::UsernameConflict) => Err(AppError::UsernameTaken),
            Err(err) => Err(err.into()),
        }
    }
}
