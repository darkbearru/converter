use converter_app::{AppError, AppResult};
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    browser_download_url: String,
}

fn unavailable() -> AppError {
    AppError::new("update_check_failed", "Не удалось проверить обновления").with_message(
        "messages.update_check_failed",
        converter_core::model::message_args([]),
    )
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

fn installer_unavailable() -> AppError {
    AppError::new(
        "update_installer_unavailable",
        "Установщик этой версии для вашей системы недоступен",
    )
    .with_message(
        "messages.update_installer_unavailable",
        converter_core::model::message_args([]),
    )
}

fn stable_version(value: &str) -> AppResult<Version> {
    Version::parse(value)
        .ok()
        .filter(|version| version.pre.is_empty() && version.build.is_empty())
        .ok_or_else(installer_unavailable)
}

fn installer_url(bytes: &[u8], version: &str, os: &str, arch: &str) -> AppResult<String> {
    let expected = stable_version(version)?;
    let release: Release = serde_json::from_slice(bytes).map_err(|_| installer_unavailable())?;
    if release.draft
        || release.prerelease
        || stable_version(
            release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name),
        )? != expected
    {
        return Err(installer_unavailable());
    }
    let suffix = match (os, arch) {
        ("macos", "aarch64") => "aarch64.dmg",
        ("macos", "x86_64") => "x64.dmg",
        ("windows", "x86_64") => "x64-setup.exe",
        _ => return Err(installer_unavailable()),
    };
    let name = format!("Converter_{expected}_{suffix}");
    let mut matches = release.assets.iter().filter(|asset| asset.name == name);
    let asset = matches.next().ok_or_else(installer_unavailable)?;
    let url = format!(
        "https://github.com/darkbearru/converter/releases/download/{}/{name}",
        release.tag_name
    );
    if matches.next().is_some()
        || asset.state != "uploaded"
        || asset.size == 0
        || asset.browser_download_url != url
    {
        return Err(installer_unavailable());
    }
    Ok(url)
}

async fn fetch_bytes(client: &Client, endpoint: &str) -> AppResult<Vec<u8>> {
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
    Ok(bytes)
}

async fn fetch_release(client: &Client, endpoint: &str, installed: &str) -> AppResult<UpdateInfo> {
    parse_release(&fetch_bytes(client, endpoint).await?, installed)
}

pub async fn check(installed: &str) -> AppResult<UpdateInfo> {
    fetch_release(&client(installed)?, RELEASE_API, installed).await
}

pub async fn download_url(version: &str, installed: &str) -> AppResult<String> {
    let version = stable_version(version)?;
    let endpoint =
        format!("https://api.github.com/repos/darkbearru/converter/releases/tags/v{version}");
    let bytes = fetch_bytes(&client(installed)?, &endpoint).await?;
    installer_url(
        &bytes,
        &version.to_string(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_with_installers() -> Vec<u8> {
        let assets: Vec<_> = ["Converter_0.4.0_aarch64.dmg", "Converter_0.4.0_x64-setup.exe", "Converter_aarch64-apple-darwin.app.zip", "SHA256SUMS"]
            .map(|name| serde_json::json!({
                "name": name, "state": "uploaded", "size": 1234,
                "browser_download_url": format!("https://github.com/darkbearru/converter/releases/download/v0.4.0/{name}")
            })).into();
        serde_json::to_vec(&serde_json::json!({
            "tag_name": "v0.4.0", "draft": false, "prerelease": false, "assets": assets
        }))
        .expect("Релиз с установщиками")
    }

    #[test]
    fn скачивание_выбирает_установщик_для_ос_и_архитектуры() {
        let body = release_with_installers();
        for (os, arch, file) in [
            ("macos", "aarch64", "Converter_0.4.0_aarch64.dmg"),
            ("windows", "x86_64", "Converter_0.4.0_x64-setup.exe"),
        ] {
            assert_eq!(
                installer_url(&body, "0.4.0", os, arch).expect("Установщик"),
                format!("https://github.com/darkbearru/converter/releases/download/v0.4.0/{file}")
            );
        }
        for (os, arch) in [
            ("macos", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "x86_64"),
        ] {
            assert!(installer_url(&body, "0.4.0", os, arch).is_err());
        }
    }

    #[test]
    fn отсутствующий_или_подменённый_установщик_не_открывает_страницу_релизов() {
        let valid: serde_json::Value =
            serde_json::from_slice(&release_with_installers()).expect("JSON");
        for (field, value) in [
            ("assets", serde_json::json!([])),
            ("draft", true.into()),
            ("prerelease", true.into()),
            ("tag_name", "v0.5.0".into()),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert!(
                installer_url(
                    &serde_json::to_vec(&invalid).expect("JSON"),
                    "0.4.0",
                    "macos",
                    "aarch64"
                )
                .is_err()
            );
        }
        for (field, value) in [
            ("state", "new".into()),
            ("size", 0.into()),
            (
                "browser_download_url",
                "https://example.org/malware.dmg".into(),
            ),
            (
                "browser_download_url",
                "https://github.com/darkbearru/converter/releases/download/v0.4.0/other.dmg".into(),
            ),
        ] {
            let mut invalid = valid.clone();
            invalid["assets"][0][field] = value;
            assert!(
                installer_url(
                    &serde_json::to_vec(&invalid).expect("JSON"),
                    "0.4.0",
                    "macos",
                    "aarch64"
                )
                .is_err()
            );
        }
        let mut duplicate = valid.clone();
        duplicate["assets"]
            .as_array_mut()
            .expect("Assets")
            .push(valid["assets"][0].clone());
        assert!(
            installer_url(
                &serde_json::to_vec(&duplicate).expect("JSON"),
                "0.4.0",
                "macos",
                "aarch64"
            )
            .is_err()
        );
        for version in ["../latest", "0.4.0-rc.1", "0.4.0+build.1", "0.5.0"] {
            assert!(
                installer_url(&release_with_installers(), version, "macos", "aarch64").is_err()
            );
        }
    }

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
        let version = check("0.0.0")
            .await
            .expect("Ответ GitHub API")
            .version
            .expect("Новая версия");
        let url = download_url(&version, "0.0.0")
            .await
            .expect("Установщик GitHub");
        assert!(url.starts_with(&format!("https://github.com/darkbearru/converter/releases/download/v{version}/Converter_{version}_")));
        assert!(url.ends_with(if cfg!(target_os = "macos") {
            ".dmg"
        } else {
            "-setup.exe"
        }));
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
