use crate::domain::{Publication, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&Source> for SourceView {
    fn from(source: &Source) -> Self {
        Self {
            id: source.id().to_string(),
            name: source.name().value().to_string(),
            url: source.url().value().to_string(),
            enabled: source.enabled(),
            created_at: source.created_at(),
            updated_at: source.updated_at(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationView {
    pub id: String,
    pub name: String,
    pub secret: String,
    pub enabled: bool,
    pub expires_at: Option<i64>,
    pub source_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&Publication> for PublicationView {
    fn from(publication: &Publication) -> Self {
        Self {
            id: publication.id().to_string(),
            name: publication.name().value().to_string(),
            secret: publication.secret().value().to_string(),
            enabled: publication.enabled(),
            expires_at: publication.expires_at(),
            source_ids: publication
                .sources()
                .iter()
                .map(|source| source.source_id().to_string())
                .collect(),
            created_at: publication.created_at(),
            updated_at: publication.updated_at(),
        }
    }
}
