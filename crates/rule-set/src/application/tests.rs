use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::executor::block_on;
use user::UserId;

use crate::domain::{RepositoryError, RuleSet, RuleSetContent, RuleSetId, RuleSetRepository};

use super::ports::{Clock, FetchError, FetchOutcome, FetchValidators, FetchedDocument, Fetcher};

#[derive(Default)]
struct Store {
    rule_sets: HashMap<String, RuleSet>,
    contents: HashMap<String, RuleSetContent>,
}

#[derive(Default)]
struct InMemoryRuleSets {
    store: Mutex<Store>,
}

impl InMemoryRuleSets {
    fn content(&self, id: &RuleSetId) -> Option<RuleSetContent> {
        self.store
            .lock()
            .unwrap()
            .contents
            .get(&id.to_string())
            .cloned()
    }
}

#[async_trait]
impl RuleSetRepository for InMemoryRuleSets {
    async fn find_by_id(
        &self,
        user_id: &UserId,
        id: &RuleSetId,
    ) -> Result<Option<RuleSet>, RepositoryError> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .rule_sets
            .get(&id.to_string())
            .filter(|rule_set| rule_set.user_id() == user_id)
            .cloned())
    }

    async fn list_by_user(&self, user_id: &UserId) -> Result<Vec<RuleSet>, RepositoryError> {
        let mut list: Vec<RuleSet> = self
            .store
            .lock()
            .unwrap()
            .rule_sets
            .values()
            .filter(|rule_set| rule_set.user_id() == user_id)
            .cloned()
            .collect();
        list.sort_by_key(|rule_set| rule_set.name().value().to_string());
        Ok(list)
    }

    async fn save(&self, rule_set: &RuleSet) -> Result<(), RepositoryError> {
        let mut store = self.store.lock().unwrap();
        let conflict = store.rule_sets.values().any(|existing| {
            existing.user_id() == rule_set.user_id()
                && existing.name() == rule_set.name()
                && existing.id() != rule_set.id()
        });
        if conflict {
            return Err(RepositoryError::NameConflict);
        }
        store
            .rule_sets
            .insert(rule_set.id().to_string(), rule_set.clone());
        Ok(())
    }

    async fn update(&self, rule_set: &RuleSet) -> Result<(), RepositoryError> {
        self.save(rule_set).await
    }

    async fn delete(&self, _user_id: &UserId, id: &RuleSetId) -> Result<(), RepositoryError> {
        self.store.lock().unwrap().rule_sets.remove(&id.to_string());
        Ok(())
    }

    async fn find_content(
        &self,
        id: &RuleSetId,
    ) -> Result<Option<RuleSetContent>, RepositoryError> {
        Ok(self.content(id))
    }

    async fn save_content(&self, content: &RuleSetContent) -> Result<(), RepositoryError> {
        self.store
            .lock()
            .unwrap()
            .contents
            .insert(content.rule_set_id().to_string(), content.clone());
        Ok(())
    }

    async fn delete_content(&self, id: &RuleSetId) -> Result<(), RepositoryError> {
        self.store
            .lock()
            .unwrap()
            .contents
            .remove(&id.to_string());
        Ok(())
    }
}

struct FixedClock(i64);

impl Clock for FixedClock {
    fn now(&self) -> i64 {
        self.0
    }
}

#[derive(Default)]
struct FakeFetcher {
    responses: Mutex<VecDeque<Result<FetchOutcome, FetchError>>>,
}

impl FakeFetcher {
    fn with(responses: Vec<Result<FetchOutcome, FetchError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

#[async_trait]
impl Fetcher for FakeFetcher {
    async fn fetch(
        &self,
        _url: &crate::domain::RuleSetUrl,
        _validators: &FetchValidators,
    ) -> Result<FetchOutcome, FetchError> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(FetchError::Failure("no scripted response".to_string())))
    }
}

fn fetched(body: &str) -> Result<FetchOutcome, FetchError> {
    Ok(FetchOutcome::Fetched(FetchedDocument {
        body: body.as_bytes().to_vec(),
        etag: Some("\"v1\"".to_string()),
        last_modified: None,
    }))
}

fn remote_command(user_id: UserId, name: &str) -> super::CreateRuleSetCommand {
    super::CreateRuleSetCommand {
        user_id,
        name: name.to_string(),
        source_kind: "remote".to_string(),
        url: Some("https://example.com/reject.yaml".to_string()),
        path: None,
        category: Some("domain".to_string()),
        content_format: "yaml".to_string(),
        interval: Some(86_400),
        content: None,
    }
}

