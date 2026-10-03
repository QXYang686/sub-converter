use super::SourceId;

/// 订阅源 `rule-providers` 中的一条声明：只有元数据，规则本体在外部。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractedRuleProvider {
    pub name: String,
    pub provider_type: Option<String>,
    pub behavior: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub interval: Option<i64>,
    pub options_json: String,
}

impl ExtractedRuleProvider {
    /// 只有声明为 http 且带 url 的 provider 才由服务端代为抓取快照。
    pub fn is_remote(&self) -> bool {
        self.provider_type.as_deref() == Some("http")
            && self
                .url
                .as_deref()
                .is_some_and(|url| !url.trim().is_empty())
    }
}

/// 服务端为某个 rule-provider 抓取的最近一次内容快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleProviderSnapshot {
    source_id: SourceId,
    name: String,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
    body_hash: String,
    rule_count: u32,
    last_error: Option<String>,
}

impl RuleProviderSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        source_id: SourceId,
        name: String,
        body: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
        fetched_at: i64,
        body_hash: String,
        rule_count: u32,
        last_error: Option<String>,
    ) -> Self {
        Self {
            source_id,
            name,
            body,
            etag,
            last_modified,
            fetched_at,
            body_hash,
            rule_count,
            last_error,
        }
    }

    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn name(&self) -> &str {
        &self.name
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

    pub fn body_hash(&self) -> &str {
        &self.body_hash
    }

    pub fn rule_count(&self) -> u32 {
        self.rule_count
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// 上游返回 304 时只更新抓取时间并清除错误。
    pub fn mark_refreshed(&mut self, fetched_at: i64) {
        self.fetched_at = fetched_at;
        self.last_error = None;
    }

    pub fn set_error(&mut self, error: &str) {
        self.last_error = Some(error.to_string());
    }
}

/// 统计 provider 内容里的规则条数，兼容 YAML `payload:` 列表与纯文本两种格式。
pub fn count_provider_rules(body: &[u8]) -> u32 {
    let text = String::from_utf8_lossy(body);
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("//")
            || line == "payload:"
            || line == "---"
        {
            continue;
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_requires_http_and_url() {
        let mut provider = ExtractedRuleProvider {
            provider_type: Some("http".to_string()),
            url: Some("https://example.com/reject.yaml".to_string()),
            ..ExtractedRuleProvider::default()
        };
        assert!(provider.is_remote());
        provider.provider_type = Some("file".to_string());
        assert!(!provider.is_remote());
        provider.provider_type = Some("http".to_string());
        provider.url = None;
        assert!(!provider.is_remote());
    }

    #[test]
    fn counts_lines_in_text_and_yaml_payloads() {
        let text = b"# comment\n\ngoogle.com\n+.github.com\n// trailing\n";
        assert_eq!(count_provider_rules(text), 2);
        let yaml = b"payload:\n  - google.com\n  - github.com\n";
        assert_eq!(count_provider_rules(yaml), 2);
    }
}
