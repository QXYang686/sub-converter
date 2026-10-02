use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tracing::{field, Instrument, Span};

pub const REQUEST_ID_HEADER: &str = "x-request-id";

const MAX_REQUEST_ID_LEN: usize = 64;

pub async fn trace_request(request: Request, next: Next) -> Response {
    let request_id = inbound_request_id(&request)
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let placement = request
        .headers()
        .get("cf-placement")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let span = tracing::info_span!(
        "http.request",
        request_id = %request_id,
        method = %method,
        route = %route,
        placement = field::Empty,
        status = field::Empty,
        latency_ms = field::Empty,
        user_id = field::Empty,
    );
    if let Some(placement) = placement {
        span.record("placement", placement.as_str());
    }

    let started = now_millis();
    let mut response = next.run(request).instrument(span.clone()).await;
    let latency_ms = now_millis().saturating_sub(started);
    let status = response.status();

    span.record("status", status.as_u16());
    span.record("latency_ms", latency_ms);
    emit_completion(&span, status);

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

pub fn record_user_id(user_id: &str) {
    Span::current().record("user_id", user_id);
}

fn inbound_request_id(request: &Request) -> Option<String> {
    let value = request.headers().get(REQUEST_ID_HEADER)?.to_str().ok()?;
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_REQUEST_ID_LEN
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    Some(value.to_string())
}

fn emit_completion(span: &Span, status: StatusCode) {
    if status.is_server_error() {
        tracing::error!(parent: span, "request completed");
    } else if status.is_client_error() {
        tracing::debug!(parent: span, "request completed");
    } else {
        tracing::info!(parent: span, "request completed");
    }
}

fn now_millis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        worker::Date::now().as_millis()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    use super::*;

    fn test_router() -> Router {
        Router::new()
            .route("/test/{id}", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(trace_request))
    }

    fn call(request: Request<Body>) -> Response {
        futures::executor::block_on(test_router().oneshot(request)).unwrap()
    }

    #[test]
    fn generates_request_id_when_absent() {
        let request = Request::builder().uri("/test/1").body(Body::empty()).unwrap();
        let response = call(request);
        assert_eq!(response.status(), StatusCode::OK);
        let request_id = response
            .headers()
            .get(REQUEST_ID_HEADER)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(request_id.len(), 32);
        assert!(request_id.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn preserves_valid_inbound_request_id() {
        let request = Request::builder()
            .uri("/test/1")
            .header(REQUEST_ID_HEADER, "abc-123_DEF")
            .body(Body::empty())
            .unwrap();
        let response = call(request);
        assert_eq!(
            response.headers().get(REQUEST_ID_HEADER).unwrap(),
            "abc-123_DEF"
        );
    }

    #[test]
    fn replaces_malformed_inbound_request_id() {
        for value in ["", "with space", "bad/slash", &"x".repeat(MAX_REQUEST_ID_LEN + 1)] {
            let request = Request::builder()
                .uri("/test/1")
                .header(REQUEST_ID_HEADER, value)
                .body(Body::empty())
                .unwrap();
            let response = call(request);
            let request_id = response
                .headers()
                .get(REQUEST_ID_HEADER)
                .unwrap()
                .to_str()
                .unwrap();
            assert_ne!(request_id, value);
        }
    }
}
