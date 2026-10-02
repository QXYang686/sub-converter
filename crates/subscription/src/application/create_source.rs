use std::sync::Arc;

use user::UserId;

use crate::domain::{
    RepositoryError, Source, SourceId, SourceRepository, SourceUrl, SubscriptionName,
};

use super::dto::SourceView;
use super::error::AppError;
use super::ports::Clock;

#[derive(Debug, Clone)]
pub struct CreateSourceCommand {
    pub user_id: UserId,
    pub name: String,
    pub url: String,
    pub enabled: Option<bool>,
}

pub struct CreateSourceHandler {
    sources: Arc<dyn SourceRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateSourceHandler {
    pub fn new(sources: Arc<dyn SourceRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { sources, clock }
    }

    pub async fn handle(&self, command: CreateSourceCommand) -> Result<SourceView, AppError> {
        let name = SubscriptionName::new(&command.name).map_err(|_| AppError::InvalidName)?;
        let url = SourceUrl::new(&command.url).map_err(|_| AppError::InvalidSourceUrl)?;

        let now = self.clock.now();
        let mut source = Source::create(SourceId::new(), command.user_id, name, url, now);
        if let Some(enabled) = command.enabled {
            source.set_enabled(enabled, now);
        }

        match self.sources.save(&source).await {
            Ok(()) => Ok(SourceView::from(&source)),
            Err(RepositoryError::SourceUrlConflict) => Err(AppError::SourceUrlTaken),
            Err(err) => Err(err.into()),
        }
    }
}
