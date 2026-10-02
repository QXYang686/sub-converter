use std::sync::Arc;

use user::UserId;

use crate::domain::{PublicationId, PublicationRepository};

use super::error::AppError;

#[derive(Debug, Clone)]
pub struct DeletePublicationCommand {
    pub user_id: UserId,
    pub publication_id: String,
}

pub struct DeletePublicationHandler {
    publications: Arc<dyn PublicationRepository>,
}

impl DeletePublicationHandler {
    pub fn new(publications: Arc<dyn PublicationRepository>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: DeletePublicationCommand) -> Result<(), AppError> {
        let publication_id =
            PublicationId::parse(&command.publication_id).map_err(|_| AppError::NotFound)?;
        if self
            .publications
            .find_by_id(&command.user_id, &publication_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        self.publications
            .delete(&command.user_id, &publication_id)
            .await?;
        Ok(())
    }
}
