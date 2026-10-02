use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::executor::block_on;
use uuid::Uuid;

use crate::application::config::{ACCESS_TOKEN_TTL_SECONDS, REFRESH_TOKEN_TTL_SECONDS};
use crate::application::error::AppError;
use crate::application::ports::{
    AccessToken, Clock, IssuedRefreshToken, PasswordHasher, PortError, RefreshTokenRepository,
    StoredRefreshToken, TokenService,
};
use crate::domain::user::{
    DomainError, PasswordHash, RepositoryError, User, UserId, UserRepository, Username,
};

use super::{
    GetCurrentUserHandler, LoginCommand, LoginHandler, LoginResult, LogoutCommand, LogoutHandler,
    RefreshCommand, RefreshHandler, RegisterCommand, RegisterHandler, UserView,
};

#[derive(Default)]
struct InMemoryUserRepository {
    users: Mutex<HashMap<String, User>>,
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_by_username(
        &self,
        username: &Username,
    ) -> Result<Option<User>, RepositoryError> {
        Ok(self.users.lock().unwrap().get(username.value()).cloned())
    }

    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|user| user.id() == id)
            .cloned())
    }

    async fn save(&self, user: &User) -> Result<(), RepositoryError> {
        let mut users = self.users.lock().unwrap();
        if users.contains_key(user.username().value()) {
            return Err(RepositoryError::UsernameConflict);
        }
        users.insert(user.username().value().to_string(), user.clone());
        Ok(())
    }
}

struct FakePasswordHasher;

#[async_trait]
impl PasswordHasher for FakePasswordHasher {
    async fn hash(&self, password: &str) -> Result<PasswordHash, PortError> {
        PasswordHash::new(format!("hashed:{password}"))
            .map_err(|err| PortError::Failure(err.to_string()))
    }

    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, PortError> {
        Ok(hash.as_str() == format!("hashed:{password}"))
    }
}

struct FakeTokenService {
    counter: AtomicU64,
}

#[async_trait]
impl TokenService for FakeTokenService {
    async fn issue_access_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<AccessToken, PortError> {
        Ok(AccessToken {
            token: format!("access:{user_id}"),
            expires_at: now + ACCESS_TOKEN_TTL_SECONDS,
        })
    }

    async fn verify_access_token(&self, token: &str, _now: i64) -> Result<UserId, PortError> {
        let raw = token.strip_prefix("access:").ok_or(PortError::InvalidToken)?;
        UserId::parse(raw).map_err(|_| PortError::InvalidToken)
    }

    async fn issue_refresh_token(
        &self,
        user_id: &UserId,
        now: i64,
    ) -> Result<IssuedRefreshToken, PortError> {
        let sequence = self.counter.fetch_add(1, Ordering::SeqCst);
        let raw = format!("refresh-{sequence}");
        Ok(IssuedRefreshToken {
            record: StoredRefreshToken {
                id: Uuid::new_v4(),
                user_id: user_id.clone(),
                token_hash: self.hash_refresh_token(&raw),
                created_at: now,
                expires_at: now + REFRESH_TOKEN_TTL_SECONDS,
                revoked_at: None,
            },
            raw,
        })
    }

    fn hash_refresh_token(&self, raw: &str) -> String {
        format!("hashed:{raw}")
    }
}

#[derive(Default)]
struct InMemoryRefreshTokenRepository {
    tokens: Mutex<HashMap<Uuid, StoredRefreshToken>>,
}

#[async_trait]
impl RefreshTokenRepository for InMemoryRefreshTokenRepository {
    async fn save(&self, token: &StoredRefreshToken) -> Result<(), PortError> {
        self.tokens.lock().unwrap().insert(token.id, token.clone());
        Ok(())
    }

    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredRefreshToken>, PortError> {
        Ok(self
            .tokens
            .lock()
            .unwrap()
            .values()
            .find(|token| token.token_hash == token_hash)
            .cloned())
    }

    async fn revoke(&self, id: Uuid, now: i64) -> Result<(), PortError> {
        if let Some(token) = self.tokens.lock().unwrap().get_mut(&id) {
            token.revoked_at = Some(now);
        }
        Ok(())
    }

    async fn revoke_all_for_user(&self, user_id: &UserId, now: i64) -> Result<(), PortError> {
        for token in self.tokens.lock().unwrap().values_mut() {
            if &token.user_id == user_id {
                token.revoked_at = Some(now);
            }
        }
        Ok(())
    }
}

struct FakeClock(AtomicI64);

impl FakeClock {
    fn new(now: i64) -> Self {
        Self(AtomicI64::new(now))
    }

