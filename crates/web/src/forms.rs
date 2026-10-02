use contract::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN, USERNAME_MAX_LEN, USERNAME_MIN_LEN};

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
