mod d1_publication_repository;
mod d1_publication_snapshot_repository;
mod d1_rule_provider_repository;
mod d1_snapshot_repository;
mod d1_source_repository;

use crate::domain::RepositoryError;

pub use d1_publication_repository::D1PublicationRepository;
pub use d1_publication_snapshot_repository::D1PublicationSnapshotRepository;
pub use d1_rule_provider_repository::D1RuleProviderRepository;
pub use d1_snapshot_repository::D1SnapshotRepository;
pub use d1_source_repository::D1SourceRepository;

impl From<worker::Error> for RepositoryError {
    fn from(err: worker::Error) -> Self {
        let message = err.to_string();
        if !message.contains("UNIQUE constraint failed") {
            return RepositoryError::Unavailable(message);
        }
        if message.contains("publications.secret") {
            RepositoryError::SecretConflict
        } else {
            RepositoryError::SourceUrlConflict
        }
    }
}
