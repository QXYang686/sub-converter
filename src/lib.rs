pub mod application;
pub mod domain;

#[cfg(target_arch = "wasm32")]
mod worker_entry;
