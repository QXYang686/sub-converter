# sub-converter

Cloudflare Workers 上的订阅转换工具：Rust（worker-rs + axum）API + Leptos CSR 前端 + D1，同域部署（`/api/*` 走 Worker，其余静态资源）。

## 快速开始

```bash
cp .dev.vars.example .dev.vars          # 修改 JWT_SECRET
npx wrangler d1 migrations apply sub-converter-db-dev --local
npx wrangler dev                        # API :8787
```

前端（另开终端）：

```bash
cd crates/web
trunk serve                             # http://localhost:8080，/api 代理到 8787
```

> 依赖 rust（含 wasm32 target）、worker-build、trunk、node。`wrangler dev` 会完整构建前后端，首次启动较慢。
> 本地测试 Passkey 请用 `http://localhost:8080`（RP ID 为 `localhost`，127.0.0.1 不匹配）。

## 常用命令

```bash
cargo test                                                 # 原生单测
cargo check --target wasm32-unknown-unknown                # worker 相关编译
cargo check -p sub-converter-web --target wasm32-unknown-unknown
cd crates/web && trunk build                               # 产物 crates/web/dist
npx wrangler deploy --dry-run --env production             # 本地验证生产构建
```

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/auth/register` | 注册，返回 201 |
| POST | `/api/auth/login` | 密码登录，返回 access token 并下发 refresh cookie |
| POST | `/api/auth/refresh` | 凭 cookie 轮换 refresh token，无请求体 |
| POST | `/api/auth/logout` | 吊销并清除 cookie（幂等），返回 204 |
| GET | `/api/users/me` | 需 `Authorization: Bearer <accessToken>` |
| POST | `/api/auth/passkey/register/{start,finish}` | 登录后添加 Passkey |
| POST | `/api/auth/passkey/login/{start,finish}` | Passkey 登录（支持 discoverable） |
| GET / DELETE | `/api/auth/passkeys[/{id}]` | 管理 Passkey |
| GET/POST | `/api/subscriptions/sources` | 订阅源列表（含抓取快照摘要：流量/到期/节点与协议分布/最近错误）/ 新建（同用户 URL 唯一） |
| GET/PATCH/DELETE | `/api/subscriptions/sources/{id}` | 订阅源详情 / 修改 / 删除 |
| GET/POST | `/api/subscriptions/publications` | 发布订阅列表 / 新建（生成 secret 并绑定源） |
| GET/PATCH/DELETE | `/api/subscriptions/publications/{id}` | 发布订阅详情 / 修改（含 expiresAt）/ 删除 |
| PUT | `/api/subscriptions/publications/{id}/sources` | 整体设置有序订阅源组成 |
| GET | `/s/{secret}?target=clash` | 公开订阅分发（无需鉴权），直接返回物化渲染快照（全协议）；无缓存时内联构建，未知/禁用/过期 404 |

refresh token 走 `HttpOnly; Secure; SameSite=Strict; Path=/api/auth` cookie，JS 不可读；refresh/logout 需带 `X-Requested-With: XMLHttpRequest`。错误统一为 `{"error":{"code","message"}}`（422 校验、409 冲突、401 凭证/令牌、403 CSRF、404 不存在）。

## 文档

- [架构](docs/architecture.md)：限界上下文、依赖方向、数据模型、测试策略
- [运维手册](docs/runbook.md)：部署、D1 迁移、secret 轮换、回滚、故障排查
- [决策记录](docs/decisions/)：重要取舍的 ADR

## 部署

生产 https://sub.yqxpro.com（Worker `sub-converter`），push `main` 由 GitHub Actions 自动测试、迁移并部署；staging 手动触发 `Deploy Staging`。环境与操作细节见 [runbook](docs/runbook.md)。

首次初始化（新账号或灾备重建）：

```bash
npx wrangler login
npx wrangler d1 create sub-converter-db-dev
npx wrangler d1 create sub-converter-db-prod
# 将输出 database_id 填入 wrangler.toml 对应 binding
npx wrangler secret put JWT_SECRET --env production
npx wrangler d1 migrations apply sub-converter-db-prod --remote --env production
npx wrangler deploy --env production
```
