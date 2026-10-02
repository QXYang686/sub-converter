pub mod application;
pub mod domain;
pub mod interfaces;

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;

#[cfg(target_arch = "wasm32")]
mod worker_entry;
