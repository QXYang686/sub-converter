# 0002 refresh token 走 httpOnly cookie

日期：2026-10-02
状态：已采纳

## 背景

前端需要在页面刷新后恢复会话。最初 API 把 access/refresh token 都放在响应体里，前端需要自行持久化 refresh token（localStorage）。localStorage 可被 XSS 读取，长期凭证一旦泄露，攻击者可在页面关闭后继续冒充用户。

## 决定

- refresh token 通过 `Set-Cookie` 下发：`HttpOnly; Secure; SameSite=Strict; Path=/api/auth; Max-Age=30天`
- 响应体不再包含 refresh token；`refresh`/`logout` 无请求体，从 cookie 读取
- access token 仅存内存（15 分钟），页面加载用 `POST /api/auth/refresh` 恢复会话
- `Path=/api/auth` 限制 cookie 只发给认证接口；`SameSite=Strict` 防跨站
- `refresh`/`logout` 额外要求 `X-Requested-With: XMLHttpRequest`，防同站子域 CSRF
- 本地/生产按请求 `x-forwarded-proto` 决定是否加 `Secure`

## 后果

- XSS 无法窃取长期凭证，但仍可在页面存活期间冒用会话；XSS 防护依然重要
- API 为破坏性变更：`AuthResponse` 去掉 refresh token，`RefreshRequest`/`LogoutRequest` 删除
- 前端简单了：不再存储/传递 refresh token，cookie 由浏览器管理
- 依赖同源部署；将来前端若跨域需要改为 CORS + `SameSite=None`
