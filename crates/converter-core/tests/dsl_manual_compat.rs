mod support;

use converter_core::{
    dsl::{DslReader, headwords::expand_headwords, parser::ArticleAst, renderer::render_article},
    formats::DictionaryReader,
    model::*,
    prepare::prepare_source,
};
use std::path::Path;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn source(path: &Path) -> PreparedFile {
    PreparedFile {
        id: path.to_string_lossy().into(),
        display_name: path
            .file_name()
            .expect("Имя файла источника")
            .to_string_lossy()
            .into(),
        path: path.into(),
    }
}

fn render(headword: &str, body: &str) -> Result<converter_core::dsl::renderer::RenderedArticle> {
    let bundle = InputBundle {
        main: source(Path::new("test.dsl")),
        annotation: None,
        includes: vec![],
        abbreviations: None,
        resources: vec![],
    };
    render_article(
        &ArticleAst {
            source_name: "test.dsl".into(),
            id: 0,
            source_line: 1,
            parent_id: None,
            headwords: vec![headword.into()],
            body: body.into(),
        },
        &bundle,
        &ReadOptions::default(),
    )
}

#[test]
fn br_переносит_строку_внутри_оформления() -> TestResult {
    let result = render("word", r"[b]first[br]second[/b] \[br\]")?;
    assert!(
        result
            .html
            .contains("<strong>first<br>second</strong> [br]")
    );
    assert!(result.diagnostics.is_empty());
    Ok(())
}

#[test]
fn перевёртыш_меняет_регистр_подстановки_и_обычного_символа() -> TestResult {
    let result = render(
        "Кубарем",
        r#"Скатиться ^~. ^ж ^Q \^~ <<^~>> [ref target="^~"]go[/ref]"#,
    )?;
    assert!(result.html.contains("Скатиться кубарем. Ж q ^Кубарем"));
    assert!(
        result
            .html
            .contains("entry://%D0%BA%D1%83%D0%B1%D0%B0%D1%80%D0%B5%D0%BC")
    );
    assert_eq!(
        result
            .html
            .matches("entry://%D0%BA%D1%83%D0%B1%D0%B0%D1%80%D0%B5%D0%BC")
            .count(),
        2
    );
    Ok(())
}

#[test]
fn необязательная_часть_видна_со_скобками_но_ключи_содержат_четыре_варианта() -> TestResult {
    let result = render("(пре)вращать(ся)", "~")?;
    assert!(
        result
            .html
            .contains("<header class=\"dsl-headword\">(пре)вращать(ся)</header>")
    );
    assert!(
        result
            .html
            .contains("<div class=\"dsl-body\">(пре)вращать(ся)</div>")
    );
    let mut keys = expand_headwords("(пре)вращать(ся)", 64)?;
    keys.sort();
    assert_eq!(keys, ["вращать", "вращаться", "превращать", "превращаться"]);
    Ok(())
}

#[test]
fn разметка_несортируемой_части_рисуется_а_экранированные_теги_остаются_текстом() -> TestResult {
    let headword = r"word{ [b]noun[/b][br]}\[i\]";
    let result = render(headword, "~")?;
    assert!(
        result
            .html
            .contains("<header class=\"dsl-headword\">word <strong>noun</strong><br>[i]</header>")
    );
    assert!(
        result
            .html
            .contains("<div class=\"dsl-body\">word noun\n[i]</div>")
    );
    assert_eq!(expand_headwords(headword, 64)?, ["word[i]"]);
    Ok(())
}

#[test]
fn include_декодирует_удвоенные_слеши_в_относительном_пути() -> TestResult {
    let temp = tempfile::tempdir()?;
    std::fs::create_dir(temp.path().join("Extra"))?;
    std::fs::write(temp.path().join("Extra/part.dsl"), "part\n body\n")?;
    let main = temp.path().join("main.dsl");
    std::fs::write(&main, "#INCLUDE \"Extra\\\\part.dsl\"\nroot\n body\n")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundles = prepare_source(&source(&main), &[], &ctx)?;
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].includes.len(), 1);
    assert_eq!(
        bundles[0].includes[0].path,
        temp.path().join("Extra/part.dsl").canonicalize()?
    );
    Ok(())
}

