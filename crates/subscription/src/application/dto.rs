use crate::domain::{Publication, Source, SourceSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolCountView {
    pub protocol: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshotView {
    pub fetched_at: Option<i64>,
    pub proxy_count: u32,
    pub group_count: u32,
    pub rule_count: u32,
    pub protocol_counts: Vec<ProtocolCountView>,
    pub upload: Option<i64>,
    pub download: Option<i64>,
    pub total: Option<i64>,
    pub expire: Option<i64>,
    pub update_interval: Option<i64>,
    pub provider_name: Option<String>,
    pub provider_url: Option<String>,
    pub last_error: Option<String>,
}

impl From<&SourceSnapshot> for SourceSnapshotView {
    fn from(snapshot: &SourceSnapshot) -> Self {
        let meta = snapshot.meta();
        Self {
            fetched_at: (snapshot.fetched_at() > 0).then_some(snapshot.fetched_at()),
            proxy_count: meta.proxy_count,
            group_count: meta.group_count,
            rule_count: meta.rule_count,
            protocol_counts: meta
                .protocol_counts
                .iter()
                .map(|(protocol, count)| ProtocolCountView {
                    protocol: protocol.clone(),
                    count: *count,
                })
                .collect(),
            upload: meta.userinfo.upload,
            download: meta.userinfo.download,
            total: meta.userinfo.total,
            expire: meta.userinfo.expire,
            update_interval: meta.update_interval,
            provider_name: meta.provider_name.clone(),
            provider_url: meta.provider_url.clone(),
            last_error: meta.last_error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub snapshot: Option<SourceSnapshotView>,
}

impl SourceView {
    pub fn from_source(source: &Source) -> Self {
        Self::with_snapshot(source, None)
    }

    pub fn with_snapshot(source: &Source, snapshot: Option<&SourceSnapshot>) -> Self {
        Self {
            id: source.id().to_string(),
            name: source.name().value().to_string(),
            url: source.url().value().to_string(),
            enabled: source.enabled(),
            created_at: source.created_at(),
            updated_at: source.updated_at(),
            snapshot: snapshot.map(SourceSnapshotView::from),
        }
    }
}

impl From<&Source> for SourceView {
    fn from(source: &Source) -> Self {
        Self::from_source(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationView {
    pub id: String,
    pub name: String,
    pub secret: String,
    pub enabled: bool,
    pub expires_at: Option<i64>,
    pub source_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&Publication> for PublicationView {
    fn from(publication: &Publication) -> Self {
        Self {
            id: publication.id().to_string(),
            name: publication.name().value().to_string(),
            secret: publication.secret().value().to_string(),
            enabled: publication.enabled(),
            expires_at: publication.expires_at(),
            source_ids: publication
                .sources()
                .iter()
                .map(|source| source.source_id().to_string())
                .collect(),
            created_at: publication.created_at(),
            updated_at: publication.updated_at(),
        }
    }
}
