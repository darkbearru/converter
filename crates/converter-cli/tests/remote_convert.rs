use converter_test_support::http::{FixtureServer, Scenario};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
fn resource_zip(bytes: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("picture.png", zip::write::SimpleFileOptions::default())
        .expect("entry");
    zip.write_all(bytes).expect("data");
    zip.finish().expect("zip").into_inner()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn не_начинает_конвертацию_до_получения_ресурсов() -> Result<(), Box<dyn std::error::Error>> {
    let dsl = b"#NAME \"Remote\"\nalpha\n\t[b]Body[/b] [s]picture.png[/s]\n".to_vec();
    let resource = vec![3u8; 300_000];
    let main = FixtureServer::spawn(Scenario::StableRange, dsl).await;
    let attachment = FixtureServer::spawn(Scenario::HoldAt(0), resource_zip(&resource)).await;
    let temp = tempfile::tempdir()?;
    let output = temp.path().join("output");
    let cache = temp.path().join("cache");
    let main_url = format!("{}?secret-token", main.url());
    let resources_url = attachment.url();
    let args = [
        "convert",
        "--url",
        &main_url,
        "--resources-url",
        &resources_url,
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_converter-cli"))
        .args(args)
        .arg("--cache")
        .arg(&cache)
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().expect("stderr");
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            recorded.lock().expect("events").push(line);
        }
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while attachment.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(
        !events
            .lock()
            .expect("events")
            .iter()
            .any(|line| line.contains("\"phase\":\"conversion\""))
    );
    assert!(!output.exists());
    attachment.release();
    let result = child.wait_with_output().await?;
    reader.await?;
    assert!(
        result.status.success(),
        "{}",
        events.lock().expect("events").join("\n")
    );
    let public = String::from_utf8(result.stdout)?;
    assert!(!public.contains("secret-token"));
    assert!(
        !events
            .lock()
            .expect("events")
            .join("\n")
            .contains("secret-token")
    );
    let json: serde_json::Value = serde_json::from_str(&public)?;
    let directory =
        std::path::PathBuf::from(json["results"][0]["directory"].as_str().expect("result"));
    let mdd = mdictlib::MddFile::open(directory.join("dictionary.mdd"))?;
    let span = mdd.lookup_span("\\picture.png")?.expect("resource");
    let mut extracted = Vec::new();
    span.copy_to(&mut extracted)?;
    assert_eq!(Sha256::digest(&extracted), Sha256::digest(&resource));
    let counts = (main.requests().len(), attachment.requests().len());
    // Остановленные серверы доказывают, что повтор целиком использует локальный кэш.
    drop(main);
    drop(attachment);
    let result = Command::new(env!("CARGO_BIN_EXE_converter-cli"))
        .args(args)
        .arg("--cache")
        .arg(&cache)
        .arg("--output")
        .arg(&output)
        .output()
        .await?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(counts, (1, 1));
    Ok(())
}

#[tokio::test]
async fn html_вместо_словаря_не_публикуется() -> Result<(), Box<dyn std::error::Error>> {
    let server = FixtureServer::spawn(Scenario::HtmlInstead, vec![]).await;
    let temp = tempfile::tempdir()?;
    let result = Command::new(env!("CARGO_BIN_EXE_converter-cli"))
        .args([
            "convert",
            "--url",
            &format!("{}?secret-token", server.url()),
        ])
        .arg("--cache")
        .arg(temp.path().join("cache"))
        .arg("--output")
        .arg(temp.path().join("output"))
        .output()
        .await?;
    assert_eq!(result.status.code(), Some(1));
    let public = String::from_utf8(result.stdout)?;
    assert!(public.contains("download_is_html"));
    assert!(!public.contains("secret-token"));
    Ok(())
}

#[tokio::test]
async fn zip_с_несколькими_словарями_требует_выбора() -> Result<(), Box<dyn std::error::Error>> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in ["first.dsl", "second.dsl"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())?;
        zip.write_all(b"#NAME \"Fixture\"\nalpha\n\tBody\n")?;
    }
    let server = FixtureServer::spawn(Scenario::StableRange, zip.finish()?.into_inner()).await;
    let temp = tempfile::tempdir()?;
    let result = Command::new(env!("CARGO_BIN_EXE_converter-cli"))
        .args(["convert", "--url", &server.url()])
        .arg("--cache")
        .arg(temp.path().join("cache"))
        .arg("--output")
        .arg(temp.path().join("output"))
        .output()
        .await?;
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stdout)?.contains("dictionary_selection_required"));
    assert_eq!(std::fs::read_dir(temp.path().join("output"))?.count(), 0);
    Ok(())
}
