#[cfg(target_arch = "wasm32")]
mod jwt_token_service;
#[cfg(target_arch = "wasm32")]
mod pbkdf2_password_hasher;
#[cfg(target_arch = "wasm32")]
mod random;
#[cfg(target_arch = "wasm32")]
mod system_clock;
mod webauthn;

#[cfg(target_arch = "wasm32")]
pub use jwt_token_service::JwtTokenService;
#[cfg(target_arch = "wasm32")]
pub use pbkdf2_password_hasher::{Pbkdf2PasswordHasher, PBKDF2_ITERATIONS};
#[cfg(target_arch = "wasm32")]
pub use random::OsRandomSource;
#[cfg(target_arch = "wasm32")]
pub use system_clock::SystemClock;
pub use webauthn::RustWebAuthnVerifier;
