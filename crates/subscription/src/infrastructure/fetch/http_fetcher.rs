use std::future::{poll_fn, Future};
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
use worker::send::SendFuture;
use worker::{Delay, Fetch, Request};

use crate::application::{FetchError, FetchOutcome, FetchValidators, FetchedDocument, Fetcher};
use crate::domain::SourceUrl;

const USER_AGENT: &str = "FlClash/v0.8.92 clash-verge Platform/macos";
const FETCH_TIMEOUT_MS: u64 = 15_000;
const MAX_BODY_BYTES: usize = 1_800_000;

pub struct HttpFetcher;

#[async_trait]
impl Fetcher for HttpFetcher {
    async fn fetch(
        &self,
        url: &SourceUrl,
        validators: &FetchValidators,
    ) -> Result<FetchOutcome, FetchError> {
        let url = url.value().to_string();
        let validators = validators.clone();
        SendFuture::new(async move { fetch_inner(&url, &validators).await }).await
    }
}

fn request_error(err: impl std::fmt::Debug) -> FetchError {
    FetchError::Failure(format!("{err:?}"))
}

async fn fetch_inner(url: &str, validators: &FetchValidators) -> Result<FetchOutcome, FetchError> {
    let headers = web_sys::Headers::new().map_err(request_error)?;
    headers
        .set("User-Agent", USER_AGENT)
        .map_err(request_error)?;
    headers.set("Accept", "*/*").map_err(request_error)?;
    if let Some(etag) = validators.etag.as_deref() {
        headers.set("If-None-Match", etag).map_err(request_error)?;
    }
    if let Some(last_modified) = validators.last_modified.as_deref() {
        headers
            .set("If-Modified-Since", last_modified)
            .map_err(request_error)?;
    }

    let controller = web_sys::AbortController::new().map_err(request_error)?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_headers(&headers);
    init.set_signal(Some(&controller.signal()));
    let web_request = web_sys::Request::new_with_str_and_init(url, &init).map_err(request_error)?;
    let request: Request = web_request.into();

    let fetch = Fetch::Request(request);
    let mut send = Box::pin(fetch.send());
    let mut timeout = Box::pin(Delay::from(Duration::from_millis(FETCH_TIMEOUT_MS)));
    let completed = poll_fn(|context| {
        if let Poll::Ready(result) = send.as_mut().poll(context) {
            return Poll::Ready(Some(result));
        }
        if timeout.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await;

    let Some(result) = completed else {
        controller.abort();
        return Err(FetchError::Failure("request timed out".to_string()));
    };
    let mut response = result.map_err(|err| FetchError::Failure(err.to_string()))?;

    let status = response.status_code();
    if status == 304 {
        return Ok(FetchOutcome::NotModified);
    }
    if !(200..300).contains(&status) {
        return Err(FetchError::Status(status));
    }

    let declared_length = response
        .headers()
        .get("content-length")
        .map_err(|err| FetchError::Failure(err.to_string()))?
        .and_then(|value| value.parse::<usize>().ok());
    if declared_length.is_some_and(|length| length > MAX_BODY_BYTES) {
        return Err(FetchError::TooLarge);
    }

    let body = response
        .bytes()
        .await
        .map_err(|err| FetchError::Failure(err.to_string()))?;
    if body.len() > MAX_BODY_BYTES {
        return Err(FetchError::TooLarge);
    }

    Ok(FetchOutcome::Fetched(FetchedDocument {
        body,
        etag: header(&response, "etag")?,
        last_modified: header(&response, "last-modified")?,
        subscription_userinfo: header(&response, "subscription-userinfo")?,
        profile_update_interval: header(&response, "profile-update-interval")?,
        profile_web_page_url: header(&response, "profile-web-page-url")?,
        content_disposition: header(&response, "content-disposition")?,
    }))
}

fn header(response: &worker::Response, name: &str) -> Result<Option<String>, FetchError> {
    response
        .headers()
        .get(name)
        .map_err(|err| FetchError::Failure(err.to_string()))
}
