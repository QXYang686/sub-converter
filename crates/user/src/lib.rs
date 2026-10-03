//! 用户上下文：账号身份锚点。
//!
//! 只回答"用户是谁"，不感知认证方式与订阅数据；对外的唯一边界是 `UserId`。
//!
//! # 模型与规则
//!
//! - 聚合 `User`：`UserId` + `Username` + 时间戳
//! - `Username` 不变量：trim 后 3~32 字符，仅 ASCII 字母/数字/`_`/`-`，统一存为小写
//!
//! # 对外契约
//!
//! - 端口 `UserRepository`（D1 适配器在 `infrastructure`，仅 `wasm32` 编译）
//! - 拥有的表：`users`
//!
//! # 边界
//!
//! user 不依赖 auth/subscription；`auth` 的注册用例通过 `UserRepository` 写入用户并做失败补偿。

pub mod domain;

pub use domain::{
    DomainError, RepositoryError, User, UserId, UserRepository, Username, USERNAME_MAX_LEN,
    USERNAME_MIN_LEN,
};

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
