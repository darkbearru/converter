use converter_download::{
    DownloadPolicy, DownloadRequest, ResumePoint, http,
    resume::{ResponseAction, ResponseMetadata, plan_response},
};
use converter_test_support::http::{FixtureServer, Scenario};

fn point(url: &str) -> ResumePoint {
    ResumePoint {
        confirmed_bytes: 1_048_576,
        validator: Some("\"fixture-v1\"".into()),
        effective_url: Some(url.parse().expect("URL")),
        total_bytes: Some(2_097_152),
    }
}
fn metadata(url: &str) -> ResponseMetadata {
    ResponseMetadata {
        status: 206,
        effective_url: url.parse().expect("URL"),
        encoding: None,
        etag: Some("\"fixture-v1\"".into()),
        content_range: Some("bytes 1048576-2097151/2097152".into()),
        content_length: Some(1_048_576),
        retry_after: None,
        display_name: "dictionary.dsl".into(),
    }
}
#[tokio::test]
async fn докачивает_неизменный_ресурс() -> Result<(), Box<dyn std::error::Error>> {
    let server = FixtureServer::spawn(Scenario::DisconnectAt(1_048_576), vec![42; 2_097_152]).await;
    let req = DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: server.url().parse()?,
        expected_sha256: None,
    };
    let mut first = http::open(&req, None, &DownloadPolicy::default()).await?;
    let mut received = 0;
    while let Some(bytes) = first.response.chunk().await.transpose() {
        match bytes {
            Ok(bytes) => received += bytes.len(),
            Err(_) => break,
        }
    }
    assert_eq!(received, 1_048_576);
    let resume = point(&server.url());
    let next = http::open(&req, Some(&resume), &DownloadPolicy::default()).await?;
    assert_eq!(
        plan_response(Some(&resume), &next.metadata)?,
        ResponseAction::Append { offset: 1_048_576 }
    );
    assert_eq!(next.response.bytes().await?.len(), 1_048_576);
    let requests = server.requests();
    assert_eq!(requests[1].headers["range"], "bytes=1048576-");
    assert_eq!(requests[1].headers["if-range"], "\"fixture-v1\"");
    for request in requests {
        assert_eq!(request.method, "GET");
        assert_eq!(request.headers["accept-encoding"], "identity");
        assert!(!request.headers.contains_key("cookie"));
        assert!(!request.headers.contains_key("authorization"));
    }
    Ok(())
}
#[test]
fn не_склеивает_версии() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://example.org/download?secret-token";
    let point = point(url);
    let good = metadata(url);
    let mut changed = good.clone();
    changed.status = 200;
    assert_eq!(
        plan_response(Some(&point), &changed)?,
        ResponseAction::StartFresh
    );
    for variant in 0..9 {
        let mut response = good.clone();
        match variant {
            0 => response.status = 416,
            1 => response.etag = Some("\"v2\"".into()),
            2 => response.etag = Some("W/\"fixture-v1\"".into()),
            3 => response.etag = None,
            4 => response.content_range = Some("bytes 0-1048575/2097152".into()),
            5 => response.content_range = Some("bytes 1048576-2097151/9999999".into()),
            6 => response.effective_url = "https://example.org/other".parse()?,
            7 => response.content_length = Some(7),
            _ => {
                response.content_range =
                    Some("bytes 18446744073709551615-18446744073709551615/*".into())
            }
        }
        assert!(!matches!(
            plan_response(Some(&point), &response)?,
            ResponseAction::Append { .. }
        ));
    }
    Ok(())
}
#[tokio::test]
async fn http_ошибки_ограничения_и_отсутствие_head() -> Result<(), Box<dyn std::error::Error>> {
    for (scenario, expected) in [
        (Scenario::EncodedResponse, "unexpected_encoding"),
        (Scenario::RedirectLoop, "download_network"),
        (Scenario::SlowHeaders, "download_network"),
    ] {
        let server = FixtureServer::spawn(scenario, b"data".to_vec()).await;
        let req = DownloadRequest {
            source_id: uuid::Uuid::new_v4().to_string(),
            url: format!("{}?secret-token", server.url()).parse()?,
            expected_sha256: None,
        };
        let policy = DownloadPolicy {
            idle_timeout: std::time::Duration::from_millis(50),
            ..Default::default()
        };
        let error = match http::open(&req, None, &policy).await {
            Err(e) => e,
            Ok(open) => match plan_response(None, &open.metadata)? {
                ResponseAction::Reject { code } => {
                    converter_download::DownloadError::new(&code, "Отклонён ответ")
                }
                _ => panic!("unexpected response"),
            },
        };
        assert_eq!(error.code, expected);
        assert!(!error.to_string().contains("secret-token"));
    }
    let server = FixtureServer::spawn(Scenario::UnknownLength, b"data".to_vec()).await;
    let req = DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: server.url().parse()?,
        expected_sha256: None,
    };
    let opened = http::open(&req, None, &DownloadPolicy::default()).await?;
    assert_eq!(opened.metadata.content_length, None);
    assert_eq!(opened.response.bytes().await?.as_ref(), b"data");
    assert_eq!(server.requests().len(), 1);
    Ok(())
}

#[test]
fn запрещает_понижение_tls_и_лишние_redirect() {
    let secure = "https://example.org/file".parse().expect("URL");
    let insecure = "http://example.org/file".parse().expect("URL");
    assert!(!converter_download::policy::redirect_allowed(
        std::slice::from_ref(&secure),
        &insecure,
        10
    ));
    assert!(!converter_download::policy::redirect_allowed(
        &vec![secure.clone(); 11],
        &secure,
        10
    ));
    assert!(converter_download::policy::redirect_allowed(
        &[insecure],
        &secure,
        10
    ));
}
