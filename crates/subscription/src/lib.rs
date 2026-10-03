//! 订阅上下文：订阅源到发布订阅的拉取、转换与分发。
//!
//! # 模型与规则
//!
//! - 聚合 `Source`：`SourceUrl` 仅接受 http/https；`SubscriptionName` 与发布共用
//! - 聚合 `Publication`：持有有序 `PublicationSource` 绑定，最多 50 个且不可重复；`PublicationSecret` 用于公开分发
//! - `SourceSnapshot`：上游原始响应 + ETag/Last-Modified + 刷新租约 + 提取结果
//! - `PublicationSnapshot`：物化的渲染结果，公开分发直接读取
//!
//! # 用例（`application`）
//!
//! - 源与发布的 CRUD、`SetPublicationSourcesHandler`
//! - `RefreshSourceHandler`：条件请求抓取、提取节点/分组/规则/设置、重建绑定该源的发布快照
//! - `ServePublicationHandler`：按 secret 校验启用/过期并返回渲染结果
//!
//! # 领域服务
//!
//! - `domain::clash`：Clash YAML 解析、按 identity 去重合并、渲染（含 `PROXY` 分组）
//! - `domain::metadata`：解析 `Subscription-Userinfo` 与 Content-Disposition
//!
//! # 端口与适配器
//!
//! - 端口：`Fetcher`、`Clock`、`SecretGenerator`、`BackgroundTasks`
//! - D1 仓储、`HttpFetcher`、`wait_until` 后台任务在 `infrastructure`（仅 `wasm32`）
//! - 拥有的表：`sources`、`publications`、`publication_sources`、`source_snapshots`、`source_proxies`、`source_proxy_groups`、`source_config`、`publication_snapshots`
//!
//! 仅单向依赖 `user` 的 `UserId`。

pub mod application;
pub mod domain;

pub use domain::{
    DomainError, Publication, PublicationId, PublicationRepository, PublicationSecret,
    PublicationSnapshot, PublicationSnapshotRepository, PublicationSource, RepositoryError,
    SnapshotRepository, Source, SourceId, SourceRepository, SourceSnapshot, SourceUrl,
    SubscriptionName, SubscriptionUserInfo, PUBLICATION_MAX_SOURCES, PUBLICATION_SECRET_MAX_LEN,
    PUBLICATION_SECRET_MIN_LEN, SOURCE_URL_MAX_LEN, SUBSCRIPTION_NAME_MAX_LEN,
    SUBSCRIPTION_NAME_MIN_LEN,
};

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
