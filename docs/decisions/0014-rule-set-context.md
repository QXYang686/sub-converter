# 0014 独立的规则集上下文

日期：2026-10-03
状态：已采纳

## 背景

订阅上下文里的 rule-provider 一直是**从订阅源配置抽取、只读**的：刷新源时解析上游 YAML 的 `rule-providers`，落库并在管理端查看，用户无法自建或修改。需求是让用户能够自己管理具名规则集。

讨论中确认了两条边界：

- 规则集是跨生态的通用概念（Clash `rule-providers`、sing-box `rule_set`、Surge ruleset、QX `filter_remote`），`provider` 只是 Clash 的说法，且与 Clash 的 `proxy-providers`（节点集）同名易混
- 独立的规则集**要真正生效必须被规则引用**，而这属于"自定义规则编排"，与源/发布订阅的集成是两个阶段

因此本期把它做成一个**独立限界上下文**，不与 subscription 发生依赖，也不做任何目标格式的渲染。

## 决定

**新上下文 `crates/rule-set`**

- 依赖方向 `user ← rule_set ← api`，与 subscription 零依赖；`api` 作为组合根平级挂路由
- 拥有表 `rule_sets`、`rule_set_contents`
- `infrastructure`（D1 仓储、HTTP 抓取、时钟）仅在 `wasm32` 下编译

**格式中性的领域内核**

- 聚合 `RuleSet`：`name`、来源 `RuleSetSource { Remote(url) | Inline | Local(path) }`、`Option<RuleSetCategory { Domain | Ip | Mixed }>`、`content_format { Yaml | Text | SourceJson | Binary | Adblock }`、`Option<RuleSetInterval>`、`enabled`、时间戳
- 实体 `RuleSetContent`（与聚合 1:1）：`body`、`etag`、`last_modified`、`updated_at`、`body_hash`、`rule_count`、`last_error`、`pinned`
- 命名用 `RuleSet` 而非 `RuleProvider`；`provider` 仅保留为将来 Clash 投影的用语
- Clash 专有的 `behavior`/`path`/`type: file`、以及 `RULE-SET` 引用语法**不进内核**，留待投影层

**抓取与内容**

- 创建 `Remote` 与手动刷新时**同步**抓取，条件请求（ETag/Last-Modified），304 只更新时间；失败保留旧内容并记录 `last_error`
- `interval` 本期仅作元数据，不加 cron 调度
- `pinned` 的手动内容不被远程刷新覆盖；`Inline` 创建即 `pinned`
- `Local` 仅表示客户端本地路径，服务端不抓取、不产生内容

**API**

```
GET/POST         /api/rule-sets
GET/PATCH/DELETE /api/rule-sets/{id}
POST             /api/rule-sets/{id}/refresh
GET/PUT/PATCH    /api/rule-sets/{id}/content    # GET 原文；PUT 覆盖并钉住；PATCH 切换 pinned
```

**明确不做（本期）**

- 任何格式渲染/投影（Clash/sing-box/Surge）
- 与 subscription / publication / source 的依赖或集成
- 定时刷新、自定义分组、`RULE-SET` 规则引用

## 后果

- 本期交付"能自建、能抓、能看、能改"，但**没有消费者**：自建规则集不会进入任何发布订阅输出
- 短期内存在两套 rule-provider 概念：subscription 的 `ExtractedRuleProvider`（源派生）与本上下文的 `RuleSet`（用户自建）；集成阶段由 `api` 编排合并
- 用户可控 URL 的抓取带来 SSRF 面，与订阅源抓取同源；抓取层沿用 15s 超时与 1.8 MB 上限，私网段/DNS rebinding 防护属已知边界
- 内容受 D1 单行约 2 MB 限制，超出需转 KV/R2（本期不做）
- 内容格式枚举预留 `Adblock`，但暂不投影；"一个规则集对应多格式内容"是已知将来项，本期维持一对一
