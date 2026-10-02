use std::fmt;

use uuid::Uuid;

use super::DomainError;

pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainError::InvalidUserId)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        let length = trimmed.chars().count();
        let valid_length = (USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&length);
        let valid_chars = trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid_length || !valid_chars {
            return Err(DomainError::InvalidUsername);
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_round_trips_through_string() {
        let id = UserId::new();
        let parsed = UserId::parse(&id.to_string()).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn user_id_rejects_invalid_input() {
        assert_eq!(UserId::parse("not-a-uuid"), Err(DomainError::InvalidUserId));
    }

    #[test]
    fn username_is_trimmed_and_lowercased() {
        let username = Username::new("  Alice-01 ").unwrap();
        assert_eq!(username.value(), "alice-01");
    }

    #[test]
    fn username_rejects_invalid_values() {
        let too_short = "ab";
        let too_long = "x".repeat(USERNAME_MAX_LEN + 1);
        let cases = ["", too_short, too_long.as_str(), "a b", "a@b", "用户"];
        for raw in cases {
            assert_eq!(
                Username::new(raw),
                Err(DomainError::InvalidUsername),
                "expected {raw:?} to be rejected"
            );
        }
    }
}
