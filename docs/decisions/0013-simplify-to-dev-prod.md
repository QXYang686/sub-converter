# 0013 环境简化为 dev/prod 并加生产审批门禁

日期：2026-10-03
状态：已采纳
取代 [ADR 0005](0005-staging-environment.md) 的环境模型

## 背景

ADR 0005 引入 staging Worker `sub-converter-staging`，但它绑定的远程库就是本地 `--remote` 用的 `sub-converter-db-dev`。结果是三套命名互相错位——“dev 库 / staging worker / production worker”——且本地远程开发与 staging 共库，本地误操作会污染 staging。实际上不需要独立的 staging：dev 环境本身就能承担功能验证。同时原 `Deploy` 工作流没有引用 GitHub Environment，无法命中审批保护规则，push 后直接上线生产。

## 决定

**两套环境：dev 与 prod**

- dev：Worker `sub-converter-dev`，绑定远程 D1 `sub-converter-db-dev`，通过 `sub-converter-dev.yqxpro.workers.dev` 访问
- prod：Worker `sub-converter`，绑定远程 D1 `sub-converter-db-prod`，自定义域名 `sub.yqxpro.com`
- 移除 `[env.staging]` 与 `sub-converter-staging` Worker

**Wrangler 顶层默认节即 dev**

- `wrangler dev`（本地 SQLite）、`wrangler dev --remote`（线上 dev 库）、`wrangler deploy`（部署 dev）都走顶层配置
- 顶层 `[vars]` 存线上 dev 的值（workers.dev 域名）；本地用 `.dev.vars` 覆盖 `WEBAUTHN_RP_ID`/`WEBAUTHN_ORIGINS` 为 `localhost`
- `[env.production]` 保持独立配置

**CI**

- 每次 push `main` 自动部署 dev（测试 → 应用 dev 迁移 → `wrangler deploy`）
- 生产 job 声明 `environment: production`，命中 Required reviewers，人工 Approve 后才执行生产迁移与部署
- 两个工作流 `cancel-in-progress: true`，新推送取消同组的过期 run

## 后果

- 环境数与远程 D1 数一致（各 2 个），消除命名错位
- 本地 `--remote` 与线上 dev 共用 `sub-converter-db-dev`：有意为之，dev 数据可随时重建、允许被污染
- dev Worker 名与 RP ID 由 `sub-converter-staging...` 改为 `sub-converter-dev...`，dev 库中已注册的 passkey 失效需重新注册（dev 数据可弃）
- 首次切换需要人工：部署 `sub-converter-dev`、为顶层（dev）设置 `JWT_SECRET` secret、删除旧的 `sub-converter-staging` Worker
- 生产审批依赖仓库 Settings 中 `production` Environment 配置 Required reviewers（建议限制 Deployment branches 为 `main`）；私有仓库该功能需 GitHub Pro/Team
- `cancel-in-progress` 也会取消已审批且正在执行的生产 run；审批前应确认没有更新推送，只审批最新一次
