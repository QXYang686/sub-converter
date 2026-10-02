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
pub struct PasswordCredential {
    id: CredentialId,
    user_id: UserId,
    password_hash: PasswordHash,
    created_at: i64,
    last_used_at: Option<i64>,
}

impl PasswordCredential {
    pub fn new(id: CredentialId, user_id: UserId, password_hash: PasswordHash, now: i64) -> Self {
        Self {
            id,
            user_id,
            password_hash,
            created_at: now,
            last_used_at: None,
        }
    }

    pub fn restore(
        id: CredentialId,
        user_id: UserId,
        password_hash: PasswordHash,
        created_at: i64,
        last_used_at: Option<i64>,
    ) -> Self {
        Self {
            id,
            user_id,
            password_hash,
            created_at,
            last_used_at,
        }
    }

    pub fn mark_used(&mut self, now: i64) {
        self.last_used_at = Some(now);
    }

    pub fn id(&self) -> &CredentialId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn last_used_at(&self) -> Option<i64> {
        self.last_used_at
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
    fn mark_used_records_timestamp() {
        let mut credential = PasswordCredential::new(
            CredentialId::new(),
            UserId::new(),
            PasswordHash::new("$hash$").unwrap(),
            1_000,
        );
        assert_eq!(credential.last_used_at(), None);
        credential.mark_used(2_000);
        assert_eq!(credential.last_used_at(), Some(2_000));
    }
}
