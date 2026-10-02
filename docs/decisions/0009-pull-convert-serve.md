# 0009 拉取、转换与公开订阅分发

日期：2026-10-03
状态：已采纳

## 背景

ADR 0006 交付了订阅源与发布订阅的管理 CRUD，本期补齐缺口：从外部源拉取、把多个源转换成 Clash YAML、通过 `/s/{secret}` 公开分发。真实机场存在按 User-Agent 分流、gzip/zstd 压缩、单源数百 KB、偶发 TLS 抖动等现实约束，需要在设计里定死行为。

实施前用 miniflare 与 remote edge worker 做了验证：

- worker-rs `D1Type::Blob` 经 `serde_wasm_bindgen` 序列化为 JS `Array`，D1 仍将其落成真正的 BLOB，读回为数组，`Vec<u8>` 可反序列化
- `Context::wait_until` 在响应返回后仍会执行任务（抓取与写库可异步）
- 两个真实源在 Cloudflare 边缘均能用固定 UA 抓取成功（221 KB / 713 KB，解压后分别解析出 44 / 48 个受支持节点）

## 决定

**触发与快照**

- 源创建、修改 URL、重新启用时触发一次后台抓取；`GET /s/{secret}` 每次都会为所有绑定且启用的源排一次后台刷新（不做 TTL，也不提供手动刷新端点）
- 响应始终使用当前快照，因此最多滞后一次请求；无快照时返回合法空配置（`proxies: []` + `DIRECT`）并触发抓取，客户端下次拉取即可拿到数据
- 快照表 `source_snapshots(source_id PK, body BLOB, etag, last_modified, fetched_at, refresh_lease_until, last_error)`，存上游原始 body，解析与渲染放在响应时
- 刷新前用 `refresh_lease_until` 做 CAS 抢租约（代码常量 120s），抢不到直接跳过，避免并发刷新风暴；抓取失败或解析不出受支持节点时保留旧快照、只记录 `last_error`
- 条件请求：带上次的 `If-None-Match`/`If-Modified-Since`，304 只更新 `fetched_at`
- 单源 body 上限 1.8 MB（抓取层限制），给 D1 单行约 2 MB 留余量

**UA 与压缩**

- 抓取固定使用 `FlClash/v0.8.92 clash-verge Platform/macos`：机场按 UA 返回不同配置或 403，该 UA 实测同时通过两个真实源
- 复用 Workers fetch 自带的响应解压，不自行处理 gzip/zstd

**转换（Clash → Clash）**

- 输入只支持 Clash/Mihomo YAML：解析根级 `proxies`，只保留 `vmess`、`anytls`、`hysteria2`，其余协议与畸形项计入 skipped（源无数据时跳过）
- 每个节点保留原始 mapping（未知字段如 `reality-opts`、`ws-opts`、`ports` 零成本透传），身份键为 `协议+server+port+凭证`（vmess=uuid，其余=password）
- 按发布订阅中源的 position 顺序合并，身份去重（先到先得），代理名冲突追加 ` 2`、` 3`
- 渲染最小完整配置：`mode: rule`、`proxies`、一个 `select` 组（末尾回退 `DIRECT`）、`MATCH,PROXY`

**分发**

- `GET /s/{secret}?target=clash`：`target` 缺省 clash，非法值 400；未知 secret、`enabled=false`、已过期统一 404，不泄露存在性
- 响应头：`Content-Type: text/yaml; charset=utf-8`、`Content-Disposition`（ASCII fallback + RFC 5987 UTF-8 文件名）、`Profile-Update-Interval: 24`、`Cache-Control: no-store`（保证每次拉取都进 Worker 触发刷新）
- `wrangler.toml` 的 `run_worker_first` 增加 `/s/*`，避免被 SPA 回退返回 HTML
- secret 与从上游抓到的订阅内容绝不入日志

## 后果

- 公开拉取的成本是绑定启用源数量的后台 subrequest；租约只解决并发去重，同一客户端连续拉取仍会重复抓取，取决于后续是否收紧
- 2 MB 行上限由 1.8 MB 抓取上限兜底；若未来出现更大源，需要 gzip+base64 或改用 KV/R2
- 原始 mapping 透传意味着不做协议字段的结构化校验；将来支持 base64/sing-box 输出时需要引入结构化 `Node` 模型
- 解析与合并是纯函数，原生单测覆盖；真实数据验证用临时 example 完成，不把任何订阅内容提交进仓库
