use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::executor::block_on;
use user::UserId;

use crate::domain::{
    body_hash, document as parse_document, ExtractedConfig, ExtractedRuleProvider, Publication,
    PublicationId, PublicationRepository, PublicationSnapshot, PublicationSnapshotRepository,
    RepositoryError, RuleProviderRepository, RuleProviderSnapshot, SnapshotMeta,
    SnapshotRepository, Source, SourceExtraction, SourceId, SourceRepository, SourceSnapshot,
    SourceUrl, SubscriptionUserInfo,
};

use super::ports::{
    BackgroundTask, BackgroundTasks, Clock, FetchError, FetchOutcome, FetchValidators,
    FetchedDocument, Fetcher, PortError, SecretGenerator, SubscriptionFormat,
};
use super::{
    AppError, CreatePublicationCommand, CreatePublicationHandler, CreateSourceCommand,
    CreateSourceHandler, DeletePublicationCommand, DeletePublicationHandler, DeleteSourceCommand,
    DeleteSourceHandler, GetPublicationCommand, GetPublicationContentCommand,
    GetPublicationContentHandler, GetPublicationHandler, GetSourceCommand, GetSourceContentCommand,
    GetSourceContentHandler, GetSourceHandler, GetSourceProviderContentCommand,
    GetSourceProviderContentHandler, ListPublicationsHandler, ListSourceProvidersCommand,
    ListSourceProvidersHandler, ListSourcesHandler, PublicationView,
    RebuildPublicationSnapshotHandler, RefreshSourceHandler, RefreshStatus,
    ServePublicationCommand, ServePublicationHandler, SetPublicationSourcesCommand,
    SetPublicationSourcesHandler, SourceView, UpdatePublicationCommand, UpdatePublicationHandler,
    UpdateSourceCommand, UpdateSourceHandler,
};

#[derive(Default)]
struct Store {
    sources: Mutex<Vec<Source>>,
    publications: Mutex<Vec<Publication>>,
    snapshots: Mutex<Vec<StoredSnapshot>>,
    extracted: Mutex<HashMap<SourceId, ExtractedConfig>>,
    rule_providers: Mutex<HashMap<SourceId, Vec<ExtractedRuleProvider>>>,
    rule_provider_snapshots: Mutex<HashMap<(SourceId, String), RuleProviderSnapshot>>,
}

#[derive(Clone)]
struct StoredSnapshot {
    source_id: SourceId,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
    meta: SnapshotMeta,
    lease_until: Option<i64>,
}

#[derive(Clone, Default)]
struct InMemorySourceRepository {
    store: Arc<Store>,
}

#[derive(Clone, Default)]
struct InMemoryPublicationRepository {
    store: Arc<Store>,
}

#[derive(Clone, Default)]
struct InMemorySnapshotRepository {
    store: Arc<Store>,
    replace_calls: Arc<AtomicUsize>,
}

impl InMemorySnapshotRepository {
    fn last_error(&self, source_id: &SourceId) -> Option<String> {
        self.store
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .find(|stored| &stored.source_id == source_id)
            .and_then(|stored| stored.meta.last_error.clone())
    }

    fn extraction(&self, source_id: &SourceId) -> Option<ExtractedConfig> {
        self.store.extracted.lock().unwrap().get(source_id).cloned()
    }

    fn replace_calls(&self) -> usize {
        self.replace_calls.load(Ordering::SeqCst)
    }

    fn to_snapshot(stored: &StoredSnapshot) -> SourceSnapshot {
        SourceSnapshot::restore(
            stored.source_id.clone(),
            stored.body.clone(),
            stored.etag.clone(),
            stored.last_modified.clone(),
            stored.fetched_at,
            stored.meta.clone(),
        )
    }
}

#[async_trait]
impl SnapshotRepository for InMemorySnapshotRepository {
    async fn find_by_source(
        &self,
        source_id: &SourceId,
    ) -> Result<Option<SourceSnapshot>, RepositoryError> {
        Ok(self
            .store
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .find(|stored| &stored.source_id == source_id)
            .map(Self::to_snapshot))
    }

    async fn list_by_sources(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceSnapshot>, RepositoryError> {
        Ok(self
            .store
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .filter(|stored| !stored.body.is_empty() && source_ids.contains(&stored.source_id))
            .map(Self::to_snapshot)
            .collect())
    }

    async fn list_extractions(
        &self,
        source_ids: &[SourceId],
    ) -> Result<Vec<SourceExtraction>, RepositoryError> {
        let snapshots = self.store.snapshots.lock().unwrap();
        let extracted = self.store.extracted.lock().unwrap();
        let mut result = Vec::new();
        for source_id in source_ids {
            let Some(stored) = snapshots
                .iter()
                .find(|stored| &stored.source_id == source_id)
            else {
                continue;
            };
            result.push(SourceExtraction {
                source_id: source_id.clone(),
                userinfo: stored.meta.userinfo.clone(),
                config: extracted.get(source_id).cloned().unwrap_or_default(),
            });
        }
        Ok(result)
    }

    async fn has_extraction(&self, source_id: &SourceId) -> Result<bool, RepositoryError> {
        Ok(self.store.extracted.lock().unwrap().contains_key(source_id))
    }

    async fn save(&self, snapshot: &SourceSnapshot) -> Result<(), RepositoryError> {
        let mut snapshots = self.store.snapshots.lock().unwrap();
        let stored = StoredSnapshot {
            source_id: snapshot.source_id().clone(),
            body: snapshot.body().to_vec(),
            etag: snapshot.etag().map(str::to_string),
            last_modified: snapshot.last_modified().map(str::to_string),
            fetched_at: snapshot.fetched_at(),
            meta: snapshot.meta().clone(),
            lease_until: None,
        };
        match snapshots
            .iter_mut()
            .find(|existing| existing.source_id == stored.source_id)
        {
            Some(existing) => *existing = stored,
            None => snapshots.push(stored),
        }
        Ok(())
    }

    async fn replace_extraction(
        &self,
        source_id: &SourceId,
        extraction: &ExtractedConfig,
    ) -> Result<(), RepositoryError> {
        self.replace_calls.fetch_add(1, Ordering::SeqCst);
        self.store
            .extracted
            .lock()
            .unwrap()
            .insert(source_id.clone(), extraction.clone());
        Ok(())
    }

    async fn touch(&self, source_id: &SourceId, fetched_at: i64) -> Result<(), RepositoryError> {
        if let Some(stored) = self
            .store
            .snapshots
            .lock()
            .unwrap()
            .iter_mut()
            .find(|stored| &stored.source_id == source_id)
        {
            stored.fetched_at = fetched_at;
            stored.lease_until = None;
            stored.meta.last_error = None;
        }
        Ok(())
    }

    async fn record_error(&self, source_id: &SourceId, error: &str) -> Result<(), RepositoryError> {
        let mut snapshots = self.store.snapshots.lock().unwrap();
        match snapshots
            .iter_mut()
            .find(|stored| &stored.source_id == source_id)
        {
            Some(stored) => {
                stored.lease_until = None;
                stored.meta.last_error = Some(error.to_string());
            }
            None => snapshots.push(StoredSnapshot {
                source_id: source_id.clone(),
                body: Vec::new(),
                etag: None,
                last_modified: None,
                fetched_at: 0,
                meta: SnapshotMeta {
                    last_error: Some(error.to_string()),
                    ..SnapshotMeta::default()
                },
                lease_until: None,
            }),
        }
        Ok(())
    }

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError> {
        self.store
            .snapshots
            .lock()
            .unwrap()
            .retain(|stored| &stored.source_id != source_id);
        self.store.extracted.lock().unwrap().remove(source_id);
        Ok(())
    }

    async fn try_acquire_lease(
        &self,
        source_id: &SourceId,
        now: i64,
        lease_until: i64,
    ) -> Result<bool, RepositoryError> {
        let mut snapshots = self.store.snapshots.lock().unwrap();
        match snapshots
            .iter_mut()
            .find(|stored| &stored.source_id == source_id)
        {
            Some(stored) => {
                let available = stored.lease_until.is_none_or(|lease| lease < now);
                if available {
                    stored.lease_until = Some(lease_until);
                }
                Ok(available)
            }
            None => {
                snapshots.push(StoredSnapshot {
                    source_id: source_id.clone(),
                    body: Vec::new(),
                    etag: None,
                    last_modified: None,
                    fetched_at: 0,
                    meta: SnapshotMeta::default(),
                    lease_until: Some(lease_until),
                });
                Ok(true)
            }
        }
    }
}

#[derive(Clone, Default)]
struct InMemoryRuleProviderRepository {
    store: Arc<Store>,
}

#[async_trait]
impl RuleProviderRepository for InMemoryRuleProviderRepository {
    async fn replace_providers(
        &self,
        source_id: &SourceId,
        providers: &[ExtractedRuleProvider],
    ) -> Result<(), RepositoryError> {
        let names: std::collections::HashSet<&str> = providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect();
        self.store
            .rule_provider_snapshots
            .lock()
            .unwrap()
            .retain(|(id, name), _| id != source_id || names.contains(name.as_str()));
        self.store
            .rule_providers
            .lock()
            .unwrap()
            .insert(source_id.clone(), providers.to_vec());
        Ok(())
    }

