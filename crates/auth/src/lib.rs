pub mod application;
pub mod domain;
pub mod webauthn;

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
