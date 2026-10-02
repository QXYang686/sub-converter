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
