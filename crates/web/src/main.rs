//! Sub Converter 的 Leptos CSR 前端。
//!
//! - 只依赖 `contract`，通过 `/api` 与后端通信
//! - `state` 持有内存中的 access token 与当前用户，refresh token 走 HttpOnly cookie
//! - `pages`/`components` 为界面，`passkey`/`clipboard` 封装浏览器 API

mod api;
mod app;
mod clipboard;
mod components;
mod forms;
mod pages;
mod passkey;
mod state;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
