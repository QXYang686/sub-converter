#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscriptionUserInfo {
    pub upload: Option<i64>,
    pub download: Option<i64>,
    pub total: Option<i64>,
    pub expire: Option<i64>,
}

impl SubscriptionUserInfo {
    pub fn is_empty(&self) -> bool {
        self.upload.is_none()
            && self.download.is_none()
            && self.total.is_none()
            && self.expire.is_none()
    }
}

pub fn parse_subscription_userinfo(raw: &str) -> SubscriptionUserInfo {
    let mut info = SubscriptionUserInfo::default();
    for part in raw.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let Ok(value) = value.parse::<i64>() else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload = Some(value),
            "download" => info.download = Some(value),
            "total" => info.total = Some(value),
            "expire" => info.expire = Some(value),
            _ => {}
        }
    }
    info
}

pub fn parse_content_disposition_filename(raw: &str) -> Option<String> {
    for part in raw.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "filename*" {
            if let Some(decoded) = decode_extended_filename(value) {
                return Some(decoded);
            }
        } else if key == "filename" {
            let name = value.trim_matches('"').trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn decode_extended_filename(value: &str) -> Option<String> {
    let mut parts = value.splitn(3, '\'');
    let charset = parts.next()?.trim();
    let _language = parts.next()?;
    let encoded = parts.next()?;
    if !charset.eq_ignore_ascii_case("utf-8") {
        return None;
    }
    let bytes = percent_decode(encoded)?;
    String::from_utf8(bytes).ok()
}

fn percent_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            out.push(high << 4 | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_userinfo() {
        let info = parse_subscription_userinfo(
            "upload=1234567; download=7654321; total=10737418240; expire=1790000000",
        );
        assert_eq!(info.upload, Some(1_234_567));
        assert_eq!(info.download, Some(7_654_321));
        assert_eq!(info.total, Some(10_737_418_240));
        assert_eq!(info.expire, Some(1_790_000_000));
    }

    #[test]
    fn parses_partial_and_empty_userinfo() {
        let info = parse_subscription_userinfo("upload=1; download=; expire=; extra=9");
        assert_eq!(info.upload, Some(1));
        assert_eq!(info.download, None);
        assert_eq!(info.expire, None);
        assert!(!info.is_empty());

        let info = parse_subscription_userinfo("expire=");
        assert!(info.is_empty());

        let info = parse_subscription_userinfo("upload=not-a-number");
        assert!(info.is_empty());
    }

    #[test]
    fn parses_rfc5987_filename() {
        let raw = "attachment;filename*=UTF-8''%E9%AD%94%E6%88%92.net";
        assert_eq!(
            parse_content_disposition_filename(raw),
            Some("魔戒.net".to_string())
        );
    }

    #[test]
    fn parses_plain_and_missing_filenames() {
        assert_eq!(
            parse_content_disposition_filename("attachment; filename=\"my-sub.yaml\"; foo=bar"),
            Some("my-sub.yaml".to_string())
        );
        assert_eq!(parse_content_disposition_filename("attachment"), None);
        assert_eq!(
            parse_content_disposition_filename("attachment; filename*=UTF-16''abc"),
            None
        );
    }
}
