//! Управляемый HTTP fixture; используется только тестами и smoke-сервером.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::{JoinHandle, JoinSet},
};
#[derive(Clone)]
pub enum Scenario {
    StableRange,
    DisconnectAt(usize),
    DisconnectThenHoldAt(usize),
    IgnoreRange,
    ChangeEtag,
    WeakEtag,
    NoEtag,
    BadRange,
    UnknownLength,
    Redirect(String),
    RedirectLoop,
    HtmlInstead,
    EncodedResponse,
    SlowHeaders,
    HoldAt(usize),
    SlowChunks {
        chunk: usize,
        delay: Duration,
    },
    Status {
        status: u16,
        retry_after: Option<String>,
    },
}
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub method: String,
    pub headers: BTreeMap<String, String>,
    pub range_start: Option<u64>,
}
pub struct FixtureServer {
    address: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    release: Arc<Semaphore>,
    task: JoinHandle<()>,
}
impl FixtureServer {
    pub async fn spawn(scenario: Scenario, payload: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture listener");
        let address = format!(
            "http://{}/download",
            listener.local_addr().expect("fixture address")
        );
        let requests = Arc::new(Mutex::new(Vec::new()));
        let release = Arc::new(Semaphore::new(0));
        let recorded = requests.clone();
        let permits = release.clone();
        let data = Arc::new(payload);
        let task = tokio::spawn(async move {
            let mut children = JoinSet::new();
            loop {
                tokio::select! {
                    socket = listener.accept() => { let Ok((socket, _)) = socket else { break }; let (scenario, data, requests, release) = (scenario.clone(), data.clone(), recorded.clone(), permits.clone()); children.spawn(async move { let _ = serve(socket, scenario, data, requests, release).await; }); },
                    _ = children.join_next(), if !children.is_empty() => (),
                }
            }
        });
        Self {
            address,
            requests,
            release,
            task,
        }
    }
    pub fn url(&self) -> String {
        self.address.clone()
    }
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn release(&self) {
        self.release.add_permits(100_000);
    }
}
impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn serve(
    mut socket: TcpStream,
    scenario: Scenario,
    data: Arc<Vec<u8>>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    release: Arc<Semaphore>,
) -> std::io::Result<()> {
    let mut raw = Vec::new();
    let mut buf = [0; 1024];
    while raw.len() < 65536 && !raw.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = socket.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&raw);
    let mut lines = text.split("\r\n");
    let method = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| {
            line.split_once(':')
                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        })
        .collect();
    let start = headers
        .get("range")
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.strip_suffix('-'))
        .and_then(|r| r.parse::<usize>().ok());
    let attempt = {
        let mut list = requests.lock().unwrap_or_else(|p| p.into_inner());
        let attempt = list.len();
        list.push(RecordedRequest {
            method: method.clone(),
            headers,
            range_start: start.map(|n| n as u64),
        });
        attempt
    };
    if method == "HEAD" {
        socket
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    }
    if matches!(scenario, Scenario::SlowHeaders) {
        let _ = release.acquire().await;
    }
    if matches!(scenario, Scenario::RedirectLoop | Scenario::Redirect(_)) {
        let location = match &scenario {
            Scenario::Redirect(url) => url.as_str(),
            _ => "/download",
        };
        socket.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await?;
        return Ok(());
    }
    if let Scenario::Status {
        status,
        retry_after,
    } = scenario
    {
        socket
            .write_all(
                format!(
                    "HTTP/1.1 {status} Fixture\r\n{}Content-Length: 0\r\nConnection: close\r\n\r\n",
                    retry_after
                        .map(|s| format!("Retry-After: {s}\r\n"))
                        .unwrap_or_default()
                )
                .as_bytes(),
            )
            .await?;
        return Ok(());
    }
    let html = b"<!doctype html><html>expired link</html>";
    let changed: Vec<u8>;
    let payload: &[u8] = if matches!(scenario, Scenario::HtmlInstead) {
        html
    } else if matches!(scenario, Scenario::ChangeEtag) && attempt > 0 {
        changed = data.iter().map(|n| n.wrapping_add(1)).collect();
        &changed
    } else {
        &data
    };
    let total = payload.len();
    let offset = if matches!(scenario, Scenario::IgnoreRange) {
        0
    } else {
        start.unwrap_or(0)
    };
    if offset >= total && start.is_some() {
        socket.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
        return Ok(());
    }
    let partial = start.is_some() && !matches!(scenario, Scenario::IgnoreRange);
    let mut response = format!(
        "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Disposition: attachment; filename=\"{}\"\r\n",
        if partial { 206 } else { 200 },
        if payload.starts_with(b"PK") {
            "source.zip"
        } else {
            "dictionary.dsl"
        }
    );
    if !matches!(scenario, Scenario::UnknownLength) {
        response.push_str(&format!("Content-Length: {}\r\n", total - offset));
    }
    if !matches!(scenario, Scenario::NoEtag) {
        response.push_str(&format!(
            "ETag: {}\r\n",
            if matches!(scenario, Scenario::WeakEtag) {
                "W/\"fixture-v1\""
            } else if matches!(scenario, Scenario::ChangeEtag) && attempt > 0 {
                "\"fixture-v2\""
            } else {
                "\"fixture-v1\""
            }
        ));
    }
    if partial {
        response.push_str(&format!(
            "Content-Range: bytes {}-{}/{}\r\n",
            if matches!(scenario, Scenario::BadRange) {
                0
            } else {
                offset
            },
            total - 1,
            total
        ));
    }
    if matches!(scenario, Scenario::EncodedResponse) {
        response.push_str("Content-Encoding: gzip\r\n");
    }
    response.push_str("\r\n");
    socket.write_all(response.as_bytes()).await?;
    match scenario {
        Scenario::DisconnectAt(cut) | Scenario::DisconnectThenHoldAt(cut) if attempt == 0 => {
            socket
                .write_all(&payload[offset..cut.min(total).max(offset)])
                .await?
        }
        Scenario::HoldAt(cut) | Scenario::DisconnectThenHoldAt(cut) => {
            let cut = cut.min(total).max(offset);
            socket.write_all(&payload[offset..cut]).await?;
            if let Ok(permit) = release.acquire().await {
                permit.forget();
            }
            socket.write_all(&payload[cut..]).await?;
        }
        Scenario::SlowChunks { chunk, delay } => {
            for bytes in payload[offset..].chunks(chunk.max(1)) {
                socket.write_all(bytes).await?;
                tokio::time::sleep(delay).await;
            }
        }
        _ => socket.write_all(&payload[offset..]).await?,
    }
    socket.shutdown().await
}
