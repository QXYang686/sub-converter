# 0007 可观测性：结构化日志与平台 traces

日期：2026-10-03
状态：已采纳

## 背景

此前仅在 500 响应时用 `console_error!` 打印一行内部错误，缺少请求上下文和关联 ID，线上排查依赖 `wrangler tail` 人工守候。Cloudflare 平台已提供免费可观测能力：Workers Logs 自动收集 console 输出并做结构化索引；Workers Traces（beta）自动为 handler、D1 binding 等平台操作生成 span；2026-12-01 前均不计费。

## 决定

- 业务 crate 统一使用 `tracing` 门面，日志实现只在组合根 `crates/api`（wasm-only），domain/application 不依赖 worker，native 测试下为 no-op
- `api` 的 `telemetry` 模块用 `tracing-subscriber`（`fmt + json + env-filter`）输出 JSON，经自定义 `MakeWriter` 走 `console.log`，由 Workers Logs 解析字段；`LOG_LEVEL` 环境变量控制级别（支持 EnvFilter 语法），默认 `info`
- 请求级 span `http.request` 记录 request_id、method、route（`MatchedPath` 模板）、user_id；完成时记录 status/latency_ms，并回写 `x-request-id` 响应头
- 级别策略：5xx = error（含内部错误详情），4xx = debug，成功 = info；响应体仍只返回通用错误消息
- application 用例统一 `#[tracing::instrument(skip_all)]`，不记录参数；password/hash/token/challenge/cookie 一律不入日志
- wrangler 各环境开启 `[observability]`（logs）与 `[observability.traces]`，采样率 1；2026-12-01 起 logs 与 traces 共用账号级免费额度（0.5 GB/天），用 `head_sampling_rate` 与 `LOG_LEVEL` 控制量
- 自定义业务 span（Rust 桥接 `ctx.tracing`）是 beta 试验，不在本期承诺范围内

## 后果

- 每请求至少 1 条结构化完成日志 + 1 条平台 invocation log；当前量级远低于免费额度，高流量时需复核 `head_sampling_rate`
- 平台 traces 只覆盖 handler、D1、fetch 等平台操作；业务步骤只能靠日志，如需业务级 span 需另评估 JS beta API 桥接
- wasm bundle 从约 1.29MB 增至约 2.07MB（gzip 约 0.65MB），仍远低于 Free 3MB 压缩上限；EnvFilter 的 regex 是主要增量，若需瘦身可换 `Targets`
- 日志时间戳由平台注入（wasm 无 `SystemTime`，JSON 不含 timestamp）；非 I/O 操作的耗时可能显示为 0ms（平台 Spectre 缓解所致）
