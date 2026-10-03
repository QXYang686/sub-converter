# 运维手册

## 环境

| 环境 | Worker | 地址 | 数据库 |
|---|---|---|---|
| dev（线上） | `sub-converter-dev` | https://sub-converter-dev.yqxpro.workers.dev | `sub-converter-db-dev` |
| dev（本地） | 同 dev 配置（miniflare 模拟） | http://127.0.0.1:8787 | `.wrangler/state` 本地 SQLite；`--remote` 时用线上 dev 库 |
| 生产 | `sub-converter` | https://sub.yqxpro.com | `sub-converter-db-prod` |

只有两套环境：`dev` 与 `prod`。Wrangler **顶层默认节就是 dev**——`wrangler dev`（本地）、`wrangler dev --remote`（线上 dev 库）、`wrangler deploy`（部署 dev）都走它；`[env.production]` 独立配置。本地与线上 dev 的差异只在 vars：顶层是 workers.dev 值，本地用 `.dev.vars` 覆盖为 `localhost`。

原则：功能验证（注册/登录/删除等写操作）在 dev 做；生产只做只读冒烟（首页 200、未登录 401）。

## 部署

- dev：push 到 `main` → GitHub Actions `Deploy Dev` 自动部署（测试 → 应用 dev 远程迁移 → `wrangler deploy`）；也可手动 `workflow_dispatch`，或本地 `npx wrangler deploy`
- 生产：push 到 `main` → GitHub Actions `Deploy` 创建 job 并**暂停等待审批**；在 Actions 页面 Approve 后才执行（测试 → 应用生产迁移 → `--env production` 部署）
- 本地全量构建验证：`npx wrangler deploy --dry-run --env production`

生产审批依赖仓库 Settings → Environments 中名为 `production` 的 Environment 配置了 Required reviewers（建议同时限制 Deployment branches 为 `main`）；连续推送会自动取消同组的过期 run（见 ADR 0013）。

CI 需要仓库 Secrets：`CLOUDFLARE_API_TOKEN`（Edit Cloudflare Workers 模板 + D1 Edit）、`CLOUDFLARE_ACCOUNT_ID`。未配置 token 时生产工作流会 warning 跳过。

Secret 按 Worker 隔离，dev 与 prod 需分别设置：`npx wrangler secret put JWT_SECRET`（dev，顶层）与 `--env production`。

## D1 迁移

新增迁移：

1. 在 `migrations/` 新建 `000N_描述.sql`
2. 本地验证：`npx wrangler d1 migrations apply sub-converter-db-dev --local`
3. 线上 dev 验证：`npx wrangler d1 migrations apply sub-converter-db-dev --remote` 后部署 dev 冒烟
4. 合并到 `main` 后由 CI 应用到 dev 与生产（生产需审批）

约束：

- 迁移一旦部署到生产不可回滚，只向前修（新的迁移文件）
- 保持向后兼容：先加列/建表，确认新代码稳定后再清理旧结构（`users.password_hash` 就是这样迁移到 `credentials` 的）
- SQLite 能力有限（如 DROP COLUMN 支持但需谨慎），改动大表前先在 dev 验证

## 本地与远程 D1

本地 `wrangler dev` 默认用 `.wrangler/state` 下的本地 SQLite，和远程 `sub-converter-db-dev` 是**两个独立库，不会自动同步**。`wrangler dev --remote` 则直接用线上 dev 库。

```bash
# 本地库初始化（只建结构）
npx wrangler d1 migrations apply sub-converter-db-dev --local

# 远程 → 本地（仅结构）
npx wrangler d1 export sub-converter-db-dev --remote --no-data --output schema.sql
npx wrangler d1 execute sub-converter-db-dev --local --file=schema.sql

# 远程 → 本地（含数据；会把真实 hash/passkey 拷到本地，慎用）
npx wrangler d1 export sub-converter-db-dev --remote --output dev.sql
npx wrangler d1 execute sub-converter-db-dev --local --file=dev.sql

# 本地 → 远程 dev
npx wrangler d1 export sub-converter-db-dev --local --output local.sql
npx wrangler d1 execute sub-converter-db-dev --remote --file=local.sql
```

