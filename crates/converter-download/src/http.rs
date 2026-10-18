use crate::{
    DownloadError, DownloadPolicy, DownloadRequest, Result, ResumePoint, policy,
    resume::{ResponseMetadata, content_range, strong_etag},
};
use reqwest::{Response, header};
use std::time::Duration;
pub struct OpenResponse {
    pub metadata: ResponseMetadata,
    pub response: Response,
}
pub async fn open(
    request: &DownloadRequest,
    point: Option<&ResumePoint>,
    policy: &DownloadPolicy,
) -> Result<OpenResponse> {
    policy::validate(policy)?;
    if !matches!(request.url.scheme(), "http" | "https")
        || !request.url.username().is_empty()
        || request.url.password().is_some()
    {
        return Err(DownloadError::new(
            "invalid_download_url",
            "Нужна ссылка HTTP или HTTPS без встроенного пароля",
        ));
    }
    let max_redirects = policy.max_redirects as usize;
    let client = reqwest::Client::builder()
        .connect_timeout(policy.connect_timeout)
        .read_timeout(policy.idle_timeout)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if !policy::redirect_allowed(attempt.previous(), attempt.url(), max_redirects) {
                attempt.error("Недопустимое перенаправление")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| network_error())?;
    let mut get = client
        .get(request.url.clone())
        .header(header::ACCEPT_ENCODING, "identity");
    if let Some(point) =
        point.filter(|p| p.confirmed_bytes > 0 && p.validator.as_deref().is_some_and(strong_etag))
    {
        get = get
            .header(header::RANGE, format!("bytes={}-", point.confirmed_bytes))
            .header(
                header::IF_RANGE,
                point.validator.as_deref().unwrap_or_default(),
            );
    }
    let response = get.send().await.map_err(|_| network_error())?;
    let text = |name: header::HeaderName| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let name = text(header::CONTENT_DISPOSITION)
        .and_then(|s| {
            s.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("filename=")
                    .map(|v| v.trim_matches('"').to_owned())
            })
        })
        .or_else(|| {
            response
                .url()
                .path_segments()
                .and_then(|mut p| p.next_back())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "Словарь".into());
    let display_name: String = name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(120)
        .collect();
    let metadata = ResponseMetadata {
        status: response.status().as_u16(),
        effective_url: response.url().clone(),
        encoding: text(header::CONTENT_ENCODING),
        etag: text(header::ETAG),
        content_range: text(header::CONTENT_RANGE),
        content_length: response.content_length(),
        retry_after: text(header::RETRY_AFTER)
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs),
        display_name,
    };
    if metadata
        .content_length
        .is_some_and(|n| n > policy.max_bytes)
        || metadata
            .content_range
            .as_deref()
            .and_then(content_range)
            .is_some_and(|(_, _, n)| n > policy.max_bytes)
    {
        return Err(DownloadError::new(
            "download_size_limit",
            "Размер источника превышает допустимый",
        ));
    }
    Ok(OpenResponse { metadata, response })
}
pub(crate) fn network_error() -> DownloadError {
    DownloadError::new(
        "download_network",
        "Сетевая ошибка или таймаут; сохранённую часть можно продолжить",
    )
}
