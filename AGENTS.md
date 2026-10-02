# AGENTS.md

## 项目
Cloudflare Workers API：Rust + worker-rs + axum + D1，DDD 分层用户系统。

## 命令
- 原生检查/测试（domain/application）：`cargo test`
- wasm 检查（infrastructure/interfaces）：`cargo check --target wasm32-unknown-unknown`
- 本地运行：`wrangler dev`；D1 迁移：`wrangler d1 migrations apply <db> --local`

## 架构约定
- 依赖方向：domain ← application ← infrastructure/interfaces
- domain 零框架依赖；worker 相关代码仅在 `cfg(target_arch = "wasm32")` 下编译
- 业务参数（TTL、迭代次数）用代码常量；仅密钥走 secret

## 工作方式
- 非必要不读写当前仓库外的内容
- 乐观执行：按约定直接实现，少读依赖源码，编译/测试报错再针对性排查