#[test]
fn include_после_декодирования_не_разрешает_выход_за_папку() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    for target in [
        r"..\\outside.dsl",
        r"C:\\outside.dsl",
        r"\\\\server\\dict.dsl",
    ] {
        std::fs::write(&main, format!("#INCLUDE \"{target}\"\nroot\n body\n"))?;
        assert_eq!(
            prepare_source(&source(&main), &[], &ctx)
                .expect_err("Выход из папки запрещён")
                .code,
            "unsafe_include_path"
        );
    }
    Ok(())
}

#[test]
fn межсловарная_ссылка_не_теряет_имя_словаря_и_сообщает_об_ограничении() -> TestResult {
    let result = render("word", "[ref dict=\"Other & dictionary\"]target[/ref]")?;
    assert!(
        result
            .html
            .contains("data-dsl-dictionary=\"Other &amp; dictionary\"")
    );
    assert!(result.html.contains("href=\"entry://target\""));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "external_dictionary_reference")
    );
    Ok(())
}

#[test]
fn ссылка_на_свой_словарь_внутренняя_а_внешний_словарь_предупреждает_один_раз() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    std::fs::write(
        &main,
        "#NAME \"Our dictionary\"\none\n [ref dict=\"Our dictionary\"]two[/ref] [ref dict=\"Other\"]one[/ref]\ntwo\n [ref dict=\"Other\"]two[/ref]\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(&main), &[], &ctx)?.remove(0);
    let mut sink = support::collect_sink::CollectSink::default();
    DslReader.read(&bundle, &ReadOptions::default(), &mut sink, &ctx)?;
    let diagnostics: Vec<_> = sink
        .diagnostics
        .iter()
        .filter(|d| d.code == "external_dictionary_reference")
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message_args["dictionary"], "Other");
    assert!(
        sink.entries[0]
            .html
            .contains("data-dsl-dictionary=\"Our dictionary\"")
    );
    Ok(())
}

#[test]
fn команды_тела_не_меняют_литеральные_символы_в_заголовке_и_названии_словаря() -> TestResult {
    let result = render("A^B~", r#"~ [ref dict="Other ^Name \~"]target[/ref]"#)?;
    assert!(
        result
            .html
            .contains("<header class=\"dsl-headword\">A^B~</header>")
    );
    assert!(result.html.contains("<div class=\"dsl-body\">A^B~ "));
    assert!(
        result
            .html
            .contains("data-dsl-dictionary=\"Other ^Name ~\"")
    );
    Ok(())
}

#[test]
fn литеральная_крышка_перед_закрытием_тега_не_поглощает_разметку() -> TestResult {
    // Реальная статья control code из LingvoComputerEnRu.dsl.dz.
    let result = render(
        "control code",
        r#"[m1][trn]символ [com]([i]код [lang id=1033]^[/i])[/lang][/com][/trn][/m] [ref target="a^]"]go[/ref]"#,
    )?;
    assert!(result.html.contains("^</span></em>"));
    assert!(result.html.contains("href=\"entry://a%5E%5D\""));
    Ok(())
}

#[test]
fn внешние_ссылки_не_считаются_отсутствующими_статьями_локального_словаря() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    std::fs::write(
        &main,
        "#NAME \"Main\"\nroot\n [ref dict=\"Other\"]foreign-one[/ref] [ref dict=\"Other\"]foreign-two[/ref] [ref]missing-local[/ref] [ref dict=\"Main\"]missing-self[/ref]\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(&main), &[], &ctx)?.remove(0);
    let artifacts = converter_core::pipeline::convert_to_staging(
        &ConversionRequest {
            bundle,
            options: ReadOptions::default(),
            metadata_override: None,
            staging_dir: temp.path().join("ready"),
        },
        &ctx,
    )?;
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(artifacts.report)?)?;
    let diagnostics = report["diagnostics"]
        .as_array()
        .ok_or("Диагностика отчёта")?;
    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "missing_reference")
        .map(|d| d["message_args"]["target"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(missing, ["missing-local", "missing-self"]);
    assert_eq!(artifacts.summary.warnings, 3);
    Ok(())
}
