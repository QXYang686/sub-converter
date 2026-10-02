pub mod auth;
pub mod passkey;
pub mod subscription;
pub mod user;

pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;
pub const PASSWORD_MIN_LEN: usize = 8;
pub const PASSWORD_MAX_LEN: usize = 72;
pub const SUBSCRIPTION_NAME_MIN_LEN: usize = 1;
pub const SUBSCRIPTION_NAME_MAX_LEN: usize = 64;
pub const SOURCE_URL_MAX_LEN: usize = 2048;
pub const PUBLICATION_MAX_SOURCES: usize = 50;
