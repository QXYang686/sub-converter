use super::{PublicationId, SubscriptionUserInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationSnapshot {
    publication_id: PublicationId,
    format: String,
    content: String,
    userinfo: SubscriptionUserInfo,
    generated_at: i64,
}

impl PublicationSnapshot {
    pub fn restore(
        publication_id: PublicationId,
        format: String,
        content: String,
        userinfo: SubscriptionUserInfo,
        generated_at: i64,
    ) -> Self {
        Self {
            publication_id,
            format,
            content,
            userinfo,
            generated_at,
        }
    }

    pub fn publication_id(&self) -> &PublicationId {
        &self.publication_id
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn userinfo(&self) -> &SubscriptionUserInfo {
        &self.userinfo
    }

    pub fn generated_at(&self) -> i64 {
        self.generated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_keeps_state() {
        let publication_id = PublicationId::new();
        let snapshot = PublicationSnapshot::restore(
            publication_id.clone(),
            "clash".to_string(),
            "mode: rule".to_string(),
            SubscriptionUserInfo {
                upload: Some(1),
                ..SubscriptionUserInfo::default()
            },
            1_700_000_000,
        );
        assert_eq!(snapshot.publication_id(), &publication_id);
        assert_eq!(snapshot.format(), "clash");
        assert_eq!(snapshot.content(), "mode: rule");
        assert_eq!(snapshot.userinfo().upload, Some(1));
        assert_eq!(snapshot.generated_at(), 1_700_000_000);
    }
}
