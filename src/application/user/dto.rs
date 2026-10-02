use crate::domain::user::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserView {
    pub id: String,
    pub username: String,
}

impl From<&User> for UserView {
    fn from(user: &User) -> Self {
        Self {
            id: user.id().to_string(),
            username: user.username().value().to_string(),
        }
    }
}
