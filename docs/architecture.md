# 架构

Cloudflare Workers 上的单体应用：Rust（worker-rs + axum）API + Leptos CSR 前端 + D1，同域部署（`/api/*` 走 Worker，其余静态资源，SPA 回退）。

## 限界上下文

| crate | 职责 | 关键模型 |
|---|---|---|
| [`crates/user`](../crates/user/src/lib.rs) | 用户上下文：账号身份锚点 | `User`、`UserId`、`Username`、`UserRepository` |
| [`crates/auth`](../crates/auth/src/lib.rs) | 认证上下文：登录方式与凭证、会话 | `Credential`（Password \| Passkey）、`Challenge`、JWT/refresh token、WebAuthn 验签 |
| [`crates/subscription`](../crates/subscription/src/lib.rs) | 订阅上下文：两类订阅与拉取转换分发 | `Source`、`Publication`、`SourceSnapshot`、Clash 解析/合并/渲染 |
| [`crates/contract`](../crates/contract/src/lib.rs) | 前后端共享契约（发布语言） | 请求/响应 DTO、输入约束常量 |
| `crates/api` | 组合根：路由、错误映射、依赖注入、跨上下文编排 | axum handlers、`AppState`、worker 入口 |
| `crates/web` | Leptos CSR 表现层 | 页面、组件、passkey JS 桥、AuthStore |

各上下文的内部细节（聚合与不变量、用例、端口、拥有的表）以 crate 级 rustdoc 维护：`cargo doc --workspace --open`，或直接看对应 `src/lib.rs`。本文件只保留 context map 与跨上下文视图。

## 依赖方向

```
        contract ←──────── api ────────┐
            ↑                ↑          │
            └──────────── web│          ▼
                         user ←──── auth
                           ↑
                      subscription
```

- `auth → user` 单向：认证上下文只使用 `UserId`/`Username`/`UserRepository`，用户上下文不知道认证的存在
- `subscription → user` 单向：订阅上下文只使用 `UserId`，不依赖 auth
- `web` 只依赖 `contract`，不接触 user/auth/subscription
- 跨上下文流程（注册、`/me`）由 `api` 编排，不在上下文之间互相调用
- `contract` 不依赖任何业务 crate

## 目录

```
crates/
├── user/src/
│   ├── domain/            # User、值对象、仓储 trait、错误
│   └── infrastructure/    # D1UserRepository（cfg(wasm32)）
├── auth/src/
│   ├── domain/            # Credential/Passkey、Challenge、仓储 trait
│   ├── application/       # 注册/登录/刷新/登出/passkey 用例、ports、config
│   └── infrastructure/    # D1 仓储、PBKDF2、JWT、随机、时钟（cfg(wasm32)）；security/webauthn 纯 Rust 验签，原生可测
├── subscription/src/
│   ├── domain/            # Source/Publication/SourceSnapshot、Clash 解析合并渲染
│   ├── application/       # 管理用例 + RefreshSource/ServePublication、ports
│   └── infrastructure/    # D1 仓储、HttpFetcher、wait_until 后台任务（cfg(wasm32)）
├── contract/src/          # auth / passkey / user DTO + 约束常量
├── api/src/
│   ├── http/              # routes、handlers、dto 映射、错误、cookie、AppState
│   └── worker_entry.rs    # #[event(fetch)] 组装依赖
└── web/src/               # api / components / pages / state / passkey / forms
```

## 数据模型（D1）

| 表 | 说明 |
|---|---|
| `users` | `id`(UUID PK)、`username`(唯一，小写)、时间戳 |
| `credentials` | 多凭证：`kind`=password/passkey；密码存 PHC 哈希，passkey 存 credential_id/public_key/sign_count/transports/label |
| `refresh_tokens` | 只存 token 的 SHA-256 哈希、过期与吊销时间 |
| `webauthn_challenges` | 一次性 challenge，注册/登录各一种 kind，5 分钟 TTL，消费即删 |
| `sources` / `publications` / `publication_sources` | 订阅源、发布订阅及有序绑定 |
| `source_snapshots` | 每个源最近一次成功抓取的原始 body（BLOB，≤1.8MB）、ETag/Last-Modified、刷新租约、最近错误，以及 body 哈希、节点/分组/规则计数、协议分布、订阅流量与机场名 |
| `source_proxies` / `source_proxy_groups` | 从快照提取的节点与分组，逐行存储，`options` 为完整原始字段 JSON |
| `source_config` | 从快照提取的规则数组与顶层设置（dns/tun/hosts/rule-providers 等）JSON |
| `publication_snapshots` | 每个发布订阅物化的渲染结果（`publication_id + format`）、聚合流量与生成时间，公开分发直接查表 |

