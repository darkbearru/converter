use converter_core::{
    formats::{
        DictionaryReader, EntrySink, FormatDescriptor, FormatRegistry, ProbeResult, ReadSummary,
    },
    model::{
        ConversionRequest, DictionaryEntry, DictionaryMetadata, InputBundle, PreparedFile,
        ReadOptions, Result, RunContext,
    },
    pipeline::convert_to_staging_with_registry,
    prepare::prepare_source_with_registry,
};

struct OtherReader;
impl DictionaryReader for OtherReader {
    fn descriptor(&self) -> FormatDescriptor {
        FormatDescriptor {
            id: "test-other".into(),
            name: "Тестовый формат".into(),
            extensions: vec!["other".into()],
            supports_resources: false,
        }
    }
    fn probe(&self, prefix: &[u8], _: &str) -> ProbeResult {
        ProbeResult {
            confidence: if prefix.starts_with(b"OTHER") { 100 } else { 0 },
        }
    }
    fn inspect(
        &self,
        _: &InputBundle,
        _: &ReadOptions,
        _: &RunContext,
    ) -> Result<DictionaryMetadata> {
        Ok(DictionaryMetadata::default())
    }
    fn read(
        &self,
        _: &InputBundle,
        _: &ReadOptions,
        sink: &mut dyn EntrySink,
        _: &RunContext,
    ) -> Result<ReadSummary> {
        sink.push(DictionaryEntry {
            source_name: String::new(),
            id: 1,
            source_line: 2,
            parent_id: None,
            headwords: vec!["other".into()],
            display_headword: "other".into(),
            html: "<p>Другой формат</p>".into(),
            resource_keys: vec![],
        })?;
        Ok(ReadSummary {
            source_articles: 1,
            warnings: 0,
        })
    }
}

#[test]
fn новый_reader_проходит_из_zip_до_читаемого_mdx()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, io::Write};
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source.zip");
    let mut zip = zip::ZipWriter::new(File::create(&source)?);
    zip.start_file("source.other", zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"OTHER\nentry\n")?;
    zip.start_file("image.png", zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"resource")?;
    zip.finish()?;
    let file = PreparedFile {
        id: "archive".into(),
        display_name: "source.zip".into(),
        path: source,
    };
    let ctx = RunContext::new(temp.path().join("scratch"));
    let mut registry = FormatRegistry::builtins();
    registry.register(OtherReader)?;
    let bundles = prepare_source_with_registry(&file, &[], &ctx, &registry)?;
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].main.display_name, "source.other");
    let result = convert_to_staging_with_registry(
        &ConversionRequest {
            bundle: bundles.into_iter().next().ok_or("Словарь")?,
            options: ReadOptions::default(),
            metadata_override: None,
            staging_dir: temp.path().join("ready"),
        },
        &ctx,
        &registry,
    )?;
    assert_eq!(result.summary.source_articles, 1);
    let reader = mdictlib::MdxFile::open(&result.mdx)?;
    assert_eq!(
        reader.lookup("other")?.ok_or("Статья")?.text(),
        "<p>Другой формат</p>"
    );
    Ok(())
}

#[test]
fn реестр_не_требует_изменения_pipeline() -> Result<()> {
    let mut registry = FormatRegistry::builtins();
    assert_eq!(registry.descriptors().len(), 1);
    registry.register(OtherReader)?;
    assert_eq!(registry.detect(b"OTHER\ndata", "source")?, "test-other");
    assert_eq!(
        registry.get("test-other")?.descriptor().name,
        "Тестовый формат"
    );
    assert_eq!(
        registry.detect(b"#NAME \"Test\"\nword\n body", "source")?,
        "dsl"
    );
    assert_eq!(
        registry
            .detect(b"<html><body>Login</body></html>", "source")
            .expect_err("HTML не словарь")
            .code,
        "unsupported_format"
    );
    assert_eq!(
        registry
            .detect(&[0, 9, 3, 7], "unknown")
            .expect_err("Неизвестный формат")
            .code,
        "unsupported_format"
    );
    Ok(())
}

#[test]
fn ограниченный_probe_учитывает_расширение_после_длинного_комментария() -> Result<()> {
    let registry = FormatRegistry::builtins();
    let prefix = format!("{{{{{}", "a".repeat(70_000));
    assert_eq!(registry.detect(prefix.as_bytes(), "long.dsl")?, "dsl");
    assert_eq!(
        registry
            .detect(b"<html>Login</html>", "login.dsl")
            .expect_err("HTML даже с расширением DSL")
            .code,
        "unsupported_format"
    );
    assert_eq!(
        registry.detect(
            &[
                0xff, 0xfe, b'#', 0, b'N', 0, b'A', 0, b'M', 0, b'E', 0, b' ', 0
            ],
            "unknown"
        )?,
        "dsl"
    );
    Ok(())
}
