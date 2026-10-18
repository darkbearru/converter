use super::{report::ReportLog, spool::EntrySpool, validate::validate_artifacts};
use crate::{
    dsl::renderer::CONVERTER_CSS,
    formats::FormatRegistry,
    mdict::{collation::mdd_sort_key, write_mdd, write_mdx},
    model::*,
    prepare::unique_resources,
};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

struct ReadyDirectory {
    path: PathBuf,
    keep: bool,
}
impl ReadyDirectory {
    fn create(path: &Path) -> Result<Self> {
        std::fs::create_dir(path)?;
        Ok(Self {
            path: path.into(),
            keep: false,
        })
    }
}
impl Drop for ReadyDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub fn convert_to_staging(
    request: &ConversionRequest,
    ctx: &RunContext,
) -> Result<ConvertedArtifacts> {
    convert_to_staging_with_registry(request, ctx, &FormatRegistry::builtins())
}

pub fn convert_to_staging_with_registry(
    request: &ConversionRequest,
    ctx: &RunContext,
    registry: &FormatRegistry,
) -> Result<ConvertedArtifacts> {
    ctx.control.check()?;
    std::fs::create_dir_all(&ctx.scratch_dir)?;
    let mut directory = ReadyDirectory::create(&request.staging_dir)?;
    let working = tempfile::Builder::new()
        .prefix("conversion-")
        .tempdir_in(&ctx.scratch_dir)?;
    let report = ReportLog::create(&working.path().join("diagnostics.jsonl"))?;
    let mut ctx = ReportLog::context(&report, ctx);
    ctx.scratch_dir = working.path().to_owned();
    let mut prefix = Vec::new();
    File::open(&request.bundle.main.path)?
        .take(64 * 1024)
        .read_to_end(&mut prefix)?;
    let format = registry.detect(&prefix, &request.bundle.main.display_name)?;
    let reader = registry.get(&format)?;
    let mut metadata = reader.inspect(&request.bundle, &request.options, &ctx)?;
    let mut options = request.options.clone();
    if matches!(options.encoding, EncodingChoice::Auto) && !metadata.source_encoding.is_empty() {
        options.encoding = EncodingChoice::Manual(metadata.source_encoding.clone());
    }
    if let Some(override_meta) = &request.metadata_override {
        if !override_meta.title.is_empty() {
            metadata.title.clone_from(&override_meta.title);
        }
        metadata.description.clone_from(&override_meta.description);
        if !override_meta.index_language.is_empty() {
            metadata
                .index_language
                .clone_from(&override_meta.index_language);
        }
        if !override_meta.contents_language.is_empty() {
            metadata
                .contents_language
                .clone_from(&override_meta.contents_language);
        }
    }
    let mut spool = EntrySpool::new(working.path(), report.clone(), &ctx)?;
    let read_summary = reader.read(&request.bundle, &options, &mut spool, &ctx)?;
    if read_summary.source_articles != spool.articles {
        return Err(ConverterError::new(
            "count_mismatch",
            "Число статей читателя не совпало с промежуточным хранилищем",
        ));
    }
    let (records, metrics) = spool.finish(&request.bundle.main.display_name)?;
    let mdx = directory.path.join("dictionary.mdx");
    let mdx_summary = {
        let mut file = File::create(&mdx)?;
        let summary = write_mdx(records, &metadata, &mut file, &ctx)?;
        file.sync_all()?;
        summary
    };
    if mdx_summary.entries != spool.keys {
        return Err(ConverterError::new(
            "count_mismatch",
            "При записи MDX изменилось число ключей",
        ));
    }
    let css = working.path().join("converter.css");
    std::fs::write(&css, CONVERTER_CSS.as_bytes())?;
    let mut resources = request.bundle.resources.clone();
    resources.push(ResourceRecord {
        key: "\\converter.css".into(),
        path: css,
    });
    let mut resources = unique_resources(resources, &ctx)?;
    resources.sort_by_cached_key(|resource| mdd_sort_key(&resource.key));
    let resource_count = resources.len() as u64;
    let mdd = directory.path.join("dictionary.mdd");
    let mdd_summary = {
        let mut file = File::create(&mdd)?;
        let summary = write_mdd(resources.into_iter().map(Ok), &mut file, &ctx)?;
        file.sync_all()?;
        summary
    };
    if mdd_summary.entries != resource_count {
        return Err(ConverterError::new(
            "count_mismatch",
            "При записи MDD изменилось число ресурсов",
        ));
    }
    let mut artifacts = ConvertedArtifacts {
        directory: directory.path.clone(),
        mdx,
        mdd,
        report: directory.path.join("conversion-report.json"),
        summary: ConversionSummary {
            source_articles: spool.articles,
            output_keys: spool.keys,
            resources: resource_count,
            warnings: 0,
            stages: Vec::new(),
            sort_metrics: metrics,
        },
    };
    validate_artifacts(&artifacts, &ctx)?;
    (artifacts.summary.warnings, artifacts.summary.stages) = ReportLog::snapshot(&report)?;
    ReportLog::write(
        &report,
        &artifacts.report,
        &metadata,
        &artifacts.summary,
        &ctx,
    )?;
    ctx.control.check()?;
    directory.keep = true;
    Ok(artifacts)
}