    async fn list_providers(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<ExtractedRuleProvider>, RepositoryError> {
        Ok(self
            .store
            .rule_providers
            .lock()
            .unwrap()
            .get(source_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn find_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
    ) -> Result<Option<RuleProviderSnapshot>, RepositoryError> {
        Ok(self
            .store
            .rule_provider_snapshots
            .lock()
            .unwrap()
            .get(&(source_id.clone(), name.to_string()))
            .cloned())
    }

    async fn list_snapshots(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<RuleProviderSnapshot>, RepositoryError> {
        Ok(self
            .store
            .rule_provider_snapshots
            .lock()
            .unwrap()
            .iter()
            .filter(|((id, _), _)| id == source_id)
            .map(|(_, snapshot)| snapshot.clone())
            .collect())
    }

    async fn save_snapshot(&self, snapshot: &RuleProviderSnapshot) -> Result<(), RepositoryError> {
        self.store.rule_provider_snapshots.lock().unwrap().insert(
            (snapshot.source_id().clone(), snapshot.name().to_string()),
            snapshot.clone(),
        );
        Ok(())
    }

    async fn touch_snapshot(
        &self,
        source_id: &SourceId,
        name: &str,
        fetched_at: i64,
    ) -> Result<(), RepositoryError> {
        if let Some(snapshot) = self
            .store
            .rule_provider_snapshots
            .lock()
            .unwrap()
            .get_mut(&(source_id.clone(), name.to_string()))
        {
            snapshot.mark_refreshed(fetched_at);
        }
        Ok(())
    }

    async fn record_snapshot_error(
        &self,
        source_id: &SourceId,
        name: &str,
        error: &str,
    ) -> Result<(), RepositoryError> {
        let mut snapshots = self.store.rule_provider_snapshots.lock().unwrap();
        let key = (source_id.clone(), name.to_string());
        match snapshots.get_mut(&key) {
            Some(snapshot) => snapshot.set_error(error),
            None => {
                snapshots.insert(
                    key,
                    RuleProviderSnapshot::restore(
                        source_id.clone(),
                        name.to_string(),
                        Vec::new(),
                        None,
                        None,
                        0,
                        String::new(),
                        0,
                        Some(error.to_string()),
                    ),
                );
            }
        }
        Ok(())
    }

    async fn clear(&self, source_id: &SourceId) -> Result<(), RepositoryError> {
        self.store.rule_providers.lock().unwrap().remove(source_id);
        self.store
            .rule_provider_snapshots
            .lock()
            .unwrap()
            .retain(|(id, _), _| id != source_id);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct InMemoryPublicationSnapshotRepository {
    store: Arc<Mutex<HashMap<(PublicationId, String), PublicationSnapshot>>>,
}

impl InMemoryPublicationSnapshotRepository {
    fn get(&self, publication_id: &PublicationId, format: &str) -> Option<PublicationSnapshot> {
        self.store
            .lock()
            .unwrap()
            .get(&(publication_id.clone(), format.to_string()))
            .cloned()
    }
}

#[async_trait]
impl PublicationSnapshotRepository for InMemoryPublicationSnapshotRepository {
    async fn find(
        &self,
        publication_id: &PublicationId,
        format: &str,
    ) -> Result<Option<PublicationSnapshot>, RepositoryError> {
        Ok(self.get(publication_id, format))
    }

    async fn save(&self, snapshot: &PublicationSnapshot) -> Result<(), RepositoryError> {
        self.store.lock().unwrap().insert(
            (
                snapshot.publication_id().clone(),
                snapshot.format().to_string(),
            ),
            snapshot.clone(),
        );
        Ok(())
    }

    async fn delete(&self, publication_id: &PublicationId) -> Result<(), RepositoryError> {
        self.store
            .lock()
            .unwrap()
            .retain(|(id, _), _| id != publication_id);
        Ok(())
    }
}

#[derive(Default)]
struct FakeFetcher {
    responses: Mutex<HashMap<String, Result<FetchOutcome, FetchError>>>,
    calls: AtomicUsize,
    last_validators: Mutex<Option<FetchValidators>>,
}

impl FakeFetcher {
    fn stub(&self, url: &str, outcome: Result<FetchOutcome, FetchError>) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), outcome);
    }
}

#[async_trait]
impl Fetcher for FakeFetcher {
    async fn fetch(
        &self,
        url: &SourceUrl,
        validators: &FetchValidators,
    ) -> Result<FetchOutcome, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_validators.lock().unwrap() = Some(validators.clone());
        self.responses
            .lock()
            .unwrap()
            .get(url.value())
            .cloned()
            .unwrap_or(Err(FetchError::Failure("no stub".to_string())))
    }
}

#[derive(Default)]
struct RecordingBackgroundTasks {
    tasks: Mutex<Vec<BackgroundTask>>,
}

impl RecordingBackgroundTasks {
    fn pending(&self) -> usize {
        self.tasks.lock().unwrap().len()
    }

    fn run_all(&self) {
        loop {
            let task = self.tasks.lock().unwrap().pop();
            match task {
                Some(task) => block_on(task),
                None => break,
            }
        }
    }
}

impl BackgroundTasks for RecordingBackgroundTasks {
    fn spawn(&self, task: BackgroundTask) {
        self.tasks.lock().unwrap().push(task);
    }
}

fn publication_with_existing_sources(
    publication: &Publication,
    existing: &HashSet<SourceId>,
) -> Publication {
    let sources = publication
        .sources()
        .iter()
        .filter(|source| existing.contains(source.source_id()))
        .cloned()
        .collect();
    Publication::restore(
        publication.id().clone(),
        publication.user_id().clone(),
        publication.name().clone(),
        publication.secret().clone(),
        publication.enabled(),
        publication.expires_at(),
        publication.created_at(),
        publication.updated_at(),
        sources,
    )
}

#[async_trait]
impl SourceRepository for InMemorySourceRepository {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &SourceId,
    ) -> Result<Option<Source>, RepositoryError> {
        Ok(self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .find(|source| source.user_id() == user_id && source.id() == id)
            .cloned())
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Source>, RepositoryError> {
        Ok(self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .filter(|source| source.user_id() == user_id)
            .cloned()
            .collect())
    }

    async fn save(&self, source: &Source) -> Result<(), RepositoryError> {
        let mut sources = self.store.sources.lock().unwrap();
        let duplicate = sources.iter().any(|existing| {
            existing.user_id() == source.user_id() && existing.url() == source.url()
        });
        if duplicate {
            return Err(RepositoryError::SourceUrlConflict);
        }
        sources.push(source.clone());
        Ok(())
    }

    async fn update(&self, source: &Source) -> Result<(), RepositoryError> {
        let mut sources = self.store.sources.lock().unwrap();
        let duplicate = sources.iter().any(|existing| {
            existing.id() != source.id()
                && existing.user_id() == source.user_id()
                && existing.url() == source.url()
        });
        if duplicate {
            return Err(RepositoryError::SourceUrlConflict);
        }
        if let Some(slot) = sources
            .iter_mut()
            .find(|existing| existing.id() == source.id())
        {
            *slot = source.clone();
        }
        Ok(())
    }

    async fn delete(&self, user_id: &UserId, id: &SourceId) -> Result<(), RepositoryError> {
        self.store
            .sources
            .lock()
            .unwrap()
            .retain(|source| !(source.user_id() == user_id && source.id() == id));
        Ok(())
    }
}

#[async_trait]
impl PublicationRepository for InMemoryPublicationRepository {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &PublicationId,
    ) -> Result<Option<Publication>, RepositoryError> {
        let existing: HashSet<SourceId> = self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .map(|source| source.id().clone())
            .collect();
        let publications = self.store.publications.lock().unwrap();
        Ok(publications
            .iter()
            .find(|publication| publication.user_id() == user_id && publication.id() == id)
            .map(|publication| publication_with_existing_sources(publication, &existing)))
    }

    async fn find_by_secret(
        &self,
        secret: &crate::domain::PublicationSecret,
    ) -> Result<Option<Publication>, RepositoryError> {
        let existing: HashSet<SourceId> = self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .map(|source| source.id().clone())
            .collect();
        let publications = self.store.publications.lock().unwrap();
        Ok(publications
            .iter()
            .find(|publication| publication.secret() == secret)
            .map(|publication| publication_with_existing_sources(publication, &existing)))
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<Publication>, RepositoryError> {
        let existing: HashSet<SourceId> = self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .map(|source| source.id().clone())
            .collect();
        Ok(self
            .store
            .publications
            .lock()
            .unwrap()
            .iter()
            .filter(|publication| publication.user_id() == user_id)
            .map(|publication| publication_with_existing_sources(publication, &existing))
            .collect())
    }

    async fn list_by_source_id(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<Publication>, RepositoryError> {
        let existing: HashSet<SourceId> = self
            .store
            .sources
            .lock()
            .unwrap()
            .iter()
            .map(|source| source.id().clone())
            .collect();
        Ok(self
            .store
            .publications
            .lock()
            .unwrap()
            .iter()
            .filter(|publication| {
                publication
                    .sources()
                    .iter()
                    .any(|binding| binding.source_id() == source_id)
            })
            .map(|publication| publication_with_existing_sources(publication, &existing))
            .collect())
    }

    async fn save(&self, publication: &Publication) -> Result<(), RepositoryError> {
        let mut publications = self.store.publications.lock().unwrap();
        if publications
            .iter()
            .any(|existing| existing.secret() == publication.secret())
        {
            return Err(RepositoryError::SecretConflict);
        }
        publications.push(publication.clone());
        Ok(())
    }

    async fn update(&self, publication: &Publication) -> Result<(), RepositoryError> {
        let mut publications = self.store.publications.lock().unwrap();
        if let Some(slot) = publications
            .iter_mut()
            .find(|existing| existing.id() == publication.id())
        {
            *slot = publication.clone();
        }
        Ok(())
    }

    async fn delete(&self, user_id: &UserId, id: &PublicationId) -> Result<(), RepositoryError> {
        self.store
            .publications
            .lock()
            .unwrap()
            .retain(|publication| !(publication.user_id() == user_id && publication.id() == id));
        Ok(())
    }
}

struct FakeClock(AtomicI64);

impl FakeClock {
    fn new(now: i64) -> Self {
        Self(AtomicI64::new(now))
    }

    fn set(&self, now: i64) {
        self.0.store(now, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct FakeSecretGenerator(AtomicU64);

impl SecretGenerator for FakeSecretGenerator {
    fn generate(&self) -> Result<String, PortError> {
        let next = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(format!("secret-{next:0>36}"))
    }
}

struct Fixture {
    sources: Arc<InMemorySourceRepository>,
    publications: Arc<InMemoryPublicationRepository>,
    snapshots: Arc<InMemorySnapshotRepository>,
    rule_providers: Arc<InMemoryRuleProviderRepository>,
    publication_snapshots: Arc<InMemoryPublicationSnapshotRepository>,
    fetcher: Arc<FakeFetcher>,
    background: Arc<RecordingBackgroundTasks>,
    clock: Arc<FakeClock>,
    secrets: Arc<FakeSecretGenerator>,
}

impl Fixture {
    fn new(now: i64) -> Self {
        let store = Arc::new(Store::default());
        Self {
            sources: Arc::new(InMemorySourceRepository {
                store: store.clone(),
            }),
            publications: Arc::new(InMemoryPublicationRepository {
                store: store.clone(),
            }),
            snapshots: Arc::new(InMemorySnapshotRepository {
                store: store.clone(),
                replace_calls: Arc::new(AtomicUsize::new(0)),
            }),
            rule_providers: Arc::new(InMemoryRuleProviderRepository { store }),
            publication_snapshots: Arc::new(InMemoryPublicationSnapshotRepository::default()),
            fetcher: Arc::new(FakeFetcher::default()),
            background: Arc::new(RecordingBackgroundTasks::default()),
            clock: Arc::new(FakeClock::new(now)),
            secrets: Arc::new(FakeSecretGenerator(AtomicU64::new(0))),
        }
    }

    fn refresh_source(&self) -> RefreshSourceHandler {
        RefreshSourceHandler::new(
            self.fetcher.clone(),
            self.snapshots.clone(),
            self.sources.clone(),
            self.publications.clone(),
            self.publication_snapshots.clone(),
            self.clock.clone(),
            self.rule_providers.clone(),
        )
    }

    fn list_source_providers(&self) -> ListSourceProvidersHandler {
        ListSourceProvidersHandler::new(self.sources.clone(), self.rule_providers.clone())
    }

    fn get_source_provider_content(&self) -> GetSourceProviderContentHandler {
        GetSourceProviderContentHandler::new(self.sources.clone(), self.rule_providers.clone())
    }

    fn rebuild_publication(&self) -> RebuildPublicationSnapshotHandler {
        RebuildPublicationSnapshotHandler::new(
            self.sources.clone(),
            self.snapshots.clone(),
            self.publication_snapshots.clone(),
            self.clock.clone(),
        )
    }

    fn serve_publication(&self) -> ServePublicationHandler {
        ServePublicationHandler::new(
            self.publications.clone(),
            self.sources.clone(),
            self.snapshots.clone(),
            self.publication_snapshots.clone(),
            Arc::new(self.refresh_source()),
            self.background.clone(),
            self.clock.clone(),
        )
    }

    fn create_source(&self) -> CreateSourceHandler {
        CreateSourceHandler::new(self.sources.clone(), self.clock.clone())
    }

    fn list_sources(&self) -> ListSourcesHandler {
        ListSourcesHandler::new(self.sources.clone(), self.snapshots.clone())
    }

    fn get_source(&self) -> GetSourceHandler {
        GetSourceHandler::new(self.sources.clone(), self.snapshots.clone())
    }

    fn get_source_content(&self) -> GetSourceContentHandler {
        GetSourceContentHandler::new(self.sources.clone(), self.snapshots.clone())
    }

    fn update_source(&self) -> UpdateSourceHandler {
        UpdateSourceHandler::new(self.sources.clone(), self.clock.clone())
    }

    fn delete_source(&self) -> DeleteSourceHandler {
        DeleteSourceHandler::new(self.sources.clone())
    }

    fn create_publication(&self) -> CreatePublicationHandler {
        CreatePublicationHandler::new(
            self.sources.clone(),
            self.publications.clone(),
            self.secrets.clone(),
            self.clock.clone(),
        )
    }

    fn list_publications(&self) -> ListPublicationsHandler {
        ListPublicationsHandler::new(self.publications.clone())
    }

    fn get_publication(&self) -> GetPublicationHandler {
        GetPublicationHandler::new(self.publications.clone())
    }

    fn get_publication_content(&self) -> GetPublicationContentHandler {
        GetPublicationContentHandler::new(
            self.publications.clone(),
            self.sources.clone(),
            self.snapshots.clone(),
        )
    }

    fn update_publication(&self) -> UpdatePublicationHandler {
        UpdatePublicationHandler::new(self.publications.clone(), self.clock.clone())
    }

    fn delete_publication(&self) -> DeletePublicationHandler {
        DeletePublicationHandler::new(self.publications.clone())
    }

    fn set_publication_sources(&self) -> SetPublicationSourcesHandler {
        SetPublicationSourcesHandler::new(
            self.sources.clone(),
            self.publications.clone(),
            self.clock.clone(),
        )
    }
}

fn create_source(fixture: &Fixture, user_id: &UserId, name: &str, url: &str) -> SourceView {
    block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id: user_id.clone(),
        name: name.to_string(),
        url: url.to_string(),
        enabled: None,
    }))
    .unwrap()
}

fn create_publication(
    fixture: &Fixture,
    user_id: &UserId,
    name: &str,
    source_ids: Vec<String>,
) -> PublicationView {
    block_on(
        fixture
            .create_publication()
            .handle(CreatePublicationCommand {
                user_id: user_id.clone(),
                name: name.to_string(),
                source_ids,
                expires_at: None,
            }),
    )
    .unwrap()
}

#[test]
fn create_source_defaults_to_enabled() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let view = create_source(&fixture, &user_id, " 机场 A ", "https://example.com/sub");

    assert_eq!(view.name, "机场 A");
    assert_eq!(view.url, "https://example.com/sub");
    assert!(view.enabled);
    assert_eq!(view.created_at, 1_000);
    assert_eq!(view.updated_at, 1_000);
}

#[test]
fn create_source_can_start_disabled() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let view = block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id,
        name: "机场".to_string(),
        url: "https://example.com/sub".to_string(),
        enabled: Some(false),
    }))
    .unwrap();

    assert!(!view.enabled);
}

