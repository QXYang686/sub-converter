use std::sync::Arc;

use user::UserId;

use crate::domain::{PublicationId, PublicationRepository, SubscriptionName};

use super::dto::PublicationView;
use super::error::AppError;
use super::ports::Clock;

#[derive(Debug, Clone)]
pub struct UpdatePublicationCommand {
    pub user_id: UserId,
    pub publication_id: String,
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub expires_at: Option<Option<i64>>,
}

pub struct UpdatePublicationHandler {
    publications: Arc<dyn PublicationRepository>,
    clock: Arc<dyn Clock>,
}

impl UpdatePublicationHandler {
    pub fn new(publications: Arc<dyn PublicationRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            publications,
            clock,
        }
    }

    pub async fn handle(
        &self,
        command: UpdatePublicationCommand,
    ) -> Result<PublicationView, AppError> {
        let publication_id =
            PublicationId::parse(&command.publication_id).map_err(|_| AppError::NotFound)?;
        let mut publication = self
            .publications
            .find_by_id(&command.user_id, &publication_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let now = self.clock.now();
        if let Some(name) = &command.name {
            let name = SubscriptionName::new(name).map_err(|_| AppError::InvalidName)?;
            publication.rename(name, now);
        }
        if let Some(enabled) = command.enabled {
            publication.set_enabled(enabled, now);
        }
        if let Some(expires_at) = command.expires_at {
            publication.set_expires_at(expires_at, now);
        }

        self.publications.update(&publication).await?;
        Ok(PublicationView::from(&publication))
    }
}