fn inline_command(user_id: UserId, name: &str, content: &str) -> super::CreateRuleSetCommand {
    super::CreateRuleSetCommand {
        user_id,
        name: name.to_string(),
        source_kind: "inline".to_string(),
        url: None,
        path: None,
        category: None,
        content_format: "text".to_string(),
        interval: None,
        content: Some(content.to_string()),
    }
}

#[test]
fn create_remote_fetches_and_stores_content() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let fetcher = Arc::new(FakeFetcher::with(vec![fetched(
            "payload:\n  - a.com\n  - b.com\n",
        )]));
        let handler =
            super::CreateRuleSetHandler::new(repo, fetcher, Arc::new(FixedClock(100)));

        let view = handler
            .handle(remote_command(UserId::new(), "reject"))
            .await
            .unwrap();

        assert_eq!(view.category.as_deref(), Some("domain"));
        assert_eq!(view.content_format, "yaml");
        let summary = view.content.unwrap();
        assert!(summary.has_content);
        assert_eq!(summary.rule_count, 2);
        assert!(!summary.pinned);
    });
}

#[test]
fn create_remote_records_error_without_failing() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let fetcher = Arc::new(FakeFetcher::with(vec![Err(FetchError::Status(500))]));
        let handler = super::CreateRuleSetHandler::new(repo, fetcher, Arc::new(FixedClock(100)));

        let view = handler
            .handle(remote_command(UserId::new(), "reject"))
            .await
            .unwrap();

        assert!(!view.content.unwrap().has_content);
        assert_eq!(view.name, "reject");
    });
}

#[test]
fn create_inline_stores_pinned_content() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let handler = super::CreateRuleSetHandler::new(
            repo,
            Arc::new(FakeFetcher::default()),
            Arc::new(FixedClock(100)),
        );

        let view = handler
            .handle(inline_command(UserId::new(), "my-ads", "a.com\nb.com\n"))
            .await
            .unwrap();

        let summary = view.content.unwrap();
        assert!(summary.pinned);
        assert_eq!(summary.rule_count, 2);
        assert!(view.url.is_none());
    });
}

#[test]
fn create_local_has_no_content() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let handler = super::CreateRuleSetHandler::new(
            repo,
            Arc::new(FakeFetcher::default()),
            Arc::new(FixedClock(100)),
        );

        let view = handler
            .handle(super::CreateRuleSetCommand {
                user_id: UserId::new(),
                name: "local".to_string(),
                source_kind: "local".to_string(),
                url: None,
                path: Some("./ruleset/local.yaml".to_string()),
                category: None,
                content_format: "yaml".to_string(),
                interval: None,
                content: None,
            })
            .await
            .unwrap();

        assert_eq!(view.source_kind, "local");
        assert_eq!(view.path.as_deref(), Some("./ruleset/local.yaml"));
        assert!(view.content.is_none());
    });
}

#[test]
fn create_rejects_duplicate_name() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let handler = super::CreateRuleSetHandler::new(
            repo,
            Arc::new(FakeFetcher::with(vec![
                fetched("payload: []"),
                fetched("payload: []"),
            ])),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        handler
            .handle(remote_command(user_id.clone(), "reject"))
            .await
            .unwrap();

        let err = handler
            .handle(remote_command(user_id, "reject"))
            .await
            .unwrap_err();
        assert!(matches!(err, super::AppError::NameTaken));
    });
}

#[test]
fn refresh_not_modified_keeps_body() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let fetcher = Arc::new(FakeFetcher::with(vec![
            fetched("payload:\n  - a.com\n"),
            Ok(FetchOutcome::NotModified),
        ]));
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            fetcher.clone(),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        let view = create
            .handle(remote_command(user_id.clone(), "reject"))
            .await
            .unwrap();

        let refresh =
            super::RefreshRuleSetHandler::new(repo.clone(), fetcher, Arc::new(FixedClock(200)));
        let status = refresh
            .handle(super::RefreshRuleSetCommand {
                user_id,
                id: view.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(status, super::RefreshStatus::NotModified);

        let content = repo
            .content(&RuleSetId::parse(&view.id).unwrap())
            .unwrap();
        assert_eq!(content.body(), Some(&b"payload:\n  - a.com\n"[..]));
        assert_eq!(content.updated_at(), 200);
    });
}

