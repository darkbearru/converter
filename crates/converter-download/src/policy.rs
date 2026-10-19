use crate::{DownloadError, DownloadPolicy, Result};
pub fn validate(policy: &DownloadPolicy) -> Result<()> {
    if policy.max_bytes == 0
        || policy.checkpoint_bytes == 0
        || policy.checkpoint_bytes > 16 * 1024 * 1024
        || policy.connect_timeout.is_zero()
        || policy.idle_timeout.is_zero()
    {
        return Err(DownloadError::new(
            "invalid_download_policy",
            "Некорректные ограничения загрузчика",
        )
        .with_message("messages.invalid_download_policy", crate::message_args([])));
    }
    Ok(())
}
pub fn retryable(status: u16) -> bool {
    matches!(status, 408 | 429 | 500..=599)
}
pub fn retry_after(value: &str, now: std::time::SystemTime) -> Option<std::time::Duration> {
    value
        .parse()
        .ok()
        .map(std::time::Duration::from_secs)
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()
                .map(|date| date.duration_since(now).unwrap_or_default())
        })
}
pub fn redirect_allowed(previous: &[url::Url], next: &url::Url, max: usize) -> bool {
    previous.len() <= max
        && matches!(next.scheme(), "http" | "https")
        && next.username().is_empty()
        && next.password().is_none()
        && !(next.scheme() == "http" && previous.iter().any(|url| url.scheme() == "https"))
}
