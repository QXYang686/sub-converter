//! 组合根：axum 路由、错误映射、依赖注入与跨上下文编排。
//!
//! - `http`：router、handlers、DTO 映射、cookie、`AppState`、请求链路追踪
//! - `worker_entry`：`#[event(fetch)]` 入口，每请求装配 D1 session 与全部端口（仅 `wasm32`）
//! - `telemetry`：JSON 日志初始化（仅 `wasm32`）
//!
//! 这里是唯一了解全部上下文的地方，负责 domain/DTO 互转；业务规则仍留在各上下文内。

pub mod http;

#[cfg(target_arch = "wasm32")]
mod telemetry;

#[cfg(target_arch = "wasm32")]
mod worker_entry;

#[cfg(test)]
mod tests {
    #[test]
    fn contract_constraints_match_domain_rules() {
        assert_eq!(contract::USERNAME_MIN_LEN, user::USERNAME_MIN_LEN);
        assert_eq!(contract::USERNAME_MAX_LEN, user::USERNAME_MAX_LEN);
        assert_eq!(contract::PASSWORD_MIN_LEN, auth::domain::PASSWORD_MIN_LEN);
        assert_eq!(contract::PASSWORD_MAX_LEN, auth::domain::PASSWORD_MAX_LEN);
        assert_eq!(
            contract::SUBSCRIPTION_NAME_MIN_LEN,
            subscription::SUBSCRIPTION_NAME_MIN_LEN
        );
        assert_eq!(
            contract::SUBSCRIPTION_NAME_MAX_LEN,
            subscription::SUBSCRIPTION_NAME_MAX_LEN
        );
        assert_eq!(
            contract::SOURCE_URL_MAX_LEN,
            subscription::SOURCE_URL_MAX_LEN
        );
        assert_eq!(
            contract::PUBLICATION_MAX_SOURCES,
            subscription::PUBLICATION_MAX_SOURCES
        );
    }
}