`wrangler d1 export` 是整库 dump（DDL+DML），不是增量/双向同步，大库会超时。日常开发建议只跑本地迁移，需要样例数据就给一个脱敏 seed。

## 日志与排查

```bash
npx wrangler tail                                           # dev 实时日志（顶层）
npx wrangler tail --env production                          # 生产实时日志
npx wrangler tail --format json                             # 实时结构化日志
```

- Dashboard → Workers → 选择 Worker → Observability，可查询结构化日志：按 `request_id` 串联单次请求，字段含 method/route/status/latency_ms/user_id/error_code/error_detail
- Traces 同页签查看（handler + D1 span，保留 7 天）
- 调整级别：改 `wrangler.toml` 对应 env 的 `LOG_LEVEL`（EnvFilter 语法，如 `info,api=debug`）后重新部署
- 调整采样：`[observability] head_sampling_rate`
- 免费额度（2026-12-01 起）：logs 与 traces 共用 0.5 GB/天，Free 超限停止摄入、不产生费用

## D1 读复制与放置

```bash
npx wrangler d1 info sub-converter-db-prod --json   # 查看 read_replication.mode 与 running_in_region
```

- 读复制通过 Dashboard（D1 → Settings → Enable Read Replication）或 REST API `PUT /accounts/{id}/d1/database/{db_id}`（body `{"read_replication":{"mode":"auto"}}`，需 D1:Edit）开启，`wrangler.toml` 无法配置；关闭后副本最长 24 小时停止服务
- 复制本身不额外计费；副本只服务读，写仍回主库
- Worker 请求日志的 `placement` 字段来自 `cf-placement` 响应头：`remote-XXX` 表示 Smart Placement 生效，`local-XXX` 表示就近执行；Placement 分析最多需要 15 分钟
- 若 D1 span 仍慢：在 traces 里对比 `cloudflare.colo` 与 `cloudflare.d1.response.served_by_region`/`served_by_primary`，并看 `sql_duration_ms` 区分网络与 SQL 时间

## Secret 轮换

`JWT_SECRET`：

```bash
openssl rand -base64 48 | npx wrangler secret put JWT_SECRET                    # dev（顶层）
openssl rand -base64 48 | npx wrangler secret put JWT_SECRET --env production   # 生产
```

- 轮换后所有已签发的 access token（≤15 分钟）立即失效，前端会自动走 refresh 换新
- refresh token 独立于该 secret（D1 存哈希），不受影响，用户无需重新登录

## 回滚

- 代码：`npx wrangler rollback --env production`（生产）、`npx wrangler rollback`（dev）回到上一个版本，或 revert 提交后 push 让 CI 重新部署
- 数据：迁移不可逆，回滚代码前确认旧代码与新 schema 兼容

## 自定义域名

`sub.yqxpro.com` 通过 `wrangler.toml` 的 `[[env.production.routes]] custom_domain = true` 管理，首次部署自动建 DNS 和证书。前提：`yqxpro.com` zone 与 Worker 在同一 Cloudflare 账号。改域名需要同步更新 `WEBAUTHN_RP_ID`/`WEBAUTHN_ORIGINS`，否则注册的 passkey 失效。

## 备份与排查

```bash
npx wrangler d1 export sub-converter-db-prod --remote --output backup.sql   # 备份
npx wrangler d1 execute <db> --remote --command "SELECT ..."                # 只读查询
npx wrangler tail --env production                                          # 实时日志（含内部错误详情）
```

订阅提取数据查询（只读）：

```bash
# 协议分布
npx wrangler d1 execute sub-converter-db-prod --remote --env production \
  --command "SELECT source_id, protocol, count(*) AS n FROM source_proxies GROUP BY source_id, protocol"
# 流量与计数
npx wrangler d1 execute sub-converter-db-prod --remote --env production \
  --command "SELECT source_id, proxy_count, group_count, rule_count, userinfo_upload, userinfo_download, userinfo_total, last_error FROM source_snapshots"
```

