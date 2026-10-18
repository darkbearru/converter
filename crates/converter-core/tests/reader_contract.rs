use converter_core::{
    formats::{
        DictionaryReader, EntrySink, FormatDescriptor, FormatRegistry, ProbeResult, ReadSummary,
    },
    model::{DictionaryMetadata, InputBundle, ReadOptions, Result, RunContext},
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
        _: &mut dyn EntrySink,
        _: &RunContext,
    ) -> Result<ReadSummary> {
        Ok(ReadSummary {
            source_articles: 0,
            warnings: 0,
        })
    }
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