#[test]
fn create_source_rejects_invalid_input() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let invalid_name = block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id: user_id.clone(),
        name: "   ".to_string(),
        url: "https://example.com/sub".to_string(),
        enabled: None,
    }));
    assert!(matches!(invalid_name, Err(AppError::InvalidName)));

    let invalid_url = block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id,
        name: "机场".to_string(),
        url: "ftp://example.com".to_string(),
        enabled: None,
    }));
    assert!(matches!(invalid_url, Err(AppError::InvalidSourceUrl)));
}

#[test]
fn duplicate_source_url_is_rejected_per_user() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();

    create_source(&fixture, &alice, "A", "https://example.com/sub");

    let duplicate = block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id: alice.clone(),
        name: "A2".to_string(),
        url: "https://example.com/sub".to_string(),
        enabled: None,
    }));
    assert!(matches!(duplicate, Err(AppError::SourceUrlTaken)));

    create_source(&fixture, &bob, "B", "https://example.com/sub");
}

#[test]
fn list_sources_is_scoped_to_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();

    create_source(&fixture, &alice, "A", "https://a.example.com");
    create_source(&fixture, &bob, "B", "https://b.example.com");

    let alice_sources = block_on(fixture.list_sources().handle(&alice)).unwrap();
    assert_eq!(alice_sources.len(), 1);
    assert_eq!(alice_sources[0].url, "https://a.example.com");
}

