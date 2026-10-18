use crate::{Encoding, Profile};
use converter_core::{
    model::*,
    pipeline::{convert_to_staging, publish_local},
    prepare::prepare_source,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn prepared(path: PathBuf, id: String) -> Result<PreparedFile> {
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
    source: PathBuf,
    resources: Vec<PathBuf>,
    output: PathBuf,
    encoding: Encoding,
    profile: Profile,
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
    let control = ctx.control.clone();
    ctrlc::set_handler(move || control.cancel()).map_err(|error| {
        ConverterError::new(
            "signal_handler",
            format!("Не удалось включить обработчик Ctrl-C: {error}"),
        )
    })?;
    let previous = Mutex::new((None, Instant::now()));
    ctx.progress = Arc::new(move |event| match event {
        CoreEvent::Diagnostic(diagnostic) => {
            eprintln!("Предупреждение: {}", diagnostic.message.escape_debug())
        }
        CoreEvent::Progress(progress) | CoreEvent::StageCompleted(progress) => {
            if let Ok(mut previous) = previous.lock() {
                let terminal = progress.total == Some(progress.completed);
                if previous.0 != Some(progress.stage)
                    || previous.1.elapsed() >= Duration::from_millis(200)
                    || terminal
                {
                    let unit = match progress.unit {
                        ProgressUnit::Bytes => "байт",
                        ProgressUnit::Entries => "записей",
                        ProgressUnit::Resources => "ресурсов",
                    };
                    match progress.total {
                        Some(total) => eprintln!(
                            "{}: {}/{} {}",
                            label(progress.stage),
                            progress.completed,
                            total,
                            unit
                        ),
                        None => {
                            eprintln!("{}: {} {}", label(progress.stage), progress.completed, unit)
                        }
                    }
                    *previous = (Some(progress.stage), Instant::now());
                }
            }
        }
    });
    let source = prepared(source, "main".into())?;
    let attachments = resources
        .into_iter()
        .enumerate()
        .map(|(index, path)| prepared(path, format!("attachment-{index}")))
        .collect::<Result<Vec<_>>>()?;
    let bundles = prepare_source(&source, &attachments, &ctx)?;
    let encoding = match encoding {
        Encoding::Auto => EncodingChoice::Auto,
        Encoding::Utf8 => EncodingChoice::Manual("utf-8".into()),
        Encoding::Utf16le => EncodingChoice::Manual("utf-16le".into()),
        Encoding::Utf16be => EncodingChoice::Manual("utf-16be".into()),
        Encoding::Windows1251 => EncodingChoice::Manual("windows-1251".into()),
    };
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
