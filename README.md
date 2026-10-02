# sub-converter-api

Cloudflare Workers API：Rust + worker-rs + axum + D1，DDD 分层用户系统。

## 架构

```
crates/
├── user/        # 用户上下文：账号（User、Username）与仓储
├── auth/        # 认证上下文：凭证（密码/Passkey）、JWT、refresh token、认证用例
├── contract/    # 前后端共享的 API 契约（serde DTO、输入约束常量）
├── api/         # 组合根：axum 路由、错误映射、worker 入口（cdylib）
└── web/         # Leptos CSR 前端（骨架）
```

依赖方向：user ← auth ← api；contract ← api / web；web 不依赖 user/auth。

## 技术要点

- 用户 ID：UUID v4，用户名不区分大小写唯一
- 密码：PBKDF2-HMAC-SHA256（WebCrypto，100k 迭代，Cloudflare 平台上限；原生执行不计 Worker CPU），PHC 格式存储
- 令牌：15 分钟 JWT HS256 + 30 天不透明 refresh token（D1 只存 SHA-256 哈希，刷新轮换，重放检测吊销整族）
- 配置：仅 `JWT_SECRET` 走 secret，TTL/迭代次数是代码常量（`src/application/config.rs`）

## 本地开发

```bash
cp .dev.vars.example .dev.vars          # 修改 JWT_SECRET
npx wrangler d1 migrations apply sub-converter-db-dev --local
npx wrangler dev                        # API :8787
```

前端骨架（Leptos CSR + Tailwind + Trunk）：

```bash
cd crates/web
trunk serve                             # http://127.0.0.1:8080，/api 代理到 8787
```

> `wrangler dev` 会执行完整构建（worker-build + `trunk build --release`），仅调 API 时首次启动会慢一些。

测试与检查：

```bash
cargo test                              # user domain/application 单测（原生）
cargo check --target wasm32-unknown-unknown
cargo check -p sub-converter-web --target wasm32-unknown-unknown
cd crates/web && trunk build            # 产物在 crates/web/dist
```

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/auth/register` | 注册，返回 201 |
| POST | `/api/auth/login` | 登录，返回 access token 并下发 refresh cookie |
| POST | `/api/auth/refresh` | 凭 cookie 轮换 refresh token，无请求体 |
| POST | `/api/auth/logout` | 吊销并清除 cookie（幂等），返回 204 |
| GET | `/api/users/me` | 需 `Authorization: Bearer <accessToken>` |

refresh token 通过 `HttpOnly; Secure; SameSite=Strict; Path=/api/auth` cookie 传递，JS 不可读；refresh/logout 需带 `X-Requested-With: XMLHttpRequest`。

错误统一为 `{"error":{"code","message"}}`；422 校验失败、409 用户名已存在、401 凭证/令牌无效、403 CSRF 校验失败。

```bash
BASE=http://127.0.0.1:8787
JAR=$(mktemp)

curl -s -X POST $BASE/api/auth/register -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"password123"}'

ACCESS=$(curl -s -c "$JAR" -X POST $BASE/api/auth/login \
  -H 'Content-Type: application/json' \
  -H 'X-Requested-With: XMLHttpRequest' \
  -d '{"username":"alice","password":"password123"}' | jq -r .accessToken)

curl -s $BASE/api/users/me -H "Authorization: Bearer $ACCESS"

# cookie 自动携带，刷新会轮换并覆盖本地 cookie
curl -s -b "$JAR" -c "$JAR" -X POST $BASE/api/auth/refresh \
  -H 'X-Requested-With: XMLHttpRequest'

curl -s -b "$JAR" -X POST $BASE/api/auth/logout \
  -H 'X-Requested-With: XMLHttpRequest'
```

## 部署

Worker 名为 `sub-converter`，生产通过自定义域名 `sub.yqxpro.com` 访问（`yqxpro.com` 需在同一个 Cloudflare 账号）。前端产物作为静态资源托管：`/api/*` 走 Worker，其余路径 SPA 回退到 `index.html`。

首次初始化：

```bash
npx wrangler login
npx wrangler d1 create sub-converter-db-dev
npx wrangler d1 create sub-converter-db-prod
# 将输出的 database_id 填入 wrangler.toml 对应 binding

npx wrangler secret put JWT_SECRET --env production
npx wrangler d1 migrations apply sub-converter-db-prod --remote --env production
npx wrangler deploy --env production
```

## 环境

| 环境 | Worker | 地址 / 数据库 |
|---|---|---|
| 生产 | `sub-converter` | https://sub.yqxpro.com，`sub-converter-db-prod` |
| Staging | `sub-converter-staging` | https://sub-converter-staging.yqxpro.workers.dev，`sub-converter-db-dev` |

功能验证（注册/登录/清理）在 staging 进行；生产只做只读冒烟。

## GitHub Actions

| 工作流 | 触发 | 说明 |
|---|---|---|
| `Deploy` | push 到 `main` | 测试 + 迁移生产库 + 部署生产 |
| `Deploy Staging` | 手动触发 | 测试 + 迁移 dev 库 + 部署 staging |

在仓库 Settings → Secrets and variables → Actions 添加：

| Secret | 说明 |
|---|---|
| `CLOUDFLARE_API_TOKEN` | 建议用 "Edit Cloudflare Workers" 模板，并额外授予 D1 Edit 权限 |
| `CLOUDFLARE_ACCOUNT_ID` | `npx wrangler whoami` 可查 |

未配置 `CLOUDFLARE_API_TOKEN` 时生产工作流会跳过部署（warning），不会失败。