#[test]
fn update_source_applies_partial_changes() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let created = create_source(&fixture, &user_id, "A", "https://example.com/sub");

    fixture.clock.set(1_100);
    let updated = block_on(fixture.update_source().handle(UpdateSourceCommand {
        user_id: user_id.clone(),
        source_id: created.id.clone(),
        name: Some("B".to_string()),
        url: None,
        enabled: Some(false),
    }))
    .unwrap();

    assert_eq!(updated.name, "B");
    assert_eq!(updated.url, "https://example.com/sub");
    assert!(!updated.enabled);
    assert_eq!(updated.updated_at, 1_100);
}

#[test]
fn update_source_rejects_conflicting_url() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com");

    let result = block_on(fixture.update_source().handle(UpdateSourceCommand {
        user_id: user_id.clone(),
        source_id: second.id.clone(),
        name: None,
        url: Some(first.url.clone()),
        enabled: None,
    }));

    assert!(matches!(result, Err(AppError::SourceUrlTaken)));
}

#[test]
fn source_operations_are_scoped_to_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();
    let created = create_source(&fixture, &alice, "A", "https://example.com/sub");

    let other_get = block_on(fixture.get_source().handle(GetSourceCommand {
        user_id: bob.clone(),
        source_id: created.id.clone(),
    }));
    assert!(matches!(other_get, Err(AppError::NotFound)));

    let other_delete = block_on(fixture.delete_source().handle(DeleteSourceCommand {
        user_id: bob.clone(),
        source_id: created.id.clone(),
    }));
    assert!(matches!(other_delete, Err(AppError::NotFound)));

    let owner_get = block_on(fixture.get_source().handle(GetSourceCommand {
        user_id: alice,
        source_id: created.id.clone(),
    }));
    assert!(owner_get.is_ok());
}

#[test]
fn delete_source_removes_it() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let created = create_source(&fixture, &user_id, "A", "https://example.com/sub");

    block_on(fixture.delete_source().handle(DeleteSourceCommand {
        user_id: user_id.clone(),
        source_id: created.id.clone(),
    }))
    .unwrap();

    let result = block_on(fixture.get_source().handle(GetSourceCommand {
        user_id,
        source_id: created.id,
    }));
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[test]
fn deleting_source_unbinds_it_from_publications() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://example.com/sub");
    let publication = create_publication(&fixture, &user_id, "我的订阅", vec![source.id.clone()]);
    assert_eq!(publication.source_ids, vec![source.id.clone()]);

    block_on(fixture.delete_source().handle(DeleteSourceCommand {
        user_id: user_id.clone(),
        source_id: source.id.clone(),
    }))
    .unwrap();

    let view = block_on(fixture.get_publication().handle(GetPublicationCommand {
        user_id,
        publication_id: publication.id,
    }))
    .unwrap();
    assert!(view.source_ids.is_empty());
}

