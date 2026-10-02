# 0008 D1 读复制与 Worker 放置

日期：2026-10-03
状态：已采纳

## 背景

平台 traces 显示每个 D1 查询约 200ms+，而 `wrangler d1 insights` 显示 SQL 执行只有 0.3~1.5ms，说明耗时来自 Worker 与 D1 主库之间的网络往返。两个库的主区域都是 APAC，但请求可能落到远离主库的机房（冒烟时生产请求出现在 AMS）。

## 决定

- 两个 D1 库开启全局读复制（`read_replication.mode = auto`，通过 Dashboard 或 REST API 操作；`wrangler.toml` 没有该配置项）
- Worker 每个请求创建一个 D1 Session（`first-primary` 约束），同一请求内所有仓储共享：首个查询走主库，保证读到最新数据；后续读可由就近副本以 bookmark 保证顺序一致；写始终转发主库
- 仓储层持有 `Arc<D1DatabaseSession>` 而不是 `D1Database`，session 由组合根在 `worker_entry` 创建
- 各环境启用 Smart Placement（`[placement] mode = "smart"`），让 Worker 尽量贴近 D1 主库；`http.request` 日志记录 `cf-placement` 响应头便于验证

## 后果

- 跨请求的读己之写由 `first-primary` 保证；代价是每个请求的首个查询仍走主库
- 副本异步延迟不影响同一 Session 内的顺序一致性；读复制不额外计费（仍按 rows_read/rows_written）
- Smart Placement 需要多地域流量的持续分析（最长 15 分钟），且可能增加用户到 Worker 的 RTT
- 本地 `wrangler dev` 需要 Miniflare 支持 D1 Sessions；若某环境不支持，需要在组合根增加回退
