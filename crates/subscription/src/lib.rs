pub mod application;
pub mod domain;

pub use domain::{
    DomainError, Publication, PublicationId, PublicationRepository, PublicationSecret,
    PublicationSource, RepositoryError, SnapshotRepository, Source, SourceId, SourceRepository,
    SourceSnapshot, SourceUrl, SubscriptionName, SubscriptionUserInfo, PUBLICATION_MAX_SOURCES,
    PUBLICATION_SECRET_MAX_LEN, PUBLICATION_SECRET_MIN_LEN, SOURCE_URL_MAX_LEN,
    SUBSCRIPTION_NAME_MAX_LEN, SUBSCRIPTION_NAME_MIN_LEN,
};

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