#[test]
fn create_publication_generates_secret_and_orders_sources() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com");

    let view = create_publication(
        &fixture,
        &user_id,
        "我的订阅",
        vec![second.id.clone(), first.id.clone()],
    );

    assert!(view.enabled);
    assert_eq!(view.source_ids, vec![second.id.clone(), first.id.clone()]);
    assert!(!view.secret.is_empty());
    assert_eq!(view.created_at, 1_000);
}

#[test]
fn create_publication_generates_unique_secrets() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let first = create_publication(&fixture, &user_id, "订阅 1", Vec::new());
    let second = create_publication(&fixture, &user_id, "订阅 2", Vec::new());

    assert_ne!(first.secret, second.secret);
}

#[test]
fn create_publication_rejects_unknown_source() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let result = block_on(
        fixture
            .create_publication()
            .handle(CreatePublicationCommand {
                user_id,
                name: "我的订阅".to_string(),
                source_ids: vec![SourceId::new().to_string()],
                expires_at: None,
            }),
    );

    assert!(matches!(result, Err(AppError::UnknownSource)));
}

#[test]
fn create_publication_rejects_duplicate_sources() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com");

    let result = block_on(
        fixture
            .create_publication()
            .handle(CreatePublicationCommand {
                user_id,
                name: "我的订阅".to_string(),
                source_ids: vec![source.id.clone(), source.id.clone()],
                expires_at: None,
            }),
    );

    assert!(matches!(result, Err(AppError::DuplicateSource)));
}

#[test]
fn create_publication_accepts_expiry() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();

    let view = block_on(
        fixture
            .create_publication()
            .handle(CreatePublicationCommand {
                user_id,
                name: "我的订阅".to_string(),
                source_ids: Vec::new(),
                expires_at: Some(2_000),
            }),
    )
    .unwrap();

    assert_eq!(view.expires_at, Some(2_000));
}

#[test]
fn set_publication_sources_replaces_and_orders_bindings() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com");
    let publication = create_publication(&fixture, &user_id, "我的订阅", vec![first.id.clone()]);

    fixture.clock.set(1_100);
    let view = block_on(
        fixture
            .set_publication_sources()
            .handle(SetPublicationSourcesCommand {
                user_id: user_id.clone(),
                publication_id: publication.id.clone(),
                source_ids: vec![second.id.clone(), first.id.clone()],
            }),
    )
    .unwrap();

    assert_eq!(view.source_ids, vec![second.id.clone(), first.id.clone()]);
    assert_eq!(view.updated_at, 1_100);

    let unknown = block_on(fixture.set_publication_sources().handle(
        SetPublicationSourcesCommand {
            user_id: user_id.clone(),
            publication_id: publication.id.clone(),
            source_ids: vec![SourceId::new().to_string()],
        },
    ));
    assert!(matches!(unknown, Err(AppError::UnknownSource)));

    let duplicate = block_on(fixture.set_publication_sources().handle(
        SetPublicationSourcesCommand {
            user_id,
            publication_id: publication.id,
            source_ids: vec![first.id.clone(), first.id.clone()],
        },
    ));
    assert!(matches!(duplicate, Err(AppError::DuplicateSource)));
}

#[test]
fn update_publication_distinguishes_absent_and_cleared_expiry() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let publication = create_publication(&fixture, &user_id, "我的订阅", Vec::new());

    let set = block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id: user_id.clone(),
                publication_id: publication.id.clone(),
                name: None,
                enabled: None,
                expires_at: Some(Some(2_000)),
            }),
    )
    .unwrap();
    assert_eq!(set.expires_at, Some(2_000));

    let absent = block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id: user_id.clone(),
                publication_id: publication.id.clone(),
                name: None,
                enabled: None,
                expires_at: None,
            }),
    )
    .unwrap();
    assert_eq!(absent.expires_at, Some(2_000));

    let cleared = block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id,
                publication_id: publication.id,
                name: None,
                enabled: None,
                expires_at: Some(None),
            }),
    )
    .unwrap();
    assert_eq!(cleared.expires_at, None);
}

#[test]
fn update_publication_renames_and_toggles() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let publication = create_publication(&fixture, &user_id, "旧名字", Vec::new());

    fixture.clock.set(1_100);
    let view = block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id: user_id.clone(),
                publication_id: publication.id.clone(),
                name: Some("新名字".to_string()),
                enabled: Some(false),
                expires_at: None,
            }),
    )
    .unwrap();

    assert_eq!(view.name, "新名字");
    assert!(!view.enabled);
    assert_eq!(view.updated_at, 1_100);
}

#[test]
fn publication_operations_are_scoped_to_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();
    let publication = create_publication(&fixture, &alice, "我的订阅", Vec::new());

    let other_get = block_on(fixture.get_publication().handle(GetPublicationCommand {
        user_id: bob.clone(),
        publication_id: publication.id.clone(),
    }));
    assert!(matches!(other_get, Err(AppError::NotFound)));

    let other_delete = block_on(
        fixture
            .delete_publication()
            .handle(DeletePublicationCommand {
                user_id: bob.clone(),
                publication_id: publication.id.clone(),
            }),
    );
    assert!(matches!(other_delete, Err(AppError::NotFound)));

    let other_list = block_on(fixture.list_publications().handle(&bob)).unwrap();
    assert!(other_list.is_empty());

    let owner_delete = block_on(
        fixture
            .delete_publication()
            .handle(DeletePublicationCommand {
                user_id: alice.clone(),
                publication_id: publication.id.clone(),
            }),
    );
    assert!(owner_delete.is_ok());

    let gone = block_on(fixture.get_publication().handle(GetPublicationCommand {
        user_id: alice,
        publication_id: publication.id,
    }));
    assert!(matches!(gone, Err(AppError::NotFound)));
}

const SNAPSHOT_A: &str = r#"
proxies:
  - name: A 节点
    type: vmess
    server: a.example.com
    port: 443
    uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
  - name: A SS
    type: ss
    server: ss.example.com
    port: 8388
    cipher: aes-128-gcm
    password: pass
"#;

const SNAPSHOT_B: &str = r#"
proxies:
  - name: B 节点
    type: hysteria2
    server: b.example.com
    port: 443
    password: b-secret
"#;

const SNAPSHOT_C: &str = r#"
mixed-port: 7890
mode: rule
dns:
  enable: true
proxies:
  - name: C 节点
    type: vmess
    server: c.example.com
    port: 443
    uuid: cccccccc-cccc-cccc-cccc-cccccccccccc
proxy-groups:
  - name: 选择
    type: select
    proxies: [C 节点]
rules:
  - DOMAIN-SUFFIX,example.com,选择
  - MATCH,选择
"#;

fn source_url(value: &str) -> SourceUrl {
    SourceUrl::new(value).unwrap()
}

fn source_id_of(view: &SourceView) -> SourceId {
    SourceId::parse(&view.id).unwrap()
}

fn seed_extraction(fixture: &Fixture, source_id: &SourceId, body: &str) {
    let document = parse_document(body).unwrap();
    block_on(
        fixture
            .snapshots
            .replace_extraction(source_id, &ExtractedConfig::from_value(&document)),
    )
    .unwrap();
}

fn seed_snapshot(fixture: &Fixture, source_id: &str, body: &str) {
    let source_id = SourceId::parse(source_id).unwrap();
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        body.as_bytes().to_vec(),
        None,
        None,
        1_000,
        SnapshotMeta::default(),
    )))
    .unwrap();
    seed_extraction(fixture, &source_id, body);
}

