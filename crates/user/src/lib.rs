pub mod application;
pub mod domain;

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
