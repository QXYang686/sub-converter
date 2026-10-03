use std::fmt;

use user::UserId;
use uuid::Uuid;

use super::DomainError;

pub const RULE_SET_NAME_MIN_LEN: usize = 1;
pub const RULE_SET_NAME_MAX_LEN: usize = 64;
pub const RULE_SET_URL_MAX_LEN: usize = 2048;
pub const RULE_SET_PATH_MAX_LEN: usize = 512;
/// 刷新间隔上限：30 天。0 表示不自动刷新。
pub const RULE_SET_INTERVAL_MAX_SECS: i64 = 2_592_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleSetId(Uuid);

impl RuleSetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainError::InvalidRuleSetId)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for RuleSetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RuleSetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 具名标识：Clash `rule-providers` 键、sing-box `rule_set.tag`、QX `tag` 的统一抽象。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleSetName(String);

impl RuleSetName {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        let length = trimmed.chars().count();
        let valid_length = (RULE_SET_NAME_MIN_LEN..=RULE_SET_NAME_MAX_LEN).contains(&length);
        let valid_chars = !trimmed.chars().any(char::is_control);
        if !valid_length || !valid_chars {
            return Err(DomainError::InvalidRuleSetName);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleSetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 远程来源 URL，仅接受 http/https。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleSetUrl(String);

impl RuleSetUrl {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.len() > RULE_SET_URL_MAX_LEN
            || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DomainError::InvalidRuleSetUrl);
        }

        let lower = trimmed.to_ascii_lowercase();
        let rest = if lower.starts_with("https://") {
            &trimmed[8..]
        } else if lower.starts_with("http://") {
            &trimmed[7..]
        } else {
            return Err(DomainError::InvalidRuleSetUrl);
        };

        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.is_empty() || authority.contains('@') {
            return Err(DomainError::InvalidRuleSetUrl);
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleSetUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 客户端本地路径（`Local` 来源）。服务端只保存、不消费。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleSetPath(String);

impl RuleSetPath {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > RULE_SET_PATH_MAX_LEN
            || trimmed.chars().any(char::is_control)
        {
            return Err(DomainError::InvalidRuleSetPath);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleSetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 刷新间隔（秒），0 表示不自动刷新。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleSetInterval(i64);

impl RuleSetInterval {
    pub fn new(seconds: i64) -> Result<Self, DomainError> {
        if !(0..=RULE_SET_INTERVAL_MAX_SECS).contains(&seconds) {
            return Err(DomainError::InvalidRuleSetInterval);
        }
        Ok(Self(seconds))
    }

    pub fn seconds(&self) -> i64 {
        self.0
    }
}

/// 内容来源。`Local` 仅表示、不由服务端抓取。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSetSource {
    Remote(RuleSetUrl),
    Inline,
    Local(RuleSetPath),
}

impl RuleSetSource {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Remote(_) => "remote",
            Self::Inline => "inline",
            Self::Local(_) => "local",
        }
    }

    pub fn url(&self) -> Option<&RuleSetUrl> {
        match self {
            Self::Remote(url) => Some(url),
            _ => None,
        }
    }

    pub fn path(&self) -> Option<&RuleSetPath> {
        match self {
            Self::Local(path) => Some(path),
            _ => None,
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }
}

/// 分类，格式中性；允许缺省（sing-box / QX 等无此轴）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleSetCategory {
    Domain,
    Ip,
    Mixed,
}

impl RuleSetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Ip => "ip",
            Self::Mixed => "mixed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "domain" => Some(Self::Domain),
            "ip" => Some(Self::Ip),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

/// 内容序列化格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleSetContentFormat {
    Yaml,
    Text,
    SourceJson,
    Binary,
    Adblock,
}

impl RuleSetContentFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Text => "text",
            Self::SourceJson => "source-json",
            Self::Binary => "binary",
            Self::Adblock => "adblock",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "yaml" => Some(Self::Yaml),
            "text" => Some(Self::Text),
            "source-json" => Some(Self::SourceJson),
            "binary" => Some(Self::Binary),
            "adblock" => Some(Self::Adblock),
            _ => None,
        }
    }
}

/// 按内容格式统计规则条数。二进制格式无法统计，返回 0。
pub fn count_rules(format: RuleSetContentFormat, body: &[u8]) -> u32 {
    match format {
        RuleSetContentFormat::Binary => 0,
        RuleSetContentFormat::SourceJson => serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|value| value.as_array().map(Vec::len))
            .unwrap_or(0) as u32,
        RuleSetContentFormat::Adblock => count_lines(body, &["!", "["]),
        RuleSetContentFormat::Yaml | RuleSetContentFormat::Text => count_lines(body, &["#", "//"]),
    }
}