#[allow(clippy::too_many_arguments)]
fn seed_snapshot_with_userinfo(
    fixture: &Fixture,
    source_id: &str,
    body: &str,
    upload: i64,
    download: i64,
    total: i64,
    expire: Option<i64>,
) {
    let source_id = SourceId::parse(source_id).unwrap();
    let meta = SnapshotMeta {
        userinfo: SubscriptionUserInfo {
            upload: Some(upload),
            download: Some(download),
            total: Some(total),
            expire,
        },
        ..SnapshotMeta::default()
    };
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        body.as_bytes().to_vec(),
        None,
        None,
        1_000,
        meta,
    )))
    .unwrap();
    seed_extraction(fixture, &source_id, body);
}

fn fetched_document(body: &str) -> FetchedDocument {
    FetchedDocument {
        body: body.as_bytes().to_vec(),
        etag: None,
        last_modified: None,
        subscription_userinfo: None,
        profile_update_interval: None,
        profile_web_page_url: None,
        content_disposition: None,
    }
}

fn fetched(body: &str) -> Result<FetchOutcome, FetchError> {
    Ok(FetchOutcome::Fetched(fetched_document(body)))
}

fn serve_command(secret: &str) -> ServePublicationCommand {
    ServePublicationCommand {
        secret: secret.to_string(),
        format: SubscriptionFormat::Clash,
    }
}

#[test]
fn serve_publication_returns_empty_config_on_cold_start() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);
    fixture
        .fetcher
        .stub("https://a.example.com/sub", fetched(SNAPSHOT_A));

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert!(served.content.contains("proxies: []"));
    assert!(served.content.contains("DIRECT"));
    assert_eq!(fixture.background.pending(), 1);
    assert_eq!(fixture.fetcher.calls.load(Ordering::SeqCst), 0);

    fixture.background.run_all();
    assert_eq!(fixture.fetcher.calls.load(Ordering::SeqCst), 1);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert!(served.content.contains("A 节点"));
}

#[test]
fn serve_publication_merges_snapshots_in_binding_order() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com/sub");
    let publication = create_publication(
        &fixture,
        &user_id,
        "合并",
        vec![second.id.clone(), first.id.clone()],
    );
    seed_snapshot(&fixture, &first.id, SNAPSHOT_A);
    seed_snapshot(&fixture, &second.id, SNAPSHOT_B);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    let first_index = served.content.find("B 节点").unwrap();
    let second_index = served.content.find("A 节点").unwrap();
    assert!(
        first_index < second_index,
        "binding order must be preserved: {}",
        served.content
    );
}

#[test]
fn serve_publication_skips_disabled_sources() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let enabled = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let disabled = block_on(fixture.create_source().handle(CreateSourceCommand {
        user_id: user_id.clone(),
        name: "B".to_string(),
        url: "https://b.example.com/sub".to_string(),
        enabled: Some(false),
    }))
    .unwrap();
    let publication = create_publication(
        &fixture,
        &user_id,
        "合并",
        vec![disabled.id.clone(), enabled.id.clone()],
    );
    seed_snapshot(&fixture, &enabled.id, SNAPSHOT_A);
    seed_snapshot(&fixture, &disabled.id, SNAPSHOT_B);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert!(served.content.contains("A 节点"));
    assert!(!served.content.contains("B 节点"));
    assert_eq!(fixture.background.pending(), 1);
}

#[test]
fn serve_publication_hides_unavailable_publications() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);

    let unknown = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&"x".repeat(32))),
    );
    assert!(matches!(unknown, Err(AppError::NotFound)));

    block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id: user_id.clone(),
                publication_id: publication.id.clone(),
                name: None,
                enabled: Some(false),
                expires_at: None,
            }),
    )
    .unwrap();
    let disabled = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    );
    assert!(matches!(disabled, Err(AppError::NotFound)));

    block_on(
        fixture
            .update_publication()
            .handle(UpdatePublicationCommand {
                user_id,
                publication_id: publication.id.clone(),
                name: None,
                enabled: Some(true),
                expires_at: Some(Some(500)),
            }),
    )
    .unwrap();
    let expired = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    );
    assert!(matches!(expired, Err(AppError::NotFound)));
}

#[test]
fn refresh_source_skips_when_lease_is_held() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);

    assert!(block_on(
        fixture
            .snapshots
            .try_acquire_lease(&source_id, 1_000, 1_120)
    )
    .unwrap());

    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::InProgress);
    assert_eq!(fixture.fetcher.calls.load(Ordering::SeqCst), 0);

    fixture.clock.set(1_121);
    fixture.fetcher.stub(&source.url, fetched(SNAPSHOT_A));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Refreshed);
}

#[test]
fn refresh_source_failure_keeps_previous_snapshot() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);
    fixture
        .fetcher
        .stub(&source.url, Err(FetchError::Status(500)));

    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Failed);

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.body(), SNAPSHOT_A.as_bytes());
    assert!(fixture.snapshots.last_error(&source_id).is_some());
}

#[test]
fn refresh_source_conditional_hit_updates_timestamp() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        SNAPSHOT_A.as_bytes().to_vec(),
        Some("etag-a".to_string()),
        Some("lm-a".to_string()),
        1_000,
        SnapshotMeta::default(),
    )))
    .unwrap();

    fixture.clock.set(2_000);
    fixture
        .fetcher
        .stub(&source.url, Ok(FetchOutcome::NotModified));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::NotModified);

    let validators = fixture
        .fetcher
        .last_validators
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    assert_eq!(validators.etag.as_deref(), Some("etag-a"));
    assert_eq!(validators.last_modified.as_deref(), Some("lm-a"));

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.body(), SNAPSHOT_A.as_bytes());
    assert_eq!(stored.fetched_at(), 2_000);
}

#[test]
fn refresh_source_rejects_body_without_supported_proxies() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);
    fixture
        .fetcher
        .stub(&source.url, fetched("<html>not a clash config</html>"));

    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::NoData);

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.body(), SNAPSHOT_A.as_bytes());
    assert!(fixture.snapshots.last_error(&source_id).is_some());
}

#[test]
fn refresh_source_persists_extraction_and_metadata() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);

    let mut document = fetched_document(SNAPSHOT_A);
    document.subscription_userinfo =
        Some("upload=1; download=2; total=3; expire=1790000000".to_string());
    document.profile_update_interval = Some("24".to_string());
    document.profile_web_page_url = Some("https://provider.example.com".to_string());
    document.content_disposition =
        Some("attachment;filename*=UTF-8''%E6%9C%BA%E5%9C%BA".to_string());
    fixture
        .fetcher
        .stub(&source.url, Ok(FetchOutcome::Fetched(document)));

    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Refreshed);
    assert_eq!(fixture.snapshots.replace_calls(), 1);

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    let meta = stored.meta();
    assert_eq!(meta.proxy_count, 2);
    assert_eq!(meta.group_count, 0);
    assert_eq!(meta.protocol_counts.get("vmess"), Some(&1));
    assert_eq!(meta.protocol_counts.get("ss"), Some(&1));
    assert_eq!(meta.userinfo.upload, Some(1));
    assert_eq!(meta.userinfo.download, Some(2));
    assert_eq!(meta.userinfo.total, Some(3));
    assert_eq!(meta.userinfo.expire, Some(1_790_000_000));
    assert_eq!(meta.update_interval, Some(24));
    assert_eq!(meta.provider_name.as_deref(), Some("机场"));
    assert_eq!(
        meta.provider_url.as_deref(),
        Some("https://provider.example.com")
    );
    assert_eq!(
        meta.body_hash.as_deref(),
        Some(body_hash(SNAPSHOT_A.as_bytes()).as_str())
    );

    let extraction = fixture.snapshots.extraction(&source_id).unwrap();
    assert_eq!(extraction.proxy_count(), 2);
    assert_eq!(extraction.proxies[1].protocol.as_deref(), Some("ss"));
}

