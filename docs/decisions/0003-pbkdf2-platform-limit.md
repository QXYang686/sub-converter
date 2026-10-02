# 0003 密码哈希采用 PBKDF2 100k（平台上限）

日期：2026-10-02
状态：已采纳

## 背景

Cloudflare Workers 免费计划每请求只有 10ms CPU，纯 Rust 的 Argon2id（推荐参数）在 wasm 下会超限，不可用。WebCrypto 的原生实现异步执行、不计入 CPU，是免费计划下唯一安全可行的路径，但生产环境的 PBKDF2 迭代次数上限为 100,000（本地 workerd 不限制，导致调试时未暴露）。

## 决定

- 使用 WebCrypto `crypto.subtle.deriveBits` 实现 PBKDF2-HMAC-SHA256，迭代 100k（平台上限），16B 随机盐，32B 输出
- 哈希按 PHC 格式 `$pbkdf2-sha256$i=100000$salt$hash` 存库，参数随哈希保存
- 保留 `PasswordHasher` 端口；将来升级付费计划可新增 Argon2id 适配器并按 PHC 前缀分发，老哈希仍可验证，可做登录时 rehash

## 后果

- 100k 低于 OWASP 对 PBKDF2-SHA256 的 600k 推荐，是平台约束下的取舍
- 参数入库使未来迁移无需重置密码
- 密码校验依赖 WebCrypto，无法在原生环境执行（相关代码在 `cfg(wasm32)` 下；纯逻辑单测用假实现）
