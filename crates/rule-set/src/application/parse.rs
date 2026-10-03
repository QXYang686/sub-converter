use crate::domain::{
    RepositoryError, RuleSetCategory, RuleSetContentFormat, RuleSetPath, RuleSetSource, RuleSetUrl,
};

use super::error::AppError;

pub(super) fn map_repository_error(err: RepositoryError) -> AppError {
    match err {
        RepositoryError::NameConflict => AppError::NameTaken,
        other => AppError::Repository(other),
    }
}

pub(super) fn parse_category(raw: Option<&str>) -> Result<Option<RuleSetCategory>, AppError> {
    match raw {
        None => Ok(None),
        Some(value) => RuleSetCategory::parse(value)
            .map(Some)
            .ok_or(AppError::InvalidCategory),
    }
}

pub(super) fn parse_content_format(raw: &str) -> Result<RuleSetContentFormat, AppError> {
    RuleSetContentFormat::parse(raw).ok_or(AppError::InvalidContentFormat)
}

pub(super) fn parse_source(
    kind: &str,
    url: Option<&str>,
    path: Option<&str>,
) -> Result<RuleSetSource, AppError> {
    match kind {
        "remote" => {
            let url = RuleSetUrl::new(url.ok_or(AppError::InvalidUrl)?)?;
            Ok(RuleSetSource::Remote(url))
        }
        "inline" => Ok(RuleSetSource::Inline),
        "local" => {
            let path = RuleSetPath::new(path.ok_or(AppError::InvalidPath)?)?;
            Ok(RuleSetSource::Local(path))
        }
        _ => Err(AppError::InvalidSourceKind),
    }
}
