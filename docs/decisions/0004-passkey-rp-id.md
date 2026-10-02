# 0004 Passkey 使用 RP ID yqxpro.com

日期：2026-10-02
状态：已采纳

## 背景

WebAuthn 凭证绑定到 Relying Party ID，RP ID 必须是访问 origin 的可注册域后缀。生产域名 `sub.yqxpro.com`、staging 为 `*.yqxpro.workers.dev`、本地为 `localhost`，三者不能用同一个 RP ID，且 RP ID 一旦变更，已注册的 passkey 全部失效。

## 决定

- 生产 RP ID 使用 `yqxpro.com`，允许 `yqxpro.com` 及其所有子域复用凭证（origin 白名单为 `https://sub.yqxpro.com`）
- staging 使用完整 workers.dev 主机名，本地使用 `localhost`
- RP ID 与允许 origin 按环境配置（`WEBAUTHN_RP_ID` / `WEBAUTHN_ORIGINS`），不写死在代码
- 验签用纯 Rust 实现（`ciborium` + `p256`/ES256 + `ed25519-dalek`），只接受 `attestation: none`，支持 ES256/Ed25519
- 本地测试需通过 `http://localhost:8080` 访问（127.0.0.1 与 RP ID 不匹配）

## 后果

- 任一 `yqxpro.com` 子域被攻破都可能用于请求该 RP ID 下的凭证，这是选择父域换取复用的代价；如需最小暴露面可改为 `sub.yqxpro.com`（会导致现有 passkey 失效）
- 不同环境无法共享 passkey，staging/本地需要各自注册
- sign_count 为 0 的同步型 passkey 不做硬拒绝，仅在双方非零且回退时判定克隆
