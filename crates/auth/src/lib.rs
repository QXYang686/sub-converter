//! 认证上下文：登录方式（密码/Passkey）、会话与刷新令牌。
//!
//! # 模型与规则
//!
//! - 聚合 `Credential`：`CredentialSecret` 为 `Password(PasswordHash)` 或 `Passkey`
//! - `Challenge`：一次性 WebAuthn challenge，注册/登录两种 kind，TTL 5 分钟，消费即删
//! - 密码 8~72 字符；access token 15 分钟；refresh token 30 天
//!
//! # 用例（`application`）
//!
//! - 密码：`RegisterHandler`（建 User + 密码凭证，凭证失败补偿删除 User）、`LoginHandler`、`RefreshHandler`（轮换 + 重放检测吊销整族）、`LogoutHandler`
//! - Passkey：注册/登录各分 start（生成 options 与 challenge）与 finish（验签、签发会话）
//! - `GetCurrentUserHandler`、`ListPasskeysHandler`、`DeletePasskeyHandler`
//!
//! # 端口与适配器
//!
//! - 端口：`PasswordHasher`、`TokenService`、`WebAuthnVerifier`、`RefreshTokenRepository`、`Clock`、`RandomSource`
//! - `webauthn`：纯 Rust ES256/Ed25519 验签，原生可测；D1/PBKDF2/JWT 适配器在 `infrastructure`（仅 `wasm32`）
//! - 拥有的表：`credentials`、`refresh_tokens`、`webauthn_challenges`
//!
//! 仅单向依赖 `user` 的 `UserId`/`Username`/`UserRepository`。

pub mod application;
pub mod domain;
pub mod webauthn;

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
