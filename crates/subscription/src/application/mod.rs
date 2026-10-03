mod create_publication;
mod create_source;
mod delete_publication;
mod delete_source;
mod dto;
mod error;
mod get_publication;
mod get_source;
mod list_publications;
mod list_sources;
mod ports;
mod rebuild_publication_snapshot;
mod refresh_source;
mod serve_publication;
mod set_publication_sources;
mod update_publication;
mod update_source;

use std::collections::HashSet;

use user::UserId;

use crate::domain::{DomainError, SourceId, SourceRepository};

pub use create_publication::{CreatePublicationCommand, CreatePublicationHandler};
pub use create_source::{CreateSourceCommand, CreateSourceHandler};
pub use delete_publication::{DeletePublicationCommand, DeletePublicationHandler};
pub use delete_source::{DeleteSourceCommand, DeleteSourceHandler};
pub use dto::{ProtocolCountView, PublicationView, SourceSnapshotView, SourceView};
pub use error::AppError;
pub use get_publication::{GetPublicationCommand, GetPublicationHandler};
pub use get_source::{GetSourceCommand, GetSourceHandler};
pub use list_publications::ListPublicationsHandler;
pub use list_sources::ListSourcesHandler;
pub use ports::{
    BackgroundTask, BackgroundTasks, Clock, FetchError, FetchOutcome, FetchValidators,
    FetchedDocument, Fetcher, PortError, SecretGenerator, SubscriptionFormat,
};
pub use rebuild_publication_snapshot::RebuildPublicationSnapshotHandler;
pub use refresh_source::{RefreshSourceHandler, RefreshStatus, REFRESH_LEASE_SECONDS};
pub use serve_publication::{
    GeneratedSubscription, ServePublicationCommand, ServePublicationHandler,
};
pub use set_publication_sources::{SetPublicationSourcesCommand, SetPublicationSourcesHandler};
pub use update_publication::{UpdatePublicationCommand, UpdatePublicationHandler};
pub use update_source::{UpdateSourceCommand, UpdateSourceHandler};

pub(crate) async fn ensure_sources_owned(
    sources: &dyn SourceRepository,
    user_id: &UserId,
    source_ids: &[SourceId],
) -> Result<(), AppError> {
    if source_ids.is_empty() {
        return Ok(());
    }

    let owned = sources.list_by_user(user_id).await?;
    let owned_ids: HashSet<SourceId> = owned
        .into_iter()
        .map(|source| source.id().clone())
        .collect();
    if source_ids.iter().any(|id| !owned_ids.contains(id)) {
        return Err(AppError::UnknownSource);
    }
    Ok(())
}

pub(crate) fn parse_source_ids(raw: &[String]) -> Result<Vec<SourceId>, AppError> {
    raw.iter()
        .map(|id| SourceId::parse(id).map_err(|_| AppError::InvalidSourceId))
        .collect()
}

pub(crate) fn map_binding_error(err: DomainError) -> AppError {
    match err {
        DomainError::DuplicateSource => AppError::DuplicateSource,
        DomainError::TooManySources => AppError::TooManySources,
        other => AppError::Internal(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
