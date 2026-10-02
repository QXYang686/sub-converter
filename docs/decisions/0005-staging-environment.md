# 0005 增加 staging 环境

日期：2026-10-02
状态：已采纳

## 背景

此前功能验证（注册、登录、清理）直接在生产 Worker 和生产 D1 上执行，存在污染数据、误操作和泄漏风险；生产库不应承载测试写入。

## 决定

- 新增 staging Worker `sub-converter-staging`，绑定远程 dev 库 `sub-converter-db-dev`，通过 workers.dev 域名访问
- 功能验证在 staging 进行；生产只做只读冒烟（首页 200、未登录 401、只读查询）
- staging 由手动 workflow `Deploy Staging` 或本地 `wrangler deploy --env staging` 部署
- 生产保持 push `main` 自动部署（含迁移），暂不加审批

## 后果

- 多一套环境需要维护，dev 库同时承担本地远程和 staging 数据
- 生产迁移仍随每次 push 自动执行，风险由"向后兼容迁移"规范约束（见 runbook）
- 如需更强保护，可后续为生产加 GitHub Environment 审批
