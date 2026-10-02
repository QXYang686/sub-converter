use super::{UserId, Username};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: UserId,
    username: Username,
    created_at: i64,
    updated_at: i64,
}

impl User {
    pub fn register(id: UserId, username: Username, now: i64) -> Self {
        Self {
            id,
            username,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn restore(id: UserId, username: Username, created_at: i64, updated_at: i64) -> Self {
        Self {
            id,
            username,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn username(&self) -> &Username {
        &self.username
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user(now: i64) -> User {
        User::register(UserId::new(), Username::new("alice").unwrap(), now)
    }

    #[test]
    fn register_initializes_timestamps() {
        let user = sample_user(1_700_000_000);
        assert_eq!(user.created_at(), 1_700_000_000);
        assert_eq!(user.updated_at(), 1_700_000_000);
    }

    #[test]
    fn restore_keeps_state() {
        let id = UserId::new();
        let username = Username::new("alice").unwrap();
        let user = User::restore(id.clone(), username.clone(), 100, 200);
        assert_eq!(user.id(), &id);
        assert_eq!(user.username(), &username);
        assert_eq!(user.created_at(), 100);
        assert_eq!(user.updated_at(), 200);
    }
}
