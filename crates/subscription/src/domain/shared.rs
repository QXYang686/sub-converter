use std::fmt;

use super::DomainError;

pub const SUBSCRIPTION_NAME_MIN_LEN: usize = 1;
pub const SUBSCRIPTION_NAME_MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionName(String);

impl SubscriptionName {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        let length = trimmed.chars().count();
        let valid_length =
            (SUBSCRIPTION_NAME_MIN_LEN..=SUBSCRIPTION_NAME_MAX_LEN).contains(&length);
        let valid_chars = !trimmed.chars().any(char::is_control);
        if !valid_length || !valid_chars {
            return Err(DomainError::InvalidSubscriptionName);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubscriptionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_name_is_trimmed() {
        let name = SubscriptionName::new("  我的订阅  ").unwrap();
        assert_eq!(name.value(), "我的订阅");
    }

    #[test]
    fn subscription_name_rejects_invalid_values() {
        let too_long = "x".repeat(SUBSCRIPTION_NAME_MAX_LEN + 1);
        let cases = ["", "  ", too_long.as_str(), "bad\nname"];
        for raw in cases {
            assert_eq!(
                SubscriptionName::new(raw),
                Err(DomainError::InvalidSubscriptionName),
                "expected {raw:?} to be rejected"
            );
        }
    }
}
