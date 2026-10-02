# sub-converter-api

Cloudflare Workers API：Rust + worker-rs + axum + D1，DDD 分层用户系统。

## 架构

```
crates/
├── user/        # 用户限界上下文：domain / application / infrastructure 模块分层
├── contract/    # 前后端共享的 API 契约（serde DTO、输入约束常量）
├── api/         # 组合根：axum 路由、错误映射、worker 入口（cdylib）
└── web/         # Leptos CSR 前端（骨架）
```

依赖方向：contract ← api / web；user ← api；web 不依赖 user。

## 技术要点

- 用户 ID：UUID v4，用户名不区分大小写唯一
- 密码：PBKDF2-HMAC-SHA256（WebCrypto，600k 迭代，原生执行不计 Worker CPU），PHC 格式存储
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
| POST | `/api/auth/login` | 登录，返回 access + refresh token |
| POST | `/api/auth/refresh` | 轮换 refresh token |
| POST | `/api/auth/logout` | 吊销 refresh token（幂等），返回 204 |
| GET | `/api/users/me` | 需 `Authorization: Bearer <accessToken>` |

错误统一为 `{"error":{"code","message"}}`；422 校验失败、409 用户名已存在、401 凭证/令牌无效。

```bash
BASE=http://127.0.0.1:8787
curl -X POST $BASE/api/auth/register -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"password123"}'

TOKENS=$(curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"password123"}')
ACCESS=$(echo "$TOKENS" | jq -r .accessToken)
REFRESH=$(echo "$TOKENS" | jq -r .refreshToken)

curl $BASE/api/users/me -H "Authorization: Bearer $ACCESS"
curl -X POST $BASE/api/auth/refresh -H 'Content-Type: application/json' \
  -d "{\"refreshToken\":\"$REFRESH\"}"
curl -X POST $BASE/api/auth/logout -H 'Content-Type: application/json' \
  -d "{\"refreshToken\":\"$REFRESH\"}"
```

## 部署

```bash
npx wrangler d1 create sub-converter-db-dev
npx wrangler d1 create sub-converter-db-prod
# 将输出的 database_id 填入 wrangler.toml 对应 binding

npx wrangler secret put JWT_SECRET --env production
npx wrangler d1 migrations apply sub-converter-db-prod --remote --env production
npx wrangler deploy --env production
```

> 前端骨架暂未接入 Worker 静态资源（`[assets]`），当前部署只包含 API；接入后再补 `run_worker_first`/SPA 回退配置。
