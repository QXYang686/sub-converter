use std::fmt;

use user::UserId;
use uuid::Uuid;

use super::DomainError;

pub const PASSWORD_MIN_LEN: usize = 8;
pub const PASSWORD_MAX_LEN: usize = 72;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialId(Uuid);

impl CredentialId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainError::InvalidCredentialId)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for CredentialId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CredentialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DomainError::InvalidPasswordHash);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn validate_password(password: &str) -> Result<(), DomainError> {
    let length = password.chars().count();
    if (PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&length) {
        Ok(())
    } else {
        Err(DomainError::InvalidPassword)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passkey {
    credential_id: String,
    public_key: String,
    sign_count: u32,
    transports: Option<String>,
}

impl Passkey {
    pub fn new(
        credential_id: String,
        public_key: String,
        sign_count: u32,
        transports: Option<String>,
    ) -> Self {
        Self {
            credential_id,
            public_key,
            sign_count,
            transports,
        }
    }

    pub fn credential_id(&self) -> &str {
        &self.credential_id
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn sign_count(&self) -> u32 {
        self.sign_count
    }

    pub fn transports(&self) -> Option<&str> {
        self.transports.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSecret {
    Password(PasswordHash),
    Passkey(Passkey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    id: CredentialId,
    user_id: UserId,
    label: Option<String>,
    created_at: i64,
    last_used_at: Option<i64>,
    secret: CredentialSecret,
}

impl Credential {
    pub fn password(id: CredentialId, user_id: UserId, password_hash: PasswordHash, now: i64) -> Self {
        Self {
            id,
            user_id,
            label: None,
            created_at: now,
            last_used_at: None,
            secret: CredentialSecret::Password(password_hash),
        }
    }

    pub fn new_passkey(
        id: CredentialId,
        user_id: UserId,
        passkey: Passkey,
        label: Option<String>,
        now: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            label,
            created_at: now,
            last_used_at: None,
            secret: CredentialSecret::Passkey(passkey),
        }
    }

    pub fn restore(
        id: CredentialId,
        user_id: UserId,
        label: Option<String>,
        created_at: i64,
        last_used_at: Option<i64>,
        secret: CredentialSecret,
    ) -> Self {
        Self {
            id,
            user_id,
            label,
            created_at,
            last_used_at,
            secret,
        }
    }

    pub fn mark_used(&mut self, now: i64) {
        self.last_used_at = Some(now);
    }

    pub fn set_sign_count(&mut self, sign_count: u32) {
        if let CredentialSecret::Passkey(passkey) = &mut self.secret {
            passkey.sign_count = sign_count;
        }
    }

    pub fn id(&self) -> &CredentialId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn last_used_at(&self) -> Option<i64> {
        self.last_used_at
    }

    pub fn secret(&self) -> &CredentialSecret {
        &self.secret
    }

    pub fn password_hash(&self) -> Option<&PasswordHash> {
        match &self.secret {
            CredentialSecret::Password(hash) => Some(hash),
            CredentialSecret::Passkey(_) => None,
        }
    }

    pub fn passkey(&self) -> Option<&Passkey> {
        match &self.secret {
            CredentialSecret::Password(_) => None,
            CredentialSecret::Passkey(passkey) => Some(passkey),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_id_round_trips_through_string() {
        let id = CredentialId::new();
        assert_eq!(CredentialId::parse(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn credential_id_rejects_invalid_input() {
        assert_eq!(
            CredentialId::parse("not-a-uuid"),
            Err(DomainError::InvalidCredentialId)
        );
    }

    #[test]
    fn password_hash_rejects_empty_value() {
        assert_eq!(
            PasswordHash::new(""),
            Err(DomainError::InvalidPasswordHash)
        );
    }

    #[test]
    fn password_policy_enforces_length_bounds() {
        assert_eq!(
            validate_password("short"),
            Err(DomainError::InvalidPassword)
        );
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password(&"x".repeat(PASSWORD_MAX_LEN)).is_ok());
        assert_eq!(
            validate_password(&"x".repeat(PASSWORD_MAX_LEN + 1)),
            Err(DomainError::InvalidPassword)
        );
    }

    #[test]
    fn password_credential_mark_used_records_timestamp() {
        let mut credential = Credential::password(
            CredentialId::new(),
            UserId::new(),
            PasswordHash::new("$hash$").unwrap(),
            1_000,
        );
        assert_eq!(credential.last_used_at(), None);
        credential.mark_used(2_000);
        assert_eq!(credential.last_used_at(), Some(2_000));
    }

    #[test]
    fn passkey_credential_updates_sign_count() {
        let mut credential = Credential::new_passkey(
            CredentialId::new(),
            UserId::new(),
            Passkey::new("cred-1".to_string(), "key".to_string(), 0, None),
            Some("iPhone".to_string()),
            1_000,
        );
        assert_eq!(credential.passkey().unwrap().sign_count(), 0);
        credential.set_sign_count(7);
        assert_eq!(credential.passkey().unwrap().sign_count(), 7);
        assert!(credential.password_hash().is_none());
    }
}
