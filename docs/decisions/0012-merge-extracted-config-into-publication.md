# 0012 公开分发基于提取数据合成完整配置

日期：2026-10-03
状态：已采纳

## 背景

ADR 0010 已把 `proxies`/`proxy-groups`/`rules`/顶层设置结构化落库，但 ADR 0011 的渲染快照仍从原始 body 重新解析，且合成逻辑只保留节点：输出固定为一个 `PROXY` select 组和 `MATCH,PROXY`，源的 proxy-groups、数千条 rules、dns/hosts 等设置全部丢失。真实机场订阅的核心价值恰在这些规则与设置上，客户端拿到“只有节点”的配置后分流策略失效。

## 决定

**渲染输入改为提取数据**

- `SnapshotRepository` 新增 `list_extractions(source_ids)`，从 `source_snapshots` 读 userinfo、从 `source_proxies`/`source_proxy_groups`/`source_config` 读结构化配置，返回 `SourceExtraction { source_id, userinfo, config }`
- 公开分发与重建的缓存未命中路径不再读取源 body，也不再解析 YAML；源启用状态与绑定顺序仍由 `SourceRepository` 决定

**合并语义（按发布订阅中源的顺序）**

- 节点：沿用「身份去重、先到先得、同名追加序号」；被去重节点的名字会被重映射到存活节点
- 分组：整组保留，跨源组名冲突追加序号；组内 `proxies` 引用、组引用的其它组名、规则目标都会跟随改名重映射
- 规则：按源顺序拼接（完全相同的规则去重）；丢弃源自身的 `MATCH`/`FINAL` 兜底规则，末尾统一追加 `MATCH,<兜底组>`；目标改写识别 `no-resolve` 后缀，逻辑规则取最后一个逗号段
- Provider：`rule-providers`/`proxy-providers` 按源合并，名字冲突追加序号；`RULE-SET` 的 payload 与分组 `use` 列表跟随改名重映射，避免第二个源的同名 provider 被第一个源覆盖
- 设置：按源顺序浅合并，先到先得的键优先；`mode` 强制为 `rule`
- 兜底组：源里存在名为 `PROXY` 的组时直接用它；否则合成一个 `select` 组并置于首位，成员为全部组名 + 全部节点 + `DIRECT`（无组时仅节点 + `DIRECT`），保证空配置仍是合法的最小 Clash 配置

**缓存失效与提取自愈**

- 迁移 `0009` 清空 `publication_snapshots`：旧快照是按旧语义渲染的，部署后必须让下一次拉取内联重建
- 之后仍按 ADR 0011：源刷新成功或管理变更时重建原子级失效
- 提取是否可用以 `source_config` 行为准：200 且 body 未变但提取缺失时补写提取；304 且 `body_hash` 已存在但提取缺失时走 backfill 流程并重建发布订阅，避免旧代码或半失败写入留下「有 body 无结构化数据」的源

## 后果

- 客户端拿到的订阅包含源的分流策略；多源合并时组名/规则目标一致性由映射保证
- 输出体积从「仅节点」增长到接近单源整包，规则多的源叠加后需要关注 D1 单行 2 MB 限制（超限时按 ADR 0011 的后续方案压缩或转对象存储）
- 提取数据缺失（迁移后尚未抓取成功）的源在本次拉取中不贡献节点，表现为空配置，与既有冷启动语义一致，下一次抓取后自愈
- 设置浅合并意味着不同源的同名设置以绑定顺序靠前者为准，冲突不会报错
- 只有 `rule-providers`/`proxy-providers` 做了改名隔离；`sub-rules` 等其余引用型顶层键仍先到先得，属已知边界
