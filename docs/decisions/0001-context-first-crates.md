# 0001 按限界上下文拆分 crate

日期：2026-10-02
状态：已采纳

## 背景

项目一开始是单 crate 的 API，前端和后续的订阅转换功能会陆续加入。若继续平铺模块，领域层、认证、接口和 worker 专属代码之间没有编译期边界，容易互相渗透；未来按业务拆分（用户/认证/订阅转换）也会更贵。

## 决定

采用 context-first 拆分：

- `crates/user`：用户上下文（账号）
- `crates/auth`：认证上下文（凭证、会话、WebAuthn）
- `crates/contract`：前后端共享的 serde DTO 与约束常量（发布语言）
- `crates/api`：组合根，负责路由、错误映射、依赖注入与跨上下文编排
- `crates/web`：Leptos CSR 前端，只依赖 contract

依赖方向：`user ← auth ← api`，`contract ← api/web`，web 不依赖 user/auth。跨上下文流程由 api 编排（如注册：建 User + 建 Password 凭证，失败补偿删除）。

## 后果

- 边界由编译器强制，worker 依赖只存在于 wasm target
- 每个上下文拥有自己的 domain/application/infrastructure，后续新增上下文只需平行加目录
- 代价：跨上下文写入没有事务，注册需要补偿逻辑；crate 数量增加，构建和导航成本略升
