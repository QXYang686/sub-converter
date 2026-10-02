use std::sync::Arc;

use user::UserId;

use crate::domain::{
    Publication, PublicationId, PublicationRepository, PublicationSecret, RepositoryError,
    SourceRepository, SubscriptionName,
};

use super::dto::PublicationView;
use super::error::AppError;
use super::ports::{Clock, SecretGenerator};
use super::{ensure_sources_owned, map_binding_error, parse_source_ids};

#[derive(Debug, Clone)]
pub struct CreatePublicationCommand {
    pub user_id: UserId,
    pub name: String,
    pub source_ids: Vec<String>,
    pub expires_at: Option<i64>,
}

pub struct CreatePublicationHandler {
    sources: Arc<dyn SourceRepository>,
    publications: Arc<dyn PublicationRepository>,
    secret_generator: Arc<dyn SecretGenerator>,
    clock: Arc<dyn Clock>,
}

impl CreatePublicationHandler {
    pub fn new(
        sources: Arc<dyn SourceRepository>,
        publications: Arc<dyn PublicationRepository>,
        secret_generator: Arc<dyn SecretGenerator>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            sources,
            publications,
            secret_generator,
            clock,
        }
    }

    pub async fn handle(
        &self,
        command: CreatePublicationCommand,
    ) -> Result<PublicationView, AppError> {
        let name = SubscriptionName::new(&command.name).map_err(|_| AppError::InvalidName)?;
        let source_ids = parse_source_ids(&command.source_ids)?;
        ensure_sources_owned(self.sources.as_ref(), &command.user_id, &source_ids).await?;

        let now = self.clock.now();
        let raw_secret = self.secret_generator.generate()?;
        let secret = PublicationSecret::new(&raw_secret).map_err(|_| AppError::InvalidSecret)?;

        let mut publication =
            Publication::create(PublicationId::new(), command.user_id, name, secret, now);
        if let Some(expires_at) = command.expires_at {
            publication.set_expires_at(Some(expires_at), now);
        }
        publication
            .replace_sources(source_ids, now)
            .map_err(map_binding_error)?;

        match self.publications.save(&publication).await {
            Ok(()) => Ok(PublicationView::from(&publication)),
            Err(RepositoryError::SecretConflict) => Err(AppError::Internal(
                "publication secret collision".to_string(),
            )),
            Err(err) => Err(err.into()),
        }
    }
}
