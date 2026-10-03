use std::collections::BTreeMap;

use super::SourceId;
use crate::domain::SubscriptionUserInfo;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnapshotMeta {
    pub body_hash: Option<String>,
    pub proxy_count: u32,
    pub group_count: u32,
    pub rule_count: u32,
    pub protocol_counts: BTreeMap<String, u32>,
    pub userinfo: SubscriptionUserInfo,
    pub update_interval: Option<i64>,
    pub provider_name: Option<String>,
    pub provider_url: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    source_id: SourceId,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
    meta: SnapshotMeta,
}

impl SourceSnapshot {
    pub fn restore(
        source_id: SourceId,
        body: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
        fetched_at: i64,
        meta: SnapshotMeta,
    ) -> Self {
        Self {
            source_id,
            body,
            etag,
            last_modified,
            fetched_at,
            meta,
        }
    }

    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    pub fn fetched_at(&self) -> i64 {
        self.fetched_at
    }

    pub fn meta(&self) -> &SnapshotMeta {
        &self.meta
    }
}

pub fn body_hash(body: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in body {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_keeps_state() {
        let source_id = SourceId::new();
        let snapshot = SourceSnapshot::restore(
            source_id.clone(),
            b"proxies: []".to_vec(),
            Some("etag".to_string()),
            None,
            1_700_000_000,
            SnapshotMeta::default(),
        );
        assert_eq!(snapshot.source_id(), &source_id);
        assert_eq!(snapshot.body(), b"proxies: []");
        assert_eq!(snapshot.etag(), Some("etag"));
        assert_eq!(snapshot.last_modified(), None);
        assert_eq!(snapshot.fetched_at(), 1_700_000_000);
        assert_eq!(snapshot.meta().proxy_count, 0);
    }

    #[test]
    fn body_hash_is_stable_and_content_sensitive() {
        assert_eq!(body_hash(b"abc"), body_hash(b"abc"));
        assert_ne!(body_hash(b"abc"), body_hash(b"abd"));
        assert_eq!(body_hash(b"abc").len(), 16);
    }
}
