use converter_app::{jobs::progress::aggregate_downloads, *};
use converter_core::model::*;
use converter_download::{DownloadProgress, TransferState};
use std::collections::BTreeMap;
fn snapshot() -> JobSnapshot {
    JobSnapshot {
        resource_attachments: vec![],
        id: "00000000-0000-4000-8000-000000000001".into(),
        display_name: "Англо-русский словарь".into(),
        source_bytes: Some(26_214_400),
        prepare_only: false,
        had_errors: false,
        output: OutputChoice::AppLibrary,
        generation: 2,
        run_id: "00000000-0000-4000-8000-000000000002".into(),
        sequence: 10,
        state: JobState::Queued,
        download: aggregate_downloads(vec![]),
        conversion: ConversionJobProgress::default(),
        result: None,
        export: ExportSnapshot::default(),
        error: None,
        input_choices: vec![],
        additional_dictionaries: None,
        diagnostic_count: 0,
        options: ReadOptions::default(),
        metadata: Some(DictionaryMetadata {
            title: "Англо-русский словарь".into(),
            source_encoding: "utf-8".into(),
            ..Default::default()
        }),
    }
}
fn file(total: Option<u64>, state: TransferState) -> DownloadProgress {
    DownloadProgress {
        source_id: "00000000-0000-4000-8000-000000000003".into(),
        state,
        received_bytes: 4 * 1024 * 1024 * 1024,
        confirmed_bytes: 4 * 1024 * 1024 * 1024,
        total_bytes: total,
        bytes_per_second: Some(2_097_152.0),
        restart_reason: None,
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut fixtures = BTreeMap::new();
    fixtures.insert("local", snapshot());
    let mut downloading = snapshot();
    downloading.state = JobState::Downloading;
    downloading.download = aggregate_downloads(vec![file(
        Some(8 * 1024 * 1024 * 1024),
        TransferState::Downloading,
    )]);
    fixtures.insert("downloading", downloading.clone());
    let mut unknown = downloading.clone();
    unknown.download.files.push(DownloadProgress {
        source_id: "00000000-0000-4000-8000-000000000004".into(),
        ..file(None, TransferState::Downloading)
    });
    unknown.download = aggregate_downloads(unknown.download.files);
    fixtures.insert("unknown", unknown);
    let mut paused = downloading;
    paused.state = JobState::DownloadPaused;
    paused.download.files[0].state = TransferState::Paused;
    paused.download.files[0].bytes_per_second = None;
    paused.download.files[0].restart_reason =
        Some("Сервер изменил файл; загрузка начнётся заново".into());
    fixtures.insert("paused", paused);
    let stage = StageProgress {
        stage: ConversionStage::WriteMdx,
        completed: 250,
        total: Some(1000),
        unit: ProgressUnit::Entries,
        articles: 1000,
        resources: 3,
    };
    let mut converting = snapshot();
    converting.state = JobState::Converting;
    converting.download = aggregate_downloads(vec![file(
        Some(4 * 1024 * 1024 * 1024),
        TransferState::Completed,
    )]);
    converting.conversion = ConversionJobProgress {
        status: ConversionStatus::Running,
        current: Some(stage.clone()),
        completed_stages: vec![StageProgress {
            stage: ConversionStage::Read,
            completed: 1000,
            total: Some(1000),
            ..stage.clone()
        }],
    };
    fixtures.insert("converting", converting.clone());
    let mut ready = converting;
    ready.state = JobState::Succeeded;
    ready.sequence = 100;
    ready.conversion.status = ConversionStatus::Succeeded;
    ready.result = Some(ResultInfo {
        id: "00000000-0000-4000-8000-000000000005".into(),
        files: ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"]
            .into_iter()
            .map(|name| ResultFile {
                name: name.into(),
                bytes: 1000,
            })
            .collect(),
        summary: ConversionSummary {
            source_articles: 1000,
            output_keys: 1200,
            resources: 3,
            ..Default::default()
        },
        can_open: true,
        can_export: true,
    });
    fixtures.insert("succeeded", ready);
    let mut stale = snapshot();
    stale.generation = 1;
    stale.sequence = 999;
    stale.state = JobState::Downloading;
    fixtures.insert("stale", stale);
    println!("{}", serde_json::to_string_pretty(&fixtures)?);
    Ok(())
}
