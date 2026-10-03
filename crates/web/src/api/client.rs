use gloo_net::http::Request;
use serde::Serialize;

use contract::auth::ErrorResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn is_unauthorized(&self) -> bool {
        self.status == 401
    }

    pub(crate) fn unauthenticated() -> Self {
        Self {
            status: 401,
            code: "UNAUTHORIZED".to_string(),
            message: "not signed in".to_string(),
        }
    }

    pub(crate) fn invalid_response(message: String) -> Self {
        Self {
            status: 0,
            code: "INVALID_RESPONSE".to_string(),
            message,
        }
    }

    pub(crate) fn network(message: String) -> Self {
        Self {
            status: 0,
            code: "NETWORK_ERROR".to_string(),
            message,
        }
    }

    pub(crate) fn passkey(message: String) -> Self {
        Self {
            status: 0,
            code: "PASSKEY_ERROR".to_string(),
            message,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

pub fn user_message(error: &ApiError) -> String {
    match error.code.as_str() {
        "VALIDATION_ERROR" => "输入不符合要求，请检查后重试".to_string(),
        "USERNAME_TAKEN" => "该用户名已被占用".to_string(),
        "SOURCE_URL_TAKEN" => "该订阅链接已存在".to_string(),
        "UNAUTHORIZED" => "用户名或密码错误".to_string(),
        "FORBIDDEN" => "请求被拒绝，请刷新页面后重试".to_string(),
        "NOT_FOUND" => "请求的资源不存在".to_string(),
        "NETWORK_ERROR" => "网络异常，请稍后重试".to_string(),
        "PASSKEY_ERROR" => "Passkey 操作未完成（可能被取消）".to_string(),
        _ => error.message.clone(),
    }
}

const XHR_HEADER: &str = "X-Requested-With";
const XHR_HEADER_VALUE: &str = "XMLHttpRequest";

pub(crate) async fn get(path: &str, bearer: Option<&str>) -> Result<String, ApiError> {
    let mut request = Request::get(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn post(path: &str, bearer: Option<&str>) -> Result<String, ApiError> {
    let mut request = Request::post(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn delete(path: &str, bearer: Option<&str>) -> Result<String, ApiError> {
    let mut request = Request::delete(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn post_json<B: Serialize>(
    path: &str,
    body: &B,
    bearer: Option<&str>,
) -> Result<String, ApiError> {
    let mut request = Request::post(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let request = request
        .json(body)
        .map_err(|err| ApiError::network(err.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn patch_json<B: Serialize>(
    path: &str,
    body: &B,
    bearer: Option<&str>,
) -> Result<String, ApiError> {
    let mut request = Request::patch(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let request = request
        .json(body)
        .map_err(|err| ApiError::network(err.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn put_json<B: Serialize>(
    path: &str,
    body: &B,
    bearer: Option<&str>,
) -> Result<String, ApiError> {
    let mut request = Request::put(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let request = request
        .json(body)
        .map_err(|err| ApiError::network(err.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

pub(crate) async fn put_text(
    path: &str,
    body: &str,
    bearer: Option<&str>,
) -> Result<String, ApiError> {
    let mut request = Request::put(path).header(XHR_HEADER, XHR_HEADER_VALUE);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let request = request
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(body.to_string())
        .map_err(|err| ApiError::network(err.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    read_response(response).await
}

async fn read_response(response: gloo_net::http::Response) -> Result<String, ApiError> {
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| ApiError::network(err.to_string()))?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(parse_error(status, &text))
    }
}

fn parse_error(status: u16, text: &str) -> ApiError {
    serde_json::from_str::<ErrorResponse>(text)
        .map(|body| ApiError {
            status,
            code: body.error.code,
            message: body.error.message,
        })
        .unwrap_or_else(|_| ApiError {
            status,
            code: "UNKNOWN".to_string(),
            message: text.to_string(),
        })
}
