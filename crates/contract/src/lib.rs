//! 前后端共享契约（发布语言）：serde DTO 与输入约束常量。
//!
//! - 模块对应上下文：`auth`、`passkey`、`user`、`subscription`、`rule_set`
//! - 约束常量是 web 表单校验与后端 domain 规则的共同来源，二者一致性由 `crates/api` 的测试保证
//! - 不依赖任何业务 crate：`api` 负责 domain/DTO 映射，`web` 只依赖本 crate

pub mod auth;
pub mod passkey;
pub mod rule_set;
pub mod subscription;
pub mod user;

pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;
pub const PASSWORD_MIN_LEN: usize = 8;
pub const PASSWORD_MAX_LEN: usize = 72;
pub const SUBSCRIPTION_NAME_MIN_LEN: usize = 1;
pub const SUBSCRIPTION_NAME_MAX_LEN: usize = 64;
pub const SOURCE_URL_MAX_LEN: usize = 2048;
pub const PUBLICATION_MAX_SOURCES: usize = 50;
pub const RULE_SET_NAME_MIN_LEN: usize = 1;
pub const RULE_SET_NAME_MAX_LEN: usize = 64;
pub const RULE_SET_URL_MAX_LEN: usize = 2048;
pub const RULE_SET_PATH_MAX_LEN: usize = 512;
pub const RULE_SET_INTERVAL_MAX_SECS: i64 = 2_592_000;
