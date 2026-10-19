use crate::{Encoding, Profile};
use converter_core::{
    model::*,
    pipeline::{convert_to_staging, publish_local},
    prepare::prepare_source_with_options,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) fn prepared(path: PathBuf, id: String) -> Result<PreparedFile> {
    let path = std::fs::canonicalize(path)?;
    let display_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            ConverterError::new("invalid_source_name", "Имя источника должно быть Unicode")
        })?
        .to_owned();
    Ok(PreparedFile {
        id,
        display_name,
        path,
    })
}

fn label(stage: ConversionStage) -> &'static str {
    match stage {
        ConversionStage::Inspect => "Проверка источника",
        ConversionStage::Read => "Чтение статей",
        ConversionStage::Sort => "Сортировка ключей",
        ConversionStage::WriteMdx => "Запись MDX",
        ConversionStage::WriteMdd => "Запись MDD",
        ConversionStage::Validate => "Проверка результата",
        ConversionStage::Publish => "Публикация",
    }
}

fn output_name(file: &PreparedFile) -> String {
    let path = Path::new(&file.display_name);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("dictionary");
    name.strip_suffix(".dz")
        .unwrap_or(name)
        .strip_suffix(".dsl")
        .unwrap_or(name)
        .to_owned()
}

pub(crate) fn run(
    source: PreparedFile,
    attachments: Vec<PreparedFile>,
    output: PathBuf,
    encoding: Encoding,
    profile: Profile,
    control: RunControl,
) -> Result<serde_json::Value> {
    std::fs::create_dir_all(&output)?;
    let output = std::fs::canonicalize(output)?;
    let working = tempfile::Builder::new()
        .prefix(".converter-")
        .tempdir_in(&output)?;
    let mut ctx = RunContext::new(working.path().join("scratch"));
    if matches!(profile, Profile::Mobile) {
        ctx.limits = Limits::mobile();
    }
    ctx.control = control;
    let previous = Mutex::new((None, Instant::now()));
    ctx.progress = Arc::new(move |event| match event {
        CoreEvent::Diagnostic(diagnostic) => {
            eprintln!(
                "{}",
                serde_json::json!({"phase": "conversion", "diagnostic": diagnostic})
            )
        }
        CoreEvent::Progress(progress) | CoreEvent::StageCompleted(progress) => {
            if let Ok(mut previous) = previous.lock() {
                let terminal = progress.total == Some(progress.completed);
                if previous.0 != Some(progress.stage)
                    || previous.1.elapsed() >= Duration::from_millis(200)
                    || terminal
                {
                    eprintln!(
                        "{}",
                        serde_json::json!({"phase": "conversion", "label": label(progress.stage), "progress": progress})
                    );
                    *previous = (Some(progress.stage), Instant::now());
                }
            }
        }
    });
    let encoding = match encoding {
        Encoding::Auto => EncodingChoice::Auto,
        Encoding::Utf8 => EncodingChoice::Manual("utf-8".into()),
        Encoding::Utf16le => EncodingChoice::Manual("utf-16le".into()),
        Encoding::Utf16be => EncodingChoice::Manual("utf-16be".into()),
        Encoding::Windows1251 => EncodingChoice::Manual("windows-1251".into()),
    };
    let bundles = prepare_source_with_options(
        &source,
        &attachments,
        &ctx,
        &ReadOptions {
            encoding: encoding.clone(),
        },
    )?;
    if bundles.len() > 1 {
        return Err(ConverterError::new(
            "dictionary_selection_required",
            "В ZIP несколько основных словарей. Выберите отдельный словарь в приложении или подготовьте ZIP с одним словарём для CLI",
        ));
    }
    let mut results = Vec::new();
    for (index, bundle) in bundles.into_iter().enumerate() {
        ctx.control.check()?;
        let name = output_name(&bundle.main);
        let request = ConversionRequest {
            bundle,
            options: ReadOptions {
                encoding: encoding.clone(),
            },
            metadata_override: None,
            staging_dir: working.path().join(format!("ready-{index}")),
        };
        let artifacts = convert_to_staging(&request, &ctx)?;
        let summary = artifacts.summary.clone();
        let directory = publish_local(artifacts, &output, &name, &ctx)?;
        results.push(serde_json::json!({"directory": directory, "summary": summary}));
    }
    Ok(serde_json::json!({"results": results}))
}
