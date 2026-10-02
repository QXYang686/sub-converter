# 0010 订阅内容的完整提取与存储

日期：2026-10-03
状态：已采纳

## 背景

ADR 0009 只把上游原始 body 存进 `source_snapshots`，公开分发时仅保留 vmess/anytls/hysteria2 三种协议，其余节点被丢弃，响应头（流量、到期等）也没有采集。真实机场的订阅里还有 ss/trojan 等节点、proxy-groups、成百上千条规则、dns/hosts/experimental 等设置，以及 `subscription-userinfo`/`profile-update-interval`/`profile-web-page-url`/`content-disposition` 等元数据。这些都需要落库并在管理端可见。

## 决定

**全协议保留**

- 公开分发与合并不再做协议白名单，任意 Clash 代理类型原样输出
- 去重身份从「协议+凭证」改为「去掉 `name` 后的规范化 JSON」，对未知协议同样生效；缺失 name 时按 `server:port` 或 `协议 N` 合成，名字冲突仍追加序号

**提取内容**

- `proxies`：每个节点提取 protocol/name/server/port，完整字段以 `options` JSON 保存（含未知字段与凭证）
- `proxy-groups`：name/type/proxies 引用 + 完整字段 JSON
- `rules`：规则字符串数组（源可达数千条）
- 顶层其余键（dns/tun/experimental/hosts/rule-providers/...）：整体 settings JSON
- 响应头：`subscription-userinfo` 解析为 upload/download/total/expire，另存 `profile-update-interval`、`profile-web-page-url`、`content-disposition` 中的机场名

**存储**

- `source_snapshots` 扩列：`body_hash`、`proxy_count`、`group_count`、`rule_count`、`protocol_counts`(JSON)、`userinfo_*`、`update_interval`、`provider_name`、`provider_url`
- 新表：`source_proxies(source_id, ordinal, protocol, name, server, port, options)`、`source_proxy_groups(source_id, ordinal, name, group_type, proxies, options)`、`source_config(source_id, rules, settings)`
- 外键级联 + 删除源时显式清理；节点/分组逐行可查询，规则/设置整行 JSON 避免数千行写放大

**刷新语义**

- 200 且 body 哈希未变：只更新响应头/抓取时间，跳过结构化重写；304 只更新 `fetched_at`
- 哈希变化：一个批处理内替换快照 meta 与三张提取表
- 解析失败或无可用户节点：保留旧快照与旧提取数据，只记 `last_error`

**管理与展示**

- `SourceResponse.snapshot` 只读对象返回计数、协议分布、流量四项、到期、更新间隔、机场名、最近错误
- 前端源列表展示流量进度、到期、节点/分组/规则数与协议分布、最近错误

## 后果

- 写放大受 body 哈希控制，稳定订阅基本只写 meta；大订阅首次/变更时一次批量写约百条节点 + 数条分组 + 一行规则 JSON
- 规则与设置整行 JSON 受 D1 单行 2 MB 限制；当前真实源约数百 KB
- `options` 含凭证，敏感级别等同 body，绝不入日志与仓库
- 公开分发输出包含客户端可能不支持的新协议，由客户端自行忽略；将来支持 base64/sing-box 输出时需要把 `options` 进一步结构化
