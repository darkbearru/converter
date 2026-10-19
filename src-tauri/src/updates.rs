use converter_app::{AppError, AppResult};
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const RELEASE_PAGE: &str = "https://github.com/darkbearru/converter/releases/latest";
const RELEASE_API: &str = "https://api.github.com/repos/darkbearru/converter/releases/latest";
const MAX_RESPONSE: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub version: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

fn unavailable() -> AppError {
    AppError::new("update_check_failed", "Не удалось проверить обновления")
}

fn parse_release(bytes: &[u8], installed: &str) -> AppResult<UpdateInfo> {
    let release: Release = serde_json::from_slice(bytes).map_err(|_| unavailable())?;
    let installed = Version::parse(installed).map_err(|_| unavailable())?;
    let latest = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| unavailable())?;
    let available = !release.draft
        && !release.prerelease
        && latest.pre.is_empty()
        && latest.cmp_precedence(&installed).is_gt();
    Ok(UpdateInfo {
        version: available.then(|| latest.to_string()),
    })
}

fn client(version: &str) -> AppResult<Client> {
    Client::builder()
        .user_agent(format!("Converter/{version}"))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable())
}

async fn fetch_release(client: &Client, endpoint: &str, installed: &str) -> AppResult<UpdateInfo> {
    let mut response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| unavailable())?
        .error_for_status()
        .map_err(|_| unavailable())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
    {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_release(&bytes, installed)
}

pub async fn check(installed: &str) -> AppResult<UpdateInfo> {
    fetch_release(&client(installed)?, RELEASE_API, installed).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, prerelease: bool) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "tag_name": tag, "draft": draft, "prerelease": prerelease
        }))
        .expect("JSON релиза")
    }

    #[test]
    fn версии_сравниваются_по_числам_без_понижения() {
        for (installed, tag, expected) in [
            ("0.2.9", "v0.2.10", Some("0.2.10")),
            ("0.9.9", "1.0.0", Some("1.0.0")),
            ("0.2.3", "v0.2.3", None),
            ("0.2.4", "v0.2.3", None),
            ("0.2.3", "v0.2.3+build.9", None),
        ] {
            assert_eq!(
                parse_release(&release(tag, false, false), installed)
                    .expect("Релиз")
                    .version
                    .as_deref(),
                expected
            );
        }
    }

    #[test]
    fn черновики_и_предварительные_версии_не_предлагаются() {
        for (tag, draft, prerelease) in [
            ("v1.0.0", true, false),
            ("v1.0.0", false, true),
            ("v1.0.0-rc.1", false, false),
        ] {
            assert!(
                parse_release(&release(tag, draft, prerelease), "0.2.3")
                    .expect("Ответ")
                    .version
                    .is_none()
            );
        }
    }

    #[test]
    fn некорректный_ответ_не_выдаётся_за_актуальную_версию() {
        for body in [
            b"{}".to_vec(),
            b"not json".to_vec(),
            release("main", false, false),
        ] {
            assert!(parse_release(&body, "0.2.3").is_err());
        }
    }

    #[tokio::test]
    #[ignore = "Проверка настоящего GitHub API требует доступа в интернет"]
    async fn опубликованный_релиз_github_доступен_без_токена() {
        assert!(
            check("0.0.0")
                .await
                .expect("Ответ GitHub API")
                .version
                .is_some()
        );
    }

    #[tokio::test]
    async fn http_проверка_передаёт_заголовки_и_обрабатывает_сбой_сервера() {
        use std::io::{Read, Write};
        for (status, body, expected) in [
            (
                "200 OK",
                String::from_utf8(release("v0.3.0", false, false)).expect("UTF-8"),
                true,
            ),
            ("403 Forbidden", "{}".to_string(), false),
            ("200 OK", "invalid".to_string(), false),
        ] {
            let server = std::net::TcpListener::bind("127.0.0.1:0").expect("Локальный HTTP");
            let address = server.local_addr().expect("Адрес");
            let thread = std::thread::spawn(move || {
                let (mut stream, _) = server.accept().expect("Запрос");
                let mut bytes = [0; 4096];
                let n = stream.read(&mut bytes).expect("Заголовки");
                let headers = String::from_utf8_lossy(&bytes[..n]).to_lowercase();
                assert!(headers.contains("user-agent: converter/0.2.3"));
                assert!(headers.contains("accept: application/vnd.github+json"));
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .expect("Ответ");
            });
            let result = fetch_release(
                &client("0.2.3").expect("HTTP клиент"),
                &format!("http://{address}/latest"),
                "0.2.3",
            )
            .await;
            thread.join().expect("Сервер завершился");
            assert_eq!(result.is_ok(), expected);
            if expected {
                assert_eq!(result.expect("Релиз").version.as_deref(), Some("0.3.0"));
            }
        }
    }
}
