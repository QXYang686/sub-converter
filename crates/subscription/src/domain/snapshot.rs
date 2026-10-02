use super::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    source_id: SourceId,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
}

impl SourceSnapshot {
    pub fn restore(
        source_id: SourceId,
        body: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
        fetched_at: i64,
    ) -> Self {
        Self {
            source_id,
            body,
            etag,
            last_modified,
            fetched_at,
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
        );
        assert_eq!(snapshot.source_id(), &source_id);
        assert_eq!(snapshot.body(), b"proxies: []");
        assert_eq!(snapshot.etag(), Some("etag"));
        assert_eq!(snapshot.last_modified(), None);
        assert_eq!(snapshot.fetched_at(), 1_700_000_000);
    }
}
