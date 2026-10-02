use contract::{
    PASSWORD_MAX_LEN, PASSWORD_MIN_LEN, SOURCE_URL_MAX_LEN, SUBSCRIPTION_NAME_MAX_LEN,
    SUBSCRIPTION_NAME_MIN_LEN, USERNAME_MAX_LEN, USERNAME_MIN_LEN,
};

pub fn validate_username(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let length = trimmed.chars().count();
    if !(USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&length) {
        return Err(format!(
            "用户名长度需在 {USERNAME_MIN_LEN}-{USERNAME_MAX_LEN} 个字符之间"
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("用户名只能包含字母、数字、下划线和连字符".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn validate_password(password: &str) -> Result<(), String> {
    let length = password.chars().count();
    if (PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&length) {
        Ok(())
    } else {
        Err(format!(
            "密码长度需在 {PASSWORD_MIN_LEN}-{PASSWORD_MAX_LEN} 个字符之间"
        ))
    }
}

pub fn validate_password_confirm(password: &str, confirm: &str) -> Result<(), String> {
    if password == confirm {
        Ok(())
    } else {
        Err("两次输入的密码不一致".to_string())
    }
}

pub fn validate_source(name: &str, url: &str) -> Result<(String, String), String> {
    let name = name.trim();
    let name_length = name.chars().count();
    if !(SUBSCRIPTION_NAME_MIN_LEN..=SUBSCRIPTION_NAME_MAX_LEN).contains(&name_length) {
        return Err(format!(
            "名称长度需在 {SUBSCRIPTION_NAME_MIN_LEN}-{SUBSCRIPTION_NAME_MAX_LEN} 个字符之间"
        ));
    }

    let url = url.trim();
    if url.len() > SOURCE_URL_MAX_LEN {
        return Err(format!("订阅链接不能超过 {SOURCE_URL_MAX_LEN} 个字符"));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("订阅链接需以 http:// 或 https:// 开头".to_string());
    }

    Ok((name.to_string(), url.to_string()))
}

pub fn validate_publication_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    let length = name.chars().count();
    if !(SUBSCRIPTION_NAME_MIN_LEN..=SUBSCRIPTION_NAME_MAX_LEN).contains(&length) {
        return Err(format!(
            "名称长度需在 {SUBSCRIPTION_NAME_MIN_LEN}-{SUBSCRIPTION_NAME_MAX_LEN} 个字符之间"
        ));
    }
    Ok(name.to_string())
}