迁移文件在 `migrations/`，操作规范见 [runbook](runbook.md)。

D1 访问策略：每请求创建一个 `first-primary` 的 D1 Session 并由全部仓储共享（顺序一致、首查询走主库），两个库均开启全局读复制；各环境启用 Smart Placement 让 Worker 贴近主库。见 [ADR 0008](decisions/0008-d1-sessions-and-placement.md)。

## 关键流程

- **注册**：`auth` 的注册用例编排「建 User → 建 Password 凭证」，凭证写入失败时补偿删除 User；`api` 只做 DTO 映射
- **密码登录**：按用户名找 User → 按 user_id 找 Password 凭证 → 校验 → 签发 access + refresh
- **Passkey 注册**：start 生成 challenge + options（需登录）→ 浏览器 `navigator.credentials.create` → finish 验签并存公钥
- **Passkey 登录**：start（可带用户名绑定 allowCredentials，也可 discoverable）→ `credentials.get` → finish 验签并签发会话
- **会话**：access token 15 分钟只存内存；refresh token 30 天走 `HttpOnly; Secure; SameSite=Strict; Path=/api/auth` cookie，D1 存哈希，刷新轮换，重放检测吊销整族
- **拉取**：源变更或公开拉取时经 `wait_until` 异步抓取（固定 FlClash UA，条件请求，租约去重），原始响应体与订阅响应头存 `source_snapshots`，并提取全部协议节点、分组、规则与顶层设置；body 哈希未变只更新元数据，失败保留旧快照与旧提取数据并记录错误；提取缺失时在 200 未变或 304 路径补写
- **转换与分发**：`GET /s/{secret}` 校验启用/过期后直接读 `publication_snapshots` 返回渲染结果；源刷新成功会重建绑定该源的所有发布订阅快照，管理变更（源 URL/启用/删除、发布组合）即时失效缓存，未命中时从提取数据合成一次（含分组、规则与设置）。详见 [ADR 0009](decisions/0009-pull-convert-serve.md)、[ADR 0010](decisions/0010-extract-subscription-content.md)、[ADR 0011](decisions/0011-publication-render-snapshots.md) 与 [ADR 0012](decisions/0012-merge-extracted-config-into-publication.md)

## 配置分层

| 类型 | 内容 | 位置 |
|---|---|---|
| 代码常量 | token/challenge TTL、PBKDF2 迭代次数 | `crates/auth/src/application/config.rs` 等 |
| 环境变量 | `WEBAUTHN_RP_ID`、`WEBAUTHN_ORIGINS`、`LOG_LEVEL` | `wrangler.toml` 各 env（本地默认 `localhost`） |
| Secret | `JWT_SECRET` | `wrangler secret` / 本地 `.dev.vars` |

## 可观测性

- 日志：业务 crate 用 `tracing` facade；`crates/api/src/telemetry.rs`（wasm-only）输出 JSON 到 `console.log`，由 Workers Logs 索引；`LOG_LEVEL` 控制级别（支持 EnvFilter 语法），默认 `info`
- 请求链路：`http.request` span 记录 request_id/method/route/status/latency_ms/user_id，响应回写 `x-request-id`；5xx 记 error（含详情），4xx 记 debug
- traces：wrangler 的 `[observability.traces]` 开关，平台自动为 handler 与 D1 binding 生成 span
- 脱敏约定：password/hash/token/challenge/cookie 一律不入日志；用例埋点用 `#[instrument(skip_all)]`
- 决策与免费额度说明见 [ADR 0007](decisions/0007-observability.md)

## 测试策略

- `cargo test`（原生）：domain 规则、application 用例（内存假仓储）、Clash 解析/合并/渲染、WebAuthn 真实签名向量、HTTP cookie/CSRF 纯函数
- `cargo check --target wasm32-unknown-unknown`：worker 相关代码编译
- 本地 e2e：curl cookie jar；passkey 用 Node WebCrypto 模拟 ES256 客户端；公开订阅用本地桩上游 + 本地 D1 验证「空配置 → 后台抓取 → 二次拉取有节点」与条件请求
- 真实数据验证用临时脚本/example 对照真实源，订阅 URL 与内容一律不落仓库
- dev 做功能验证，生产只做只读冒烟，见 [runbook](runbook.md)

相关决策记录见 [docs/decisions](decisions/)。