#[test]
fn refresh_source_skips_extraction_when_body_unchanged() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        SNAPSHOT_A.as_bytes().to_vec(),
        None,
        None,
        1_000,
        SnapshotMeta {
            body_hash: Some(body_hash(SNAPSHOT_A.as_bytes())),
            ..SnapshotMeta::default()
        },
    )))
    .unwrap();
    seed_extraction(&fixture, &source_id, SNAPSHOT_A);
    let replace_calls = fixture.snapshots.replace_calls();

    fixture.clock.set(2_000);
    fixture.fetcher.stub(&source.url, fetched(SNAPSHOT_A));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Refreshed);
    assert_eq!(fixture.snapshots.replace_calls(), replace_calls);

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.fetched_at(), 2_000);
}

#[test]
fn refresh_source_reextracts_when_extraction_missing() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        SNAPSHOT_A.as_bytes().to_vec(),
        None,
        None,
        1_000,
        SnapshotMeta {
            body_hash: Some(body_hash(SNAPSHOT_A.as_bytes())),
            ..SnapshotMeta::default()
        },
    )))
    .unwrap();

    fixture.clock.set(2_000);
    fixture.fetcher.stub(&source.url, fetched(SNAPSHOT_A));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Refreshed);
    assert_eq!(fixture.snapshots.replace_calls(), 1);

    let extraction = fixture.snapshots.extraction(&source_id).unwrap();
    assert_eq!(extraction.proxy_count(), 2);
}

#[test]
fn serve_publication_keeps_all_protocols() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert!(served.content.contains("A 节点"));
    assert!(served.content.contains("A SS"));
    assert!(served.content.contains("type: ss"));
}

#[test]
fn serve_publication_keeps_rules_groups_and_settings() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);
    seed_snapshot(&fixture, &source.id, SNAPSHOT_C);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    let document: serde_yaml::Value = serde_yaml::from_str(&served.content).unwrap();
    let root = document.as_mapping().unwrap();
    assert_eq!(root.get("mixed-port").unwrap().as_u64(), Some(7890));
    assert_eq!(root.get("mode").unwrap().as_str(), Some("rule"));
    assert_eq!(
        root.get("dns")
            .and_then(|dns| dns.as_mapping())
            .and_then(|dns| dns.get("enable"))
            .and_then(serde_yaml::Value::as_bool),
        Some(true)
    );
    let group_names: Vec<&str> = root
        .get("proxy-groups")
        .unwrap()
        .as_sequence()
        .unwrap()
        .iter()
        .map(|group| {
            group
                .as_mapping()
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(group_names, vec!["PROXY", "选择"]);
    let rules: Vec<&str> = root
        .get("rules")
        .unwrap()
        .as_sequence()
        .unwrap()
        .iter()
        .map(|rule| rule.as_str().unwrap())
        .collect();
    assert_eq!(rules, vec!["DOMAIN-SUFFIX,example.com,选择", "MATCH,PROXY"]);
}

#[test]
fn list_sources_includes_snapshot_summary() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);

    let mut document = fetched_document(SNAPSHOT_A);
    document.subscription_userinfo = Some("upload=10; download=20; total=30".to_string());
    fixture
        .fetcher
        .stub(&source.url, Ok(FetchOutcome::Fetched(document)));
    block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();

    let list = block_on(fixture.list_sources().handle(&user_id)).unwrap();
    let snapshot = list[0].snapshot.as_ref().expect("snapshot summary");
    assert_eq!(snapshot.fetched_at, Some(1_000));
    assert_eq!(snapshot.proxy_count, 2);
    assert_eq!(snapshot.upload, Some(10));
    assert_eq!(snapshot.download, Some(20));
    assert_eq!(snapshot.total, Some(30));
    assert_eq!(snapshot.protocol_counts.len(), 2);

    let fetched = block_on(fixture.get_source().handle(GetSourceCommand {
        user_id,
        source_id: source.id.clone(),
    }))
    .unwrap();
    assert!(fetched.snapshot.is_some());
}

#[test]
fn refresh_source_backfills_extraction_on_not_modified() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        SNAPSHOT_A.as_bytes().to_vec(),
        Some("etag-a".to_string()),
        None,
        1_000,
        SnapshotMeta::default(),
    )))
    .unwrap();

    fixture.clock.set(2_000);
    fixture
        .fetcher
        .stub(&source.url, Ok(FetchOutcome::NotModified));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::NotModified);
    assert_eq!(fixture.snapshots.replace_calls(), 1);

    let stored = block_on(fixture.snapshots.find_by_source(&source_id))
        .unwrap()
        .unwrap();
    assert!(stored.meta().body_hash.is_some());
    assert_eq!(stored.meta().proxy_count, 2);
    assert_eq!(stored.fetched_at(), 2_000);

    let extraction = fixture.snapshots.extraction(&source_id).unwrap();
    assert_eq!(extraction.proxy_count(), 2);
}

#[test]
fn refresh_source_backfills_missing_extraction_on_not_modified() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    block_on(fixture.snapshots.save(&SourceSnapshot::restore(
        source_id.clone(),
        SNAPSHOT_A.as_bytes().to_vec(),
        Some("etag-a".to_string()),
        None,
        1_000,
        SnapshotMeta {
            body_hash: Some(body_hash(SNAPSHOT_A.as_bytes())),
            ..SnapshotMeta::default()
        },
    )))
    .unwrap();
    assert!(!block_on(fixture.snapshots.has_extraction(&source_id)).unwrap());

    fixture.clock.set(2_000);
    fixture
        .fetcher
        .stub(&source.url, Ok(FetchOutcome::NotModified));
    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::NotModified);
    assert_eq!(fixture.snapshots.replace_calls(), 1);
    assert!(block_on(fixture.snapshots.has_extraction(&source_id)).unwrap());

    let extraction = fixture.snapshots.extraction(&source_id).unwrap();
    assert_eq!(extraction.proxy_count(), 2);
}

#[test]
fn serve_publication_aggregates_userinfo() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com/sub");
    let publication = create_publication(
        &fixture,
        &user_id,
        "合并",
        vec![first.id.clone(), second.id.clone()],
    );
    seed_snapshot_with_userinfo(&fixture, &first.id, SNAPSHOT_A, 10, 20, 100, Some(2_000));
    seed_snapshot_with_userinfo(&fixture, &second.id, SNAPSHOT_B, 1, 2, 50, Some(1_000));

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert_eq!(served.userinfo.upload, Some(11));
    assert_eq!(served.userinfo.download, Some(22));
    assert_eq!(served.userinfo.total, Some(150));
    assert_eq!(served.userinfo.expire, Some(1_000));
}

#[test]
fn serve_publication_reads_cached_snapshot() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);
    let publication_id = PublicationId::parse(&publication.id).unwrap();
    block_on(
        fixture
            .publication_snapshots
            .save(&PublicationSnapshot::restore(
                publication_id.clone(),
                "clash".to_string(),
                "cached-content".to_string(),
                SubscriptionUserInfo {
                    upload: Some(7),
                    ..SubscriptionUserInfo::default()
                },
                1_000,
            )),
    )
    .unwrap();
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);

    let served = block_on(
        fixture
            .serve_publication()
            .handle(serve_command(&publication.secret)),
    )
    .unwrap();
    assert_eq!(served.content, "cached-content");
    assert_eq!(served.userinfo.upload, Some(7));
    assert_eq!(fixture.background.pending(), 1);
}

