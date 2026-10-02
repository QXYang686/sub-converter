pub mod domain;

pub use domain::{
    DomainError, RepositoryError, User, UserId, UserRepository, Username, USERNAME_MAX_LEN,
    USERNAME_MIN_LEN,
};

#[cfg(target_arch = "wasm32")]
pub mod infrastructure;
