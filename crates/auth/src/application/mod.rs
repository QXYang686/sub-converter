pub mod config;
mod delete_passkey;
mod dto;
mod encoding;
mod error;
mod finish_passkey_login;
mod finish_passkey_registration;
mod get_current_user;
mod list_passkeys;
mod login;
mod logout;
mod ports;
mod refresh;
mod register;
mod start_passkey_login;
mod start_passkey_registration;

pub use delete_passkey::{DeletePasskeyCommand, DeletePasskeyHandler};
pub use dto::UserView;
pub use error::AppError;
pub use finish_passkey_login::{FinishPasskeyLoginCommand, FinishPasskeyLoginHandler};
pub use finish_passkey_registration::{
    FinishPasskeyRegistrationCommand, FinishPasskeyRegistrationHandler, RegisteredPasskey,
};
pub use get_current_user::GetCurrentUserHandler;
pub use list_passkeys::{ListPasskeysHandler, PasskeyView};
pub use login::{LoginCommand, LoginHandler, LoginResult};
pub use logout::{LogoutCommand, LogoutHandler};
pub use ports::{
    AccessToken, Clock, CollectedClientData, IssuedRefreshToken, PasskeyRegistration,
    PasswordHasher, PortError, RandomSource, RefreshTokenRepository, StoredRefreshToken,
    TokenService, WebAuthnVerifier,
};
pub use refresh::{RefreshCommand, RefreshHandler, RefreshResult};
pub use register::{RegisterCommand, RegisterHandler};
pub use start_passkey_login::{PasskeyLoginOptions, StartPasskeyLoginCommand, StartPasskeyLoginHandler};
pub use start_passkey_registration::{
    PasskeyRegistrationOptions, StartPasskeyRegistrationCommand, StartPasskeyRegistrationHandler,
};

#[cfg(test)]
mod tests;
