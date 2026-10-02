use user::UserId;

pub const CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeKind {
    PasskeyRegister,
    PasskeyLogin,
}

impl ChallengeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChallengeKind::PasskeyRegister => "passkey_register",
            ChallengeKind::PasskeyLogin => "passkey_login",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "passkey_register" => Some(ChallengeKind::PasskeyRegister),
            "passkey_login" => Some(ChallengeKind::PasskeyLogin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    challenge: String,
    user_id: Option<UserId>,
    kind: ChallengeKind,
    created_at: i64,
    expires_at: i64,
}

impl Challenge {
    pub fn new(
        challenge: String,
        user_id: Option<UserId>,
        kind: ChallengeKind,
        now: i64,
        ttl_seconds: i64,
    ) -> Self {
        Self {
            challenge,
            user_id,
            kind,
            created_at: now,
            expires_at: now + ttl_seconds,
        }
    }

    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    pub fn user_id(&self) -> Option<&UserId> {
        self.user_id.as_ref()
    }

    pub fn kind(&self) -> ChallengeKind {
        self.kind
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }

    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_through_string() {
        for kind in [ChallengeKind::PasskeyRegister, ChallengeKind::PasskeyLogin] {
            assert_eq!(ChallengeKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ChallengeKind::parse("unknown"), None);
    }

    #[test]
    fn expiry_uses_ttl() {
        let challenge = Challenge::new(
            "abc".to_string(),
            Some(UserId::new()),
            ChallengeKind::PasskeyRegister,
            1_000,
            CHALLENGE_TTL_SECONDS,
        );
        assert!(!challenge.is_expired(1_000 + CHALLENGE_TTL_SECONDS - 1));
        assert!(challenge.is_expired(1_000 + CHALLENGE_TTL_SECONDS));
    }
}
