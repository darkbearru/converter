use crate::{Result, ResumePoint};
use std::time::Duration;
use url::Url;

#[derive(Clone)]
pub struct ResponseMetadata {
    pub status: u16,
    pub effective_url: Url,
    pub encoding: Option<String>,
    pub etag: Option<String>,
    pub content_range: Option<String>,
    pub content_length: Option<u64>,
    pub retry_after: Option<Duration>,
    pub display_name: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ResponseAction {
    StartFresh,
    Append { offset: u64 },
    Restart { reason: String },
    Reject { code: String },
}
pub fn strong_etag(value: &str) -> bool {
    value.starts_with('"')
        && value.ends_with('"')
        && value.len() >= 2
        && value.as_bytes()[1..value.len() - 1]
            .iter()
            .all(|b| *b == 0x21 || (0x23..=0x7e).contains(b) || *b >= 0x80)
}
pub fn content_range(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let (start, end, total): (u64, u64, u64) =
        (start.parse().ok()?, end.parse().ok()?, total.parse().ok()?);
    (start <= end && end < total && end.checked_sub(start)?.checked_add(1).is_some())
        .then_some((start, end, total))
}
pub fn plan_response(
    point: Option<&ResumePoint>,
    response: &ResponseMetadata,
) -> Result<ResponseAction> {
    if response
        .encoding
        .as_deref()
        .is_some_and(|s| !s.eq_ignore_ascii_case("identity"))
    {
        return Ok(ResponseAction::Reject {
            code: "unexpected_encoding".into(),
        });
    }
    let point = point.filter(|p| p.confirmed_bytes > 0);
    if response.status == 416 && point.is_some() {
        return Ok(ResponseAction::Restart {
            reason: "Сервер отклонил диапазон; загрузка начнётся заново".into(),
        });
    }
    if response.status == 200 {
        return Ok(ResponseAction::StartFresh);
    }
    if response.status != 206 {
        return Ok(ResponseAction::Reject {
            code: format!("http_{}", response.status),
        });
    }
    let Some(point) = point else {
        return Ok(ResponseAction::Reject {
            code: "unexpected_partial_response".into(),
        });
    };
    if point.effective_url.as_ref() != Some(&response.effective_url)
        || point.validator.as_deref().is_none_or(|v| !strong_etag(v))
        || response.etag.as_deref().is_none_or(|v| !strong_etag(v))
        || point.validator != response.etag
    {
        return Ok(ResponseAction::Restart {
            reason: "Изменился ресурс или его надёжный ETag; загрузка начнётся заново".into(),
        });
    }
    let Some((start, end, total)) = response.content_range.as_deref().and_then(content_range)
    else {
        return Ok(ResponseAction::Reject {
            code: "invalid_content_range".into(),
        });
    };
    if start != point.confirmed_bytes
        || point.total_bytes.is_some_and(|old| old != total)
        || response
            .content_length
            .is_some_and(|length| length != end - start + 1)
    {
        return Ok(ResponseAction::Reject {
            code: "invalid_content_range".into(),
        });
    }
    Ok(ResponseAction::Append { offset: start })
}
