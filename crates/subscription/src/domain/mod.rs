mod clash;
mod error;
mod metadata;
mod publication;
mod repository;
mod shared;
mod source;

pub use clash::{
    document, merge, parse, parse_value, render, ClashError, ClashProxy, ExtractedConfig,
    ExtractedGroup, ExtractedProxy, ParsedClash, PROXY_GROUP_NAME,
};
pub use error::{DomainError, RepositoryError};
pub use metadata::{
    parse_content_disposition_filename, parse_subscription_userinfo, SubscriptionUserInfo,
};
pub use publication::{
    Publication, PublicationId, PublicationSecret, PublicationSnapshot, PublicationSource,
    PUBLICATION_MAX_SOURCES, PUBLICATION_SECRET_MAX_LEN, PUBLICATION_SECRET_MIN_LEN,
};
pub use repository::{
    PublicationRepository, PublicationSnapshotRepository, SnapshotRepository, SourceRepository,
};
pub use shared::{SubscriptionName, SUBSCRIPTION_NAME_MAX_LEN, SUBSCRIPTION_NAME_MIN_LEN};
pub use source::{
    body_hash, SnapshotMeta, Source, SourceId, SourceSnapshot, SourceUrl, SOURCE_URL_MAX_LEN,
};
