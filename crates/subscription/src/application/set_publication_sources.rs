use std::sync::Arc;

use user::UserId;

use crate::domain::{PublicationId, PublicationRepository, SourceRepository};

use super::dto::PublicationView;
use super::error::AppError;
use super::ports::Clock;
use super::{ensure_sources_owned, map_binding_error, parse_source_ids};

#[derive(Debug, Clone)]
pub struct SetPublicationSourcesCommand {
    pub user_id: UserId,
    pub publication_id: String,
    pub source_ids: Vec<String>,
}

pub struct SetPublicationSourcesHandler {
    sources: Arc<dyn SourceRepository>,
    publications: Arc<dyn PublicationRepository>,
    clock: Arc<dyn Clock>,
}

impl SetPublicationSourcesHandler {
    pub fn new(
        sources: Arc<dyn SourceRepository>,
        publications: Arc<dyn PublicationRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            sources,
            publications,
            clock,
        }
    }

    pub async fn handle(
        &self,
        command: SetPublicationSourcesCommand,
    ) -> Result<PublicationView, AppError> {
        let publication_id =
            PublicationId::parse(&command.publication_id).map_err(|_| AppError::NotFound)?;
        let source_ids = parse_source_ids(&command.source_ids)?;

        let mut publication = self
            .publications
            .find_by_id(&command.user_id, &publication_id)
            .await?
            .ok_or(AppError::NotFound)?;
        ensure_sources_owned(self.sources.as_ref(), &command.user_id, &source_ids).await?;

        let now = self.clock.now();
        publication
            .replace_sources(source_ids, now)
            .map_err(map_binding_error)?;
        self.publications.update(&publication).await?;
        Ok(PublicationView::from(&publication))
    }
}
