mod clash;
mod error;
mod publication;
mod repository;
mod snapshot;
mod source;
mod value_object;
mod yaml_json;

pub use clash::{
    document, merge, parse, parse_value, render, ClashError, ClashProxy, ParsedClash,
    PROXY_GROUP_NAME,
};
pub use error::{DomainError, RepositoryError};
pub use publication::{Publication, PublicationSource};
pub use repository::{PublicationRepository, SnapshotRepository, SourceRepository};
pub use snapshot::SourceSnapshot;
pub use source::Source;
pub use value_object::{
    PublicationId, PublicationSecret, SourceId, SourceUrl, SubscriptionName,
    PUBLICATION_MAX_SOURCES, PUBLICATION_SECRET_MAX_LEN, PUBLICATION_SECRET_MIN_LEN,
    SOURCE_URL_MAX_LEN, SUBSCRIPTION_NAME_MAX_LEN, SUBSCRIPTION_NAME_MIN_LEN,
};
