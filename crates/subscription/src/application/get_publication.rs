use std::sync::Arc;

use user::UserId;

use crate::domain::{PublicationId, PublicationRepository};

use super::dto::PublicationView;
use super::error::AppError;

#[derive(Debug, Clone)]
pub struct GetPublicationCommand {
    pub user_id: UserId,
    pub publication_id: String,
}

pub struct GetPublicationHandler {
    publications: Arc<dyn PublicationRepository>,
}

impl GetPublicationHandler {
    pub fn new(publications: Arc<dyn PublicationRepository>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(
        &self,
        command: GetPublicationCommand,
    ) -> Result<PublicationView, AppError> {
        let publication_id =
            PublicationId::parse(&command.publication_id).map_err(|_| AppError::NotFound)?;
        let publication = self
            .publications
            .find_by_id(&command.user_id, &publication_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(PublicationView::from(&publication))
    }
}
