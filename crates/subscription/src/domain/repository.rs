use async_trait::async_trait;
use user::UserId;

use super::{
    ExtractedConfig, ExtractedRuleProvider, Publication, PublicationId, PublicationSecret,
    PublicationSnapshot, RepositoryError, RuleProviderSnapshot, Source, SourceExtraction, SourceId,
    SourceSnapshot,
};

#[async_trait]
pub trait SourceRepository: Send + Sync {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &SourceId,
    ) -> Result<Option<Source>, RepositoryError>;

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Source>, RepositoryError>;

    async fn save(&self, source: &Source) -> Result<(), RepositoryError>;

    async fn update(&self, source: &Source) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &SourceId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait PublicationRepository: Send + Sync {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &PublicationId,
    ) -> Result<Option<Publication>, RepositoryError>;

    async fn find_by_secret(
        &self,
        secret: &PublicationSecret,
    ) -> Result<Option<Publication>, RepositoryError>;

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Publication>, RepositoryError>;

    async fn list_by_source_id(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<Publication>, RepositoryError>;

    async fn save(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn update(&self, publication: &Publication) -> Result<(), RepositoryError>;

    async fn delete(&self, user_id: &UserId, id: &PublicationId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait SnapshotRepository: Send + Sync {
    async fn find_by_source(
        &self,
        source_id: &SourceId,
    ) -> Result<Option<SourceSnapshot>, RepositoryError>;

    async fn list_by_sources(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceSnapshot>, RepositoryError>;

    async fn list_extractions(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceExtraction>, RepositoryError>;

    async fn has_extraction(&self, source_id: &SourceId) -> Result<bool, RepositoryError>;

    async fn save(&self, snapshot: &SourceSnapshot) -> Result<(), RepositoryError>;

    async fn replace_extraction(
        &self,
        source_id: &SourceId,
        extraction: &ExtractedConfig,
    ) -> Result<(), RepositoryError>;

    async fn touch(&self, source_id: &SourceId, fetched_at: i64) -> Result<(), RepositoryError>;

    async fn record_error(&self, source_id: &SourceId, error: &str) -> Result<(), RepositoryError>;

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError>;

    async fn try_acquire_lease(
        &self,
        source_id: &SourceId,
        now: i64,
        lease_until: i64,
    ) -> Result<bool, RepositoryError>;
}

#[async_trait]
pub trait RuleProviderRepository: Send + Sync {
    /// 用最新的声明整体替换某源的 rule-provider 列表，并清理已移除 provider 的快照。
    async fn replace_providers(
        &self,
        source_id: &SourceId,
        providers: &[ExtractedRuleProvider],
    ) -> Result<(), RepositoryError>;

    async fn list_providers(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<ExtractedRuleProvider>, RepositoryError>;

    async fn find_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
    ) -> Result<Option<RuleProviderSnapshot>, RepositoryError>;

    async fn list_snapshots(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<RuleProviderSnapshot>, RepositoryError>;

    async fn save_snapshot(&self, snapshot: &RuleProviderSnapshot) -> Result<(), RepositoryError>;

    async fn touch_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
        fetched_at: i64,
    ) -> Result<(), RepositoryError>;

    async fn record_snapshot_error(
        &self,
        source_id: &SourceId,
        name: &str,
        error: &str,
    ) -> Result<(), RepositoryError>;

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait PublicationSnapshotRepository: Send + Sync {
    async fn find(
        &self,
        publication_id: &PublicationId,
        format: &str,
    ) -> Result<Option<PublicationSnapshot>, RepositoryError>;

    async fn save(&self, snapshot: &PublicationSnapshot) -> Result<(), RepositoryError>;

    async fn delete(&self, publication_id: &PublicationId) -> Result<(), RepositoryError>;
}
