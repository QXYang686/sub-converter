# 运维手册

## 环境

| 环境 | Worker | 地址 | 数据库 |
|---|---|---|---|
| 生产 | `sub-converter` | https://sub.yqxpro.com | `sub-converter-db-prod` |
| Staging | `sub-converter-staging` | https://sub-converter-staging.yqxpro.workers.dev | `sub-converter-db-dev`（远程） |
| 本地 | `sub-converter` | http://127.0.0.1:8787 | `.wrangler/state` 本地 SQLite |

原则：功能验证（注册/登录/删除等写操作）在 staging 做；生产只做只读冒烟（首页 200、未登录 401）。

## 部署

- 生产：push 到 `main` → GitHub Actions `Deploy`（测试 → 应用生产迁移 → 部署）
- Staging：GitHub Actions 手动触发 `Deploy Staging`，或本地 `npx wrangler deploy --env staging`
- 本地全量构建验证：`npx wrangler deploy --dry-run --env production`

CI 需要仓库 Secrets：`CLOUDFLARE_API_TOKEN`（Edit Cloudflare Workers 模板 + D1 Edit）、`CLOUDFLARE_ACCOUNT_ID`。未配置 token 时生产工作流会 warning 跳过。

## D1 迁移

新增迁移：

1. 在 `migrations/` 新建 `000N_描述.sql`
2. 本地验证：`npx wrangler d1 migrations apply sub-converter-db-dev --local`
3. Staging 验证：`npx wrangler d1 migrations apply sub-converter-db-dev --remote --env staging` 后部署 staging 冒烟
4. 合并到 `main` 后由 CI 应用到生产

约束：

- 迁移一旦部署到生产不可回滚，只向前修（新的迁移文件）
- 保持向后兼容：先加列/建表，确认新代码稳定后再清理旧结构（`users.password_hash` 就是这样迁移到 `credentials` 的）
- SQLite 能力有限（如 DROP COLUMN 支持但需谨慎），改动大表前先在 staging 验证

## Secret 轮换

`JWT_SECRET`：

```bash
openssl rand -base64 48 | npx wrangler secret put JWT_SECRET --env production
```

- 轮换后所有已签发的 access token（≤15 分钟）立即失效，前端会自动走 refresh 换新
- refresh token 独立于该 secret（D1 存哈希），不受影响，用户无需重新登录

## 回滚

- 代码：`npx wrangler rollback --env production`（回到上一个版本），或 revert 提交后 push 让 CI 重新部署
- 数据：迁移不可逆，回滚代码前确认旧代码与新 schema 兼容

## 自定义域名

`sub.yqxpro.com` 通过 `wrangler.toml` 的 `[[env.production.routes]] custom_domain = true` 管理，首次部署自动建 DNS 和证书。前提：`yqxpro.com` zone 与 Worker 在同一 Cloudflare 账号。改域名需要同步更新 `WEBAUTHN_RP_ID`/`WEBAUTHN_ORIGINS`，否则注册的 passkey 失效。

## 备份与排查

```bash
npx wrangler d1 export sub-converter-db-prod --remote --output backup.sql   # 备份
npx wrangler d1 execute <db> --remote --command "SELECT ..."                # 只读查询
npx wrangler tail --env production                                          # 实时日志（含内部错误详情）
```

本地状态重置：删除 `.wrangler/state`，重新 `wrangler d1 migrations apply --local`，`.dev.vars` 保留。

## 常见问题

| 现象 | 原因 / 处理 |
|---|---|
| 注册 500，日志 `iteration counts above 100000 are not supported` | 生产 WebCrypto PBKDF2 上限 100k，不能调高迭代（见 ADR 0003） |
| 登录后 cookie 没存下 | 本地请用 `http://localhost:8080`；`Secure` cookie 在非 localhost 的 http 下会被拒 |
| Passkey 注册/登录 422 | origin 不在 `WEBAUTHN_ORIGINS`，或 rpIdHash 与 `WEBAUTHN_RP_ID` 不匹配 |
| Passkey 注册成功但换域名后无法登录 | RP ID 变更会使已注册凭证失效，需重新注册 |
| `wrangler dev` 启动慢 | build 命令会同时构建前端（`trunk build --release`），只调 API 时会慢一些 |
| CI 跳过部署 | 缺少 `CLOUDFLARE_API_TOKEN` secret |
