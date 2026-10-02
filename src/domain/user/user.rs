use super::{PasswordHash, UserId, Username};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: UserId,
    username: Username,
    password_hash: PasswordHash,
    created_at: i64,
    updated_at: i64,
}

impl User {
    pub fn register(id: UserId, username: Username, password_hash: PasswordHash, now: i64) -> Self {
        Self {
            id,
            username,
            password_hash,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn restore(
        id: UserId,
        username: Username,
        password_hash: PasswordHash,
        created_at: i64,
        updated_at: i64,
    ) -> Self {
        Self {
            id,
            username,
            password_hash,
            created_at,
            updated_at,
        }
    }

    pub fn change_password(&mut self, password_hash: PasswordHash, now: i64) {
        self.password_hash = password_hash;
        self.updated_at = now;
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn username(&self) -> &Username {
        &self.username
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
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
        User::register(
            UserId::new(),
            Username::new("alice").unwrap(),
            PasswordHash::new("$hash$").unwrap(),
            now,
        )
    }

    #[test]
    fn register_initializes_timestamps() {
        let user = sample_user(1_700_000_000);
        assert_eq!(user.created_at(), 1_700_000_000);
        assert_eq!(user.updated_at(), 1_700_000_000);
    }

    #[test]
    fn change_password_updates_hash_and_updated_at() {
        let mut user = sample_user(1_700_000_000);
        let new_hash = PasswordHash::new("$new-hash$").unwrap();

        user.change_password(new_hash.clone(), 1_700_000_100);

        assert_eq!(user.password_hash(), &new_hash);
        assert_eq!(user.created_at(), 1_700_000_000);
        assert_eq!(user.updated_at(), 1_700_000_100);
    }
}
