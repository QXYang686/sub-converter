# AGENTS.md

## 项目
Cloudflare Workers API + Leptos CSR 前端：Rust workspace，DDD 用户系统。

## 结构
- `crates/user`：用户上下文，账号（User、Username）与仓储
- `crates/auth`：认证上下文，凭证（密码/Passkey）、JWT、refresh token、认证用例
- `crates/contract`：前后端共享的 API 契约（serde DTO、输入约束常量）
- `crates/api`：组合根，axum 路由、错误映射与 worker 入口（cdylib）
- `crates/web`：Leptos CSR 前端
- `docs/`：架构、运维手册与 ADR；重要决策追加新 ADR（只增不改）
- 依赖方向：user ← auth ← api；contract ← api/web；web 不依赖 user/auth

## 命令
- 原生检查/测试（domain/application）：`cargo test`
- wasm 检查：`cargo check --target wasm32-unknown-unknown`
- 本地运行：`wrangler dev`；D1 迁移：`wrangler d1 migrations apply <db> --local`
- 前端本地：`cd crates/web && trunk serve`（代理 /api 到 8787）

## 约定
- worker 相关代码仅在 `cfg(target_arch = "wasm32")` 下编译
- 业务参数（TTL、迭代次数）用代码常量；仅密钥走 secret
- contract 约束常量与 domain 规则的一致性由 crates/api 的测试保证

## 工作方式
- 非必要不读写当前仓库外的内容
- 乐观执行：按约定直接实现，少读依赖源码，编译/测试报错再针对性排查