#[test]
fn refresh_source_rebuilds_publication_snapshot() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &user_id, "合并", vec![source.id.clone()]);
    let publication_id = PublicationId::parse(&publication.id).unwrap();
    let source_id = source_id_of(&source);
    fixture.fetcher.stub(&source.url, fetched(SNAPSHOT_A));

    block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();

    let cached = fixture
        .publication_snapshots
        .get(&publication_id, "clash")
        .expect("publication snapshot rebuilt after refresh");
    assert!(cached.content().contains("A 节点"));
    assert!(cached.content().contains("A SS"));
}

#[test]
fn rebuild_publication_snapshot_uses_current_sources() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com/sub");
    let publication = create_publication(
        &fixture,
        &user_id,
        "合并",
        vec![second.id.clone(), first.id.clone()],
    );
    let publication_id = PublicationId::parse(&publication.id).unwrap();
    seed_snapshot(&fixture, &first.id, SNAPSHOT_A);
    seed_snapshot(&fixture, &second.id, SNAPSHOT_B);

    let handler = fixture.rebuild_publication();
    let stored = block_on(fixture.publications.find_by_id(&user_id, &publication_id))
        .unwrap()
        .unwrap();
    block_on(handler.handle(&stored)).unwrap();

    let cached = fixture
        .publication_snapshots
        .get(&publication_id, "clash")
        .unwrap();
    let b_index = cached.content().find("B 节点").unwrap();
    let a_index = cached.content().find("A 节点").unwrap();
    assert!(b_index < a_index);
}

#[test]
fn get_source_content_returns_raw_snapshot_body() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);

    let content = block_on(
        fixture
            .get_source_content()
            .handle(GetSourceContentCommand {
                user_id,
                source_id: source.id,
            }),
    )
    .unwrap();
    assert_eq!(content, SNAPSHOT_A);
}

#[test]
fn get_source_content_requires_snapshot() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");

    let result = block_on(
        fixture
            .get_source_content()
            .handle(GetSourceContentCommand {
                user_id,
                source_id: source.id,
            }),
    );
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[test]
fn get_source_content_is_scoped_to_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();
    let source = create_source(&fixture, &alice, "A", "https://a.example.com/sub");
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);

    let result = block_on(
        fixture
            .get_source_content()
            .handle(GetSourceContentCommand {
                user_id: bob,
                source_id: source.id,
            }),
    );
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[test]
fn get_publication_content_merges_bound_sources() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let first = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let second = create_source(&fixture, &user_id, "B", "https://b.example.com/sub");
    let publication = create_publication(
        &fixture,
        &user_id,
        "合并",
        vec![second.id.clone(), first.id.clone()],
    );
    seed_snapshot(&fixture, &first.id, SNAPSHOT_A);
    seed_snapshot(&fixture, &second.id, SNAPSHOT_B);

    let content = block_on(fixture.get_publication_content().handle(
        GetPublicationContentCommand {
            user_id,
            publication_id: publication.id,
        },
    ))
    .unwrap();
    let b_index = content.find("B 节点").unwrap();
    let a_index = content.find("A 节点").unwrap();
    assert!(
        b_index < a_index,
        "binding order must be preserved: {content}"
    );
}

#[test]
fn get_publication_content_is_scoped_to_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();
    let source = create_source(&fixture, &alice, "A", "https://a.example.com/sub");
    let publication = create_publication(&fixture, &alice, "合并", vec![source.id.clone()]);
    seed_snapshot(&fixture, &source.id, SNAPSHOT_A);

    let result = block_on(
        fixture
            .get_publication_content()
            .handle(GetPublicationContentCommand {
                user_id: bob,
                publication_id: publication.id,
            }),
    );
    assert!(matches!(result, Err(AppError::NotFound)));
}

const SNAPSHOT_WITH_PROVIDER: &str = r#"
proxies:
  - name: A 节点
    type: vmess
    server: a.example.com
    port: 443
    uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
rule-providers:
  reject:
    type: http
    behavior: domain
    url: https://cdn.example.com/reject.yaml
    path: ./ruleset/reject.yaml
    interval: 86400
  local:
    type: file
    behavior: classical
    path: ./ruleset/local.yaml
rules:
  - RULE-SET,reject,REJECT
  - MATCH,DIRECT
"#;

const PROVIDER_BODY: &str = "# ad rules\ngoogle.com\n+.doubleclick.net\n";

#[test]
fn refresh_source_fetches_remote_rule_providers() {
    let fixture = Fixture::new(1_000);
    let user_id = UserId::new();
    let source = create_source(&fixture, &user_id, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    fixture
        .fetcher
        .stub(&source.url, fetched(SNAPSHOT_WITH_PROVIDER));
    fixture.fetcher.stub(
        "https://cdn.example.com/reject.yaml",
        fetched(PROVIDER_BODY),
    );

    let status = block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();
    assert_eq!(status, RefreshStatus::Refreshed);

    let providers = block_on(
        fixture
            .list_source_providers()
            .handle(ListSourceProvidersCommand {
                user_id: user_id.clone(),
                source_id: source.id.clone(),
            }),
    )
    .unwrap();
    assert_eq!(providers.len(), 2);

    let reject = providers.iter().find(|p| p.name == "reject").unwrap();
    assert_eq!(reject.behavior.as_deref(), Some("domain"));
    assert_eq!(
        reject.url.as_deref(),
        Some("https://cdn.example.com/reject.yaml")
    );
    assert_eq!(reject.rule_count, 2);
    assert!(reject.has_snapshot);
    assert!(reject.fetched_at.is_some());

    let local = providers.iter().find(|p| p.name == "local").unwrap();
    assert!(!local.has_snapshot);
    assert_eq!(local.rule_count, 0);
    assert_eq!(local.url, None);

    let content = block_on(fixture.get_source_provider_content().handle(
        GetSourceProviderContentCommand {
            user_id,
            source_id: source.id.clone(),
            name: "reject".to_string(),
        },
    ))
    .unwrap();
    assert_eq!(content, PROVIDER_BODY);
}

#[test]
fn get_source_provider_content_requires_snapshot_and_owner() {
    let fixture = Fixture::new(1_000);
    let alice = UserId::new();
    let bob = UserId::new();
    let source = create_source(&fixture, &alice, "A", "https://a.example.com/sub");
    let source_id = source_id_of(&source);
    fixture
        .fetcher
        .stub(&source.url, fetched(SNAPSHOT_WITH_PROVIDER));
    fixture.fetcher.stub(
        "https://cdn.example.com/reject.yaml",
        fetched(PROVIDER_BODY),
    );
    block_on(
        fixture
            .refresh_source()
            .handle(&source_id, &source_url(&source.url)),
    )
    .unwrap();

    let missing = block_on(fixture.get_source_provider_content().handle(
        GetSourceProviderContentCommand {
            user_id: alice.clone(),
            source_id: source.id.clone(),
            name: "local".to_string(),
        },
    ));
    assert!(matches!(missing, Err(AppError::NotFound)));

    let other = block_on(fixture.get_source_provider_content().handle(
        GetSourceProviderContentCommand {
            user_id: bob,
            source_id: source.id.clone(),
            name: "reject".to_string(),
        },
    ));
    assert!(matches!(other, Err(AppError::NotFound)));
}