#[test]
fn refresh_skips_pinned_and_non_remote() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::with(vec![fetched("payload: []")])),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        let view = create
            .handle(remote_command(user_id.clone(), "reject"))
            .await
            .unwrap();

        super::SetRuleSetPinnedHandler::new(repo.clone())
            .handle(super::SetRuleSetPinnedCommand {
                user_id: user_id.clone(),
                id: view.id.clone(),
                pinned: true,
            })
            .await
            .unwrap();

        let refresh = super::RefreshRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::with(vec![fetched("payload:\n  - x.com\n")])),
            Arc::new(FixedClock(200)),
        );
        let status = refresh
            .handle(super::RefreshRuleSetCommand {
                user_id: user_id.clone(),
                id: view.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(status, super::RefreshStatus::Skipped);

        let inline = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::default()),
            Arc::new(FixedClock(100)),
        )
        .handle(inline_command(user_id.clone(), "inline", "a.com\n"))
        .await
        .unwrap();
        let status = refresh
            .handle(super::RefreshRuleSetCommand {
                user_id,
                id: inline.id,
            })
            .await
            .unwrap();
        assert_eq!(status, super::RefreshStatus::Skipped);
    });
}

#[test]
fn replace_content_sets_pinned_and_recounts() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::with(vec![fetched("payload: []")])),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        let view = create
            .handle(remote_command(user_id.clone(), "reject"))
            .await
            .unwrap();

        let replace =
            super::ReplaceRuleSetContentHandler::new(repo.clone(), Arc::new(FixedClock(300)));
        let updated = replace
            .handle(super::ReplaceRuleSetContentCommand {
                user_id,
                id: view.id,
                body: b"a.com\nb.com\nc.com\n".to_vec(),
                pin: true,
            })
            .await
            .unwrap();
        let summary = updated.content.unwrap();
        assert!(summary.pinned);
        assert_eq!(summary.rule_count, 3);
    });
}

#[test]
fn update_clears_category_and_recounts_on_format_change() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::with(vec![fetched(
                "payload:\n  - a.com\n  - b.com\n",
            )])),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        let view = create
            .handle(remote_command(user_id.clone(), "reject"))
            .await
            .unwrap();
        assert_eq!(view.content.unwrap().rule_count, 2);

        let update = super::UpdateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::default()),
            Arc::new(FixedClock(200)),
        );
        let updated = update
            .handle(super::UpdateRuleSetCommand {
                user_id,
                id: view.id,
                name: None,
                url: None,
                path: None,
                category: Some(None),
                content_format: Some("text".to_string()),
                interval: None,
                enabled: None,
            })
            .await
            .unwrap();
        assert!(updated.category.is_none());
        // text 统计时 `payload:` 被跳过，剩两行域名。
        assert_eq!(updated.content.unwrap().rule_count, 2);
    });
}

#[test]
fn local_rejects_content_edits() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::default()),
            Arc::new(FixedClock(100)),
        );
        let user_id = UserId::new();
        let view = create
            .handle(super::CreateRuleSetCommand {
                user_id: user_id.clone(),
                name: "local".to_string(),
                source_kind: "local".to_string(),
                url: None,
                path: Some("./local.yaml".to_string()),
                category: None,
                content_format: "yaml".to_string(),
                interval: None,
                content: None,
            })
            .await
            .unwrap();

        let err = super::ReplaceRuleSetContentHandler::new(repo, Arc::new(FixedClock(100)))
            .handle(super::ReplaceRuleSetContentCommand {
                user_id,
                id: view.id,
                body: b"x".to_vec(),
                pin: true,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, super::AppError::ContentNotEditable));
    });
}

#[test]
fn list_isolates_users_and_delete_removes() {
    block_on(async {
        let repo = Arc::new(InMemoryRuleSets::default());
        let create = super::CreateRuleSetHandler::new(
            repo.clone(),
            Arc::new(FakeFetcher::with(vec![fetched("payload: []")])),
            Arc::new(FixedClock(100)),
        );
        let alice = UserId::new();
        let bob = UserId::new();
        let view = create
            .handle(remote_command(alice.clone(), "reject"))
            .await
            .unwrap();

        let list = super::ListRuleSetsHandler::new(repo.clone());
        assert_eq!(list.handle(alice.clone()).await.unwrap().len(), 1);
        assert!(list.handle(bob.clone()).await.unwrap().is_empty());

        let get = super::GetRuleSetHandler::new(repo.clone());
        assert!(get
            .handle(super::GetRuleSetCommand {
                user_id: bob,
                id: view.id.clone(),
            })
            .await
            .is_err());

        super::DeleteRuleSetHandler::new(repo.clone())
            .handle(super::DeleteRuleSetCommand {
                user_id: alice.clone(),
                id: view.id,
            })
            .await
            .unwrap();
        assert!(list.handle(alice).await.unwrap().is_empty());
    });
}
