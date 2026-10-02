use axum::{routing::get, Router};
use tower_service::Service;
use worker::*;

fn router() -> Router {
    Router::new().route("/", get(root))
}

pub async fn root() -> &'static str {
    "Hello Axum!"
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}