    fn advance_to(&self, now: i64) {
        self.0.store(now, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Fixture {
    users: Arc<InMemoryUserRepository>,
    refresh_tokens: Arc<InMemoryRefreshTokenRepository>,
    clock: Arc<FakeClock>,
    register: RegisterHandler,
    login: LoginHandler,
    refresh: RefreshHandler,
    logout: LogoutHandler,
    current_user: GetCurrentUserHandler,
}

impl Fixture {
    fn new(now: i64) -> Self {
        let users = Arc::new(InMemoryUserRepository::default());
        let refresh_tokens = Arc::new(InMemoryRefreshTokenRepository::default());
        let clock = Arc::new(FakeClock::new(now));
        let hasher = Arc::new(FakePasswordHasher);
        let tokens = Arc::new(FakeTokenService {
            counter: AtomicU64::new(0),
        });

        Self {
            users: users.clone(),
            refresh_tokens: refresh_tokens.clone(),
            clock: clock.clone(),
            register: RegisterHandler::new(users.clone(), hasher.clone(), clock.clone()),
            login: LoginHandler::new(
                users.clone(),
                hasher,
                tokens.clone(),
                refresh_tokens.clone(),
                clock.clone(),
            ),
            refresh: RefreshHandler::new(
                users.clone(),
                tokens.clone(),
                refresh_tokens.clone(),
                clock.clone(),
            ),
            logout: LogoutHandler::new(tokens.clone(), refresh_tokens.clone(), clock.clone()),
            current_user: GetCurrentUserHandler::new(users, tokens, clock),
        }
    }

    async fn register_user(&self, username: &str, password: &str) -> UserView {
        self.register
            .handle(RegisterCommand {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await
            .expect("registration should succeed")
    }

    async fn login_user(&self, username: &str, password: &str) -> LoginResult {
        self.login
            .handle(LoginCommand {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await
            .expect("login should succeed")
    }
}

#[test]
fn register_creates_user_with_normalized_username() {
    block_on(async {
        let fixture = Fixture::new(1_000);

        let view = fixture.register_user(" Alice_01 ", "password123").await;

        assert_eq!(view.username, "alice_01");
        let username = Username::new("ALICE_01").unwrap();
        let stored = fixture
            .users
            .find_by_username(&username)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.id().to_string(), view.id);
        assert_eq!(stored.password_hash().as_str(), "hashed:password123");
        assert_eq!(stored.created_at(), 1_000);
    });
}

#[test]
fn register_rejects_duplicate_username() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;

        let error = fixture
            .register
            .handle(RegisterCommand {
                username: "ALICE".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::UsernameTaken));
    });
}

#[test]
fn register_validates_username_and_password() {
    block_on(async {
        let fixture = Fixture::new(1_000);

        let invalid_username = fixture
            .register
            .handle(RegisterCommand {
                username: "ab".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            invalid_username,
            AppError::Domain(DomainError::InvalidUsername)
        ));

        let invalid_password = fixture
            .register
            .handle(RegisterCommand {
                username: "alice".to_string(),
                password: "short".to_string(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            invalid_password,
            AppError::Domain(DomainError::InvalidPassword)
        ));
    });
}

#[test]
fn login_returns_tokens_for_valid_credentials() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;

        let result = fixture.login_user("Alice", "password123").await;

        assert_eq!(result.user.username, "alice");
        assert_eq!(result.access_token, format!("access:{}", result.user.id));
        assert_eq!(
            result.access_token_expires_at,
            1_000 + ACCESS_TOKEN_TTL_SECONDS
        );
        assert_eq!(result.refresh_token, "refresh-0");
        assert_eq!(
            result.refresh_token_expires_at,
            1_000 + REFRESH_TOKEN_TTL_SECONDS
        );
    });
}

#[test]
fn login_rejects_invalid_credentials() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;

        let unknown_user = fixture
            .login
            .handle(LoginCommand {
                username: "nobody".to_string(),
                password: "password123".to_string(),
            })
            .await
            .unwrap_err();
        assert!(matches!(unknown_user, AppError::InvalidCredentials));

        let wrong_password = fixture
            .login
            .handle(LoginCommand {
                username: "alice".to_string(),
                password: "wrong-password".to_string(),
            })
            .await
            .unwrap_err();
        assert!(matches!(wrong_password, AppError::InvalidCredentials));
    });
}

#[test]
fn refresh_rotates_refresh_token() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;
        let login = fixture.login_user("alice", "password123").await;

        let rotated = fixture
            .refresh
            .handle(RefreshCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap();

        assert_ne!(rotated.refresh_token, login.refresh_token);
        let old_hash = fixture
            .refresh_tokens
            .find_by_hash("hashed:refresh-0")
            .await
            .unwrap()
            .unwrap();
        assert!(old_hash.revoked_at.is_some());
        let new_hash = fixture
            .refresh_tokens
            .find_by_hash(&format!("hashed:{}", rotated.refresh_token))
            .await
            .unwrap()
            .unwrap();
        assert!(new_hash.revoked_at.is_none());
    });
}

#[test]
fn refresh_rejects_reused_token_and_revokes_family() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;
        let login = fixture.login_user("alice", "password123").await;
        let rotated = fixture
            .refresh
            .handle(RefreshCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap();

        let error = fixture
            .refresh
            .handle(RefreshCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidToken));
        let reused_family = fixture
            .refresh_tokens
            .find_by_hash(&format!("hashed:{}", rotated.refresh_token))
            .await
            .unwrap()
            .unwrap();
        assert!(reused_family.revoked_at.is_some());
    });
}

#[test]
fn refresh_rejects_expired_token() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;
        let login = fixture.login_user("alice", "password123").await;

        fixture
            .clock
            .advance_to(1_000 + REFRESH_TOKEN_TTL_SECONDS + 1);

        let error = fixture
            .refresh
            .handle(RefreshCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidToken));
    });
}

#[test]
fn logout_revokes_refresh_token_idempotently() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;
        let login = fixture.login_user("alice", "password123").await;

        fixture
            .logout
            .handle(LogoutCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap();
        fixture
            .logout
            .handle(LogoutCommand {
                refresh_token: login.refresh_token.clone(),
            })
            .await
            .unwrap();

        let stored = fixture
            .refresh_tokens
            .find_by_hash("hashed:refresh-0")
            .await
            .unwrap()
            .unwrap();
        assert!(stored.revoked_at.is_some());
    });
}

#[test]
fn current_user_resolves_access_token() {
    block_on(async {
        let fixture = Fixture::new(1_000);
        fixture.register_user("alice", "password123").await;
        let login = fixture.login_user("alice", "password123").await;

        let view = fixture
            .current_user
            .handle(&login.access_token)
            .await
            .unwrap();
        assert_eq!(view, login.user);

        let error = fixture.current_user.handle("access:not-a-uuid").await.unwrap_err();
        assert!(matches!(error, AppError::InvalidToken));
    });
}
