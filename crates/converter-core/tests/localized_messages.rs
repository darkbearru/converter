mod support;
use converter_core::{
    dsl::DslReader, formats::DictionaryReader, model::*, pipeline::validate_artifacts,
};
use support::collect_sink::CollectSink;

#[test]
fn ошибки_структуры_dsl_содержат_ключ_перевода()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("source.dsl");
    let bundle = InputBundle {
        includes: vec![],
        main: PreparedFile {
            id: "main".into(),
            display_name: "source.dsl".into(),
            path: path.clone(),
        },
        abbreviations: None,
        resources: vec![],
    };
    for (text, key) in [
        (" orphan body\n", "messages.dsl_body_without_heading"),
        ("word\n", "messages.dsl_heading_without_body"),
        ("{{comment\n", "messages.dsl_unclosed_comment"),
    ] {
        std::fs::write(&path, text)?;
        let error = DslReader
            .read(
                &bundle,
                &ReadOptions::default(),
                &mut CollectSink::default(),
                &RunContext::new(temp.path().join("work")),
            )
            .expect_err("invalid DSL");
        assert_eq!(error.message_key.as_deref(), Some(key));
    }
    Ok(())
}

#[test]
fn проверка_mdict_сохраняет_ключи_ошибок_и_параметры_io()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let artifacts = ConvertedArtifacts {
        directory: temp.path().into(),
        mdx: temp.path().join("dictionary.mdx"),
        mdd: temp.path().join("dictionary.mdd"),
        report: temp.path().join("report.json"),
        summary: ConversionSummary::default(),
    };
    let ctx = RunContext::new(temp.path().join("work"));
    let error = validate_artifacts(&artifacts, &ctx).expect_err("missing file");
    assert_eq!(error.code, "invalid_mdict");
    assert_eq!(error.message_key.as_deref(), Some("messages.io_error"));
    assert!(error.message_args.contains_key("value"));
    std::fs::write(&artifacts.mdx, [0; 64])?;
    std::fs::write(&artifacts.mdd, [0; 64])?;
    let error = validate_artifacts(&artifacts, &ctx).expect_err("invalid header");
    assert_eq!(
        error.message_key.as_deref(),
        Some("messages.mdict_corrupt_header")
    );
    Ok(())
}
