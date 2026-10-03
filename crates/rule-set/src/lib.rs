//! 规则集上下文：用户自建的具名规则集（Clash `rule-providers`、sing-box `rule_set`、
//! Surge ruleset 等的中性抽象）的声明、内容与刷新。
//!
//! # 模型与规则
//!
//! - 聚合 `RuleSet`：具名、按用户隔离；来源为 `Remote`（远程 URL）、`Inline`（自写内容）
//!   或 `Local`（客户端本地路径，仅表示、不抓取）；分类与内容格式均为格式中性的枚举
//! - 实体 `RuleSetContent`：与聚合 1:1，保存当前内容字节与抓取元数据；`pinned` 时手动
//!   内容不被刷新覆盖
//!
//! # 范围
//!
//! 本上下文只负责"具名规则集"自身的建模、存储与抓取，不做任何目标格式的渲染/投影，
//! 也不依赖 publication/source；格式渲染与集成由后续阶段在 `api` 编排。
//!
//! # 拥有的表
//!
//! `rule_sets`、`rule_set_contents`
//!
//! 仅单向依赖 `user` 的 `UserId`。

pub mod domain;

pub use domain::{
    count_rules, body_hash, DomainError, RepositoryError, RuleSet, RuleSetCategory,
    RuleSetContent, RuleSetContentFormat, RuleSetId, RuleSetInterval, RuleSetName, RuleSetPath,
    RuleSetRepository, RuleSetSource, RuleSetUrl, RULE_SET_INTERVAL_MAX_SECS,
    RULE_SET_NAME_MAX_LEN, RULE_SET_NAME_MIN_LEN, RULE_SET_PATH_MAX_LEN, RULE_SET_URL_MAX_LEN,
};

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