fn count_lines(body: &[u8], comment_prefixes: &[&str]) -> u32 {
    let text = String::from_utf8_lossy(body);
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line == "payload:"
            || line == "---"
            || comment_prefixes.iter().any(|prefix| line.starts_with(prefix))
        {
            continue;
        }
        count += 1;
    }
    count
}

/// 用户自建的具名规则集。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    id: RuleSetId,
    user_id: UserId,
    name: RuleSetName,
    source: RuleSetSource,
    category: Option<RuleSetCategory>,
    content_format: RuleSetContentFormat,
    interval: Option<RuleSetInterval>,
    enabled: bool,
    created_at: i64,
    updated_at: i64,
}

impl RuleSet {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        id: RuleSetId,
        user_id: UserId,
        name: RuleSetName,
        source: RuleSetSource,
        category: Option<RuleSetCategory>,
        content_format: RuleSetContentFormat,
        interval: Option<RuleSetInterval>,
        now: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            source,
            category,
            content_format,
            interval,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: RuleSetId,
        user_id: UserId,
        name: RuleSetName,
        source: RuleSetSource,
        category: Option<RuleSetCategory>,
        content_format: RuleSetContentFormat,
        interval: Option<RuleSetInterval>,
        enabled: bool,
        created_at: i64,
        updated_at: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            name,
            source,
            category,
            content_format,
            interval,
            enabled,
            created_at,
            updated_at,
        }
    }

    pub fn rename(&mut self, name: RuleSetName, now: i64) {
        if self.name != name {
            self.name = name;
            self.updated_at = now;
        }
    }

    pub fn change_source(&mut self, source: RuleSetSource, now: i64) {
        if self.source != source {
            self.source = source;
            self.updated_at = now;
        }
    }

    pub fn set_category(&mut self, category: Option<RuleSetCategory>, now: i64) {
        if self.category != category {
            self.category = category;
            self.updated_at = now;
        }
    }

    pub fn set_content_format(&mut self, format: RuleSetContentFormat, now: i64) {
        if self.content_format != format {
            self.content_format = format;
            self.updated_at = now;
        }
    }

    pub fn set_interval(&mut self, interval: Option<RuleSetInterval>, now: i64) {
        if self.interval != interval {
            self.interval = interval;
            self.updated_at = now;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, now: i64) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.updated_at = now;
        }
    }

    pub fn id(&self) -> &RuleSetId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn name(&self) -> &RuleSetName {
        &self.name
    }

    pub fn source(&self) -> &RuleSetSource {
        &self.source
    }

    pub fn category(&self) -> Option<RuleSetCategory> {
        self.category
    }

    pub fn content_format(&self) -> RuleSetContentFormat {
        self.content_format
    }

    pub fn interval(&self) -> Option<RuleSetInterval> {
        self.interval
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn is_remote(&self) -> bool {
        self.source.is_remote()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_set_id_round_trips_through_string() {
        let id = RuleSetId::new();
        assert_eq!(RuleSetId::parse(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn rule_set_id_rejects_invalid_input() {
        assert_eq!(
            RuleSetId::parse("not-a-uuid"),
            Err(DomainError::InvalidRuleSetId)
        );
    }

    #[test]
    fn name_is_trimmed_and_validated() {
        assert_eq!(RuleSetName::new("  reject  ").unwrap().value(), "reject");
        let too_long = "x".repeat(RULE_SET_NAME_MAX_LEN + 1);
        for raw in ["", "  ", too_long.as_str(), "bad\nname"] {
            assert_eq!(
                RuleSetName::new(raw),
                Err(DomainError::InvalidRuleSetName),
                "expected {raw:?} to be rejected"
            );
        }
    }

    #[test]
    fn url_accepts_http_and_https_only() {
        for raw in [
            "http://example.com/rules.yaml",
            "HTTPS://example.com/rules.list",
            " https://example.com/a?b=1#c ",
        ] {
            assert!(RuleSetUrl::new(raw).is_ok(), "expected {raw:?} accepted");
        }
        for raw in ["", "ftp://example.com", "example.com", "https://"] {
            assert_eq!(
                RuleSetUrl::new(raw),
                Err(DomainError::InvalidRuleSetUrl),
                "expected {raw:?} rejected"
            );
        }
    }

    #[test]
    fn path_rejects_empty_and_control() {
        assert_eq!(
            RuleSetPath::new("./ruleset/reject.yaml").unwrap().value(),
            "./ruleset/reject.yaml"
        );
        for raw in ["", "  ", "bad\tpath"] {
            assert_eq!(RuleSetPath::new(raw), Err(DomainError::InvalidRuleSetPath));
        }
    }

    #[test]
    fn interval_is_bounded() {
        assert_eq!(RuleSetInterval::new(0).unwrap().seconds(), 0);
        assert_eq!(
            RuleSetInterval::new(RULE_SET_INTERVAL_MAX_SECS)
                .unwrap()
                .seconds(),
            RULE_SET_INTERVAL_MAX_SECS
        );
        for raw in [-1, RULE_SET_INTERVAL_MAX_SECS + 1] {
            assert_eq!(
                RuleSetInterval::new(raw),
                Err(DomainError::InvalidRuleSetInterval)
            );
        }
    }

    #[test]
    fn enum_str_round_trips() {
        for category in [
            RuleSetCategory::Domain,
            RuleSetCategory::Ip,
            RuleSetCategory::Mixed,
        ] {
            assert_eq!(RuleSetCategory::parse(category.as_str()), Some(category));
        }
        for format in [
            RuleSetContentFormat::Yaml,
            RuleSetContentFormat::Text,
            RuleSetContentFormat::SourceJson,
            RuleSetContentFormat::Binary,
            RuleSetContentFormat::Adblock,
        ] {
            assert_eq!(RuleSetContentFormat::parse(format.as_str()), Some(format));
        }
    }

    #[test]
    fn counts_rules_for_supported_formats() {
        let text = b"# comment\n\ngoogle.com\n+.github.com\n// trailing\n";
        assert_eq!(count_rules(RuleSetContentFormat::Text, text), 2);
        let yaml = b"payload:\n  - google.com\n  - github.com\n";
        assert_eq!(count_rules(RuleSetContentFormat::Yaml, yaml), 2);
        let adblock = b"! title\n[Adblock Plus 2.0]\n||ads.example.com^\n";
        assert_eq!(count_rules(RuleSetContentFormat::Adblock, adblock), 1);
        let source_json = br#"[{"domain":"a"},{"domain":"b"}]"#;
        assert_eq!(count_rules(RuleSetContentFormat::SourceJson, source_json), 2);
        assert_eq!(count_rules(RuleSetContentFormat::Binary, b"\x00\x01"), 0);
    }

    fn sample() -> RuleSet {
        RuleSet::create(
            RuleSetId::new(),
            UserId::new(),
            RuleSetName::new("reject").unwrap(),
            RuleSetSource::Remote(RuleSetUrl::new("https://example.com/reject.yaml").unwrap()),
            Some(RuleSetCategory::Domain),
            RuleSetContentFormat::Yaml,
            Some(RuleSetInterval::new(86_400).unwrap()),
            1_700_000_000,
        )
    }

    #[test]
    fn create_enables_and_initializes_timestamps() {
        let rule_set = sample();
        assert!(rule_set.enabled());
        assert!(rule_set.is_remote());
        assert_eq!(rule_set.created_at(), 1_700_000_000);
        assert_eq!(rule_set.updated_at(), 1_700_000_000);
    }

    #[test]
    fn changes_bump_updated_at_and_no_op_keeps_it() {
        let mut rule_set = sample();
        rule_set.set_enabled(false, 1_700_000_100);
        assert_eq!(rule_set.updated_at(), 1_700_000_100);
        rule_set.set_enabled(false, 1_700_000_200);
        assert_eq!(rule_set.updated_at(), 1_700_000_100);

        let same_name = rule_set.name().clone();
        rule_set.rename(same_name, 1_700_000_200);
        assert_eq!(rule_set.updated_at(), 1_700_000_100);
    }

    #[test]
    fn source_accessors_report_expected_variants() {
        let remote = RuleSetSource::Remote(RuleSetUrl::new("https://e.com/a").unwrap());
        assert!(remote.is_remote());
        assert_eq!(remote.kind(), "remote");
        assert!(remote.path().is_none());

        let local = RuleSetSource::Local(RuleSetPath::new("./a.yaml").unwrap());
        assert!(!local.is_remote());
        assert_eq!(local.path().unwrap().value(), "./a.yaml");

        assert_eq!(RuleSetSource::Inline.kind(), "inline");
        assert!(RuleSetSource::Inline.url().is_none());
    }
}
