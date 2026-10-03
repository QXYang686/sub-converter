use super::RuleSetId;

/// FNV-1a 内容指纹，用于判断远程内容是否变化。
pub fn body_hash(body: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in body {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 规则集的当前内容与抓取元数据；与 `RuleSet` 一一对应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetContent {
    rule_set_id: RuleSetId,
    body: Option<Vec<u8>>,
    etag: Option<String>,
    last_modified: Option<String>,
    updated_at: i64,
    body_hash: String,
    rule_count: u32,
    last_error: Option<String>,
    pinned: bool,
}

impl RuleSetContent {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        rule_set_id: RuleSetId,
        body: Option<Vec<u8>>,
        etag: Option<String>,
        last_modified: Option<String>,
        updated_at: i64,
        body_hash: String,
        rule_count: u32,
        last_error: Option<String>,
        pinned: bool,
    ) -> Self {
        Self {
            rule_set_id,
            body,
            etag,
            last_modified,
            updated_at,
            body_hash,
            rule_count,
            last_error,
            pinned,
        }
    }

    pub fn empty(rule_set_id: RuleSetId) -> Self {
        Self {
            rule_set_id,
            body: None,
            etag: None,
            last_modified: None,
            updated_at: 0,
            body_hash: String::new(),
            rule_count: 0,
            last_error: None,
            pinned: false,
        }
    }

    /// 写入新内容（远程抓取或用户手写），并清除错误。
    pub fn set_body(&mut self, body: Vec<u8>, rule_count: u32, now: i64) {
        self.body_hash = body_hash(&body);
        self.body = Some(body);
        self.rule_count = rule_count;
        self.updated_at = now;
        self.last_error = None;
    }

    /// 上游 304：只更新时间并清除错误。
    pub fn mark_refreshed(&mut self, now: i64) {
        self.updated_at = now;
        self.last_error = None;
    }

    pub fn set_error(&mut self, error: &str) {
        self.last_error = Some(error.to_string());
    }

    pub fn clear_body(&mut self) {
        self.body = None;
        self.etag = None;
        self.last_modified = None;
        self.body_hash = String::new();
        self.rule_count = 0;
    }

    pub fn set_validators(&mut self, etag: Option<String>, last_modified: Option<String>) {
        self.etag = etag;
        self.last_modified = last_modified;
    }

    pub fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
    }

    pub fn rule_set_id(&self) -> &RuleSetId {
        &self.rule_set_id
    }

    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn body_hash(&self) -> &str {
        &self.body_hash
    }

    pub fn rule_count(&self) -> u32 {
        self.rule_count
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn pinned(&self) -> bool {
        self.pinned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_hash_is_stable_and_content_sensitive() {
        assert_eq!(body_hash(b"abc"), body_hash(b"abc"));
        assert_ne!(body_hash(b"abc"), body_hash(b"abd"));
        assert_eq!(body_hash(b""), "cbf29ce484222325");
    }

    #[test]
    fn set_body_updates_hash_and_clears_error() {
        let mut content = RuleSetContent::empty(RuleSetId::new());
        content.set_error("boom");
        content.set_body(b"a\nb\n".to_vec(), 2, 1_700_000_000);
        assert_eq!(content.body(), Some(&b"a\nb\n"[..]));
        assert_eq!(content.rule_count(), 2);
        assert_eq!(content.updated_at(), 1_700_000_000);
        assert_eq!(content.body_hash(), body_hash(b"a\nb\n"));
        assert!(content.last_error().is_none());
    }

    #[test]
    fn mark_refreshed_keeps_body_and_clears_error() {
        let mut content = RuleSetContent::empty(RuleSetId::new());
        content.set_body(b"x".to_vec(), 1, 100);
        content.set_error("boom");
        content.mark_refreshed(200);
        assert_eq!(content.body(), Some(&b"x"[..]));
        assert_eq!(content.updated_at(), 200);
        assert!(content.last_error().is_none());
    }

    #[test]
    fn pinned_toggle_round_trips() {
        let mut content = RuleSetContent::empty(RuleSetId::new());
        assert!(!content.pinned());
        content.set_pinned(true);
        assert!(content.pinned());
    }
}
