mod d1_rule_set_repository;

use crate::domain::RepositoryError;

pub use d1_rule_set_repository::D1RuleSetRepository;

impl From<worker::Error> for RepositoryError {
    fn from(err: worker::Error) -> Self {
        let message = err.to_string();
        if message.contains("UNIQUE constraint failed") {
            RepositoryError::NameConflict
        } else {
            RepositoryError::Unavailable(message)
        }
    }
}
