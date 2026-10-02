use std::sync::Arc;

use user::UserId;

use crate::domain::{RepositoryError, SourceId, SourceRepository, SourceUrl, SubscriptionName};

use super::dto::SourceView;
use super::error::AppError;
use super::ports::Clock;

#[derive(Debug, Clone)]
pub struct UpdateSourceCommand {
    pub user_id: UserId,
    pub source_id: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
}

pub struct UpdateSourceHandler {
    sources: Arc<dyn SourceRepository>,
    clock: Arc<dyn Clock>,
}

impl UpdateSourceHandler {
    pub fn new(sources: Arc<dyn SourceRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { sources, clock }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, command: UpdateSourceCommand) -> Result<SourceView, AppError> {
        let source_id = SourceId::parse(&command.source_id).map_err(|_| AppError::NotFound)?;
        let mut source = self
            .sources
            .find_by_id(&command.user_id, &source_id)
            .await?
            .ok_or(AppError::NotFound)?;

        let now = self.clock.now();
        if let Some(name) = &command.name {
            let name = SubscriptionName::new(name).map_err(|_| AppError::InvalidName)?;
            source.rename(name, now);
        }
        if let Some(url) = &command.url {
            let url = SourceUrl::new(url).map_err(|_| AppError::InvalidSourceUrl)?;
            source.change_url(url, now);
        }
        if let Some(enabled) = command.enabled {
            source.set_enabled(enabled, now);
        }

        match self.sources.update(&source).await {
            Ok(()) => Ok(SourceView::from(&source)),
            Err(RepositoryError::SourceUrlConflict) => Err(AppError::SourceUrlTaken),
            Err(err) => Err(err.into()),
        }
    }
}
