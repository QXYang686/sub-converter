mod clash;
mod error;
mod extract;
mod metadata;
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
pub use extract::{ExtractedConfig, ExtractedGroup, ExtractedProxy};
pub use metadata::{
    parse_content_disposition_filename, parse_subscription_userinfo, SubscriptionUserInfo,
};
pub use publication::{Publication, PublicationSource};
pub use repository::{PublicationRepository, SnapshotRepository, SourceRepository};
pub use snapshot::{body_hash, SnapshotMeta, SourceSnapshot};
pub use source::Source;
pub use value_object::{
    PublicationId, PublicationSecret, SourceId, SourceUrl, SubscriptionName,
    PUBLICATION_MAX_SOURCES, PUBLICATION_SECRET_MAX_LEN, PUBLICATION_SECRET_MIN_LEN,
    SOURCE_URL_MAX_LEN, SUBSCRIPTION_NAME_MAX_LEN, SUBSCRIPTION_NAME_MIN_LEN,
};