本地状态重置：删除 `.wrangler/state`，重新 `wrangler d1 migrations apply --local`，`.dev.vars` 保留。

## 常见问题

| 现象 | 原因 / 处理 |
|---|---|
| `/s/{secret}` 返回空配置或缺少某个源的节点 | 首次拉取尚无快照，或该源解析不出可用的 Clash 节点。查 `SELECT source_id, proxy_count, last_error FROM source_snapshots`；抓取完成后自动恢复，失败会保留旧快照 |
| 源返回 403 或内容异常 | 机场常按 User-Agent 分流。抓取固定用 `FlClash/v0.8.92 clash-verge Platform/macos`（ADR 0009）；源方策略变化时更新 `crates/subscription/src/infrastructure/fetch/http_fetcher.rs` 的常量并重新部署 |
| 日志出现 `Network connection lost` | 源站对 Cloudflare 出网偶发 TLS 抖动或拦截；租约 120s 后下次拉取会自动重试，持续失败考虑换源 |
| 源抓取长期失败但旧数据仍可用 | 预期行为：失败只记 `last_error`，不覆盖最后一次成功快照 |
| 订阅体超过 1.8 MB | 抓取层按 `TooLarge` 跳过（D1 单行约 2 MB），需要更大容量时改 gzip+base64 或迁 KV/R2 |
| 公开订阅内容不更新 | 源刷新成功后会自动重建发布订阅快照；管理端改源/组合会先失效缓存，下次拉取重建。可查 `SELECT publication_id, generated_at, length(content) FROM publication_snapshots` 确认时间 |
| 源列表不显示流量/节点分布 | 迁移后首次抓取前的旧快照没有提取数据，等下一次抓取；解析失败的源会保留旧摘要并在列表显示 `last_error` |
| 公开订阅缺少规则/分组/设置 | 内容由 `source_proxies`/`source_proxy_groups`/`source_config` 合成（ADR 0012），提取缺失时先输出节点、下次抓取补齐；查 `SELECT source_id, rule_count FROM source_snapshots` 与 `source_config` 是否落库 |
| 公开订阅里没有某类协议节点 | 现在全协议输出。仍缺失时查 `source_proxies` 是否落库，或看 `source_snapshots.last_error`（源站按 UA 分流/解析失败都会跳过） |
| 注册 500，日志 `iteration counts above 100000 are not supported` | 生产 WebCrypto PBKDF2 上限 100k，不能调高迭代（见 ADR 0003） |
| 登录后 cookie 没存下 | 本地请用 `http://localhost:8080`；`Secure` cookie 在非 localhost 的 http 下会被拒 |
| Passkey 注册/登录 422 | origin 不在 `WEBAUTHN_ORIGINS`，或 rpIdHash 与 `WEBAUTHN_RP_ID` 不匹配 |
| Passkey 注册成功但换域名后无法登录 | RP ID 变更会使已注册凭证失效，需重新注册 |
| `wrangler dev` 启动慢 | build 命令会同时构建前端（`trunk build --release`），只调 API 时会慢一些 |
| 本地 passkey 注册异常/`WEBAUTHN_RP_ID` 不对 | 顶层 `[vars]` 是线上 dev 值，本地需在 `.dev.vars` 覆盖 `WEBAUTHN_RP_ID=localhost`、`WEBAUTHN_ORIGINS=http://localhost:8080`（见 `.dev.vars.example`） |
| CI 跳过部署 | 缺少 `CLOUDFLARE_API_TOKEN` secret |
| 生产运行一直 pending | 正常，等待 `production` Environment 的 Required reviewers 审批；到 Actions 对应 run 点 Review deployments → Approve |
| 生产 run 被取消 | 同组有新推送，`cancel-in-progress` 取消了未审批的旧 run，只需审批最新一次；如取消发生在已审批执行阶段，迁移可能未完成，需人工确认后重跑 |
| 审批生产前 | 确认没有更新推送、只审批最新一次 run，避免审批后立即被后续推送取消 |
