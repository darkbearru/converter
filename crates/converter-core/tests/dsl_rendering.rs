mod support;

use converter_core::{
    dsl::{DslReader, parser::ArticleAst, renderer::render_article},
    formats::DictionaryReader,
    model::{InputBundle, PreparedFile, ReadOptions, ResourceRecord, RunContext},
};
use std::path::Path;
use support::collect_sink::CollectSink;

fn prepared(path: &Path) -> PreparedFile {
    PreparedFile {
        id: "main".into(),
        display_name: path
            .file_name()
            .expect("Имя файла")
            .to_string_lossy()
            .into_owned(),
        path: path.into(),
    }
}
fn article(body: &str) -> ArticleAst {
    ArticleAst {
        id: 0,
        source_line: 1,
        parent_id: None,
        headwords: vec!["{the }colo(u)r".into()],
        body: body.into(),
    }
}

#[test]
fn сохраняет_семантику_и_экранирует_текст() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("test.dsl");
    let abrv = temp.path().join("test_abrv.dsl");
    std::fs::write(&main, "word\n body")?;
    std::fs::write(&abrv, "n.\n [b]существительное[/b]\n")?;
    let image = temp.path().join("image.bin");
    let audio = temp.path().join("audio.bin");
    std::fs::write(&image, b"PNG")?;
    std::fs::write(&audio, b"MP3")?;
    let bundle = InputBundle {
        main: prepared(&main),
        abbreviations: Some(prepared(&abrv)),
        resources: vec![
            ResourceRecord {
                key: "\\media\\Image.png".into(),
                path: image,
            },
            ResourceRecord {
                key: "\\media\\voice.mp3".into(),
                path: audio,
            },
        ],
    };
    let ast = article(
        r#"[m1][b]~ [i]курсив[/i][/b][/m]
[u]подчёркнутый[/u] [c red]красный[/c] [p]n.[/p] [p]missing[/p]
[trn]перевод[/trn] [ex]пример[/ex] [com]комментарий[/com]
[sup]верх[/sup] [sub]низ[/sub] [t]ˈwɜːd[/t] [lang id=1033]English[/lang]
[ref]яблоко[/ref] <<другой ключ>> [ref target="target"]ссылка[/ref]
[url]https://example.com?q=a&b=c[/url] [url target="javascript:alert(1)"]плохая ссылка[/url]
[s]media/image.png[/s] [s]media/voice.mp3[/s] [s]missing.jpg[/s]
\[b\]буквально\[/b\] \~ <script>alert(1)</script>
[неизвестный]русский текст[/неизвестный]"#,
    );
    let rendered = render_article(&ast, &bundle, &ReadOptions::default())?;
    assert!(!rendered.html.contains("<script>"));
    assert!(rendered.html.contains("&lt;script&gt;"));
    assert!(
        rendered
            .html
            .contains("<strong>the colour <em>курсив</em></strong>")
    );
    assert!(rendered.html.contains("[b]буквально[/b] ~"));
    assert!(rendered.html.contains("entry://"));
    assert!(rendered.html.contains("entry://target"));
    assert!(rendered.html.contains("sound://media/voice.mp3"));
    assert!(rendered.html.contains("src=\"media/Image.png\""));
    assert!(
        rendered
            .resource_keys
            .contains(&"\\media\\voice.mp3".to_owned())
    );
    assert!(rendered.html.contains("converter.css"));
    assert!(rendered.html.contains("существительное"));
    assert!(rendered.html.contains("lang=\"en\""));
    for class in [
        "dsl-indent-1",
        "dsl-underline",
        "dsl-color",
        "dsl-translation",
        "dsl-example",
        "dsl-comment",
        "dsl-transcription",
    ] {
        assert!(
            rendered.html.contains(class),
            "Не сохранено оформление {class}"
        );
    }
    assert!(rendered.html.contains("<sup>верх</sup>"));
    assert!(rendered.html.contains("<sub>низ</sub>"));
    assert!(
        rendered
            .html
            .contains("[неизвестный]русский текст[/неизвестный]")
    );
    assert!(rendered.html.contains("Отсутствует ресурс"));
    assert!(!rendered.html.contains("href=\"javascript:"));
    for code in [
        "unknown_tag",
        "unsafe_url",
        "missing_resource",
        "resource_name_fallback",
        "missing_abbreviation",
    ] {
        assert!(
            rendered.diagnostics.iter().any(|d| d.code == code),
            "Нет диагностики {code}"
        );
    }
    Ok(())
}

#[test]
fn незакрытый_тег_и_слишком_большой_html_дают_ошибку() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    assert_eq!(
        render_article(&article("[b]не закрыт"), &bundle, &ReadOptions::default())
            .expect_err("Не закрыт тег")
            .code,
        "invalid_markup"
    );
    let large = article(&"&".repeat(2 * 1024 * 1024));
    assert_eq!(
        render_article(&large, &bundle, &ReadOptions::default())
            .expect_err("Лимит HTML")
            .code,
        "article_too_large"
    );
    Ok(())
}

#[test]
fn экранирование_и_подстановка_работают_в_целях_ссылок() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    let rendered = render_article(
        &article(r#"[ref target="a\]b"]скобка[/ref] <<~>> <<\~>> <<a\>>b>>"#),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(rendered.html.contains("entry://a%5Db"));
    assert!(rendered.html.contains("entry://the%20colour"));
    assert!(rendered.html.contains("entry://~"));
    assert!(rendered.html.contains("entry://a%3E%3Eb"));
    Ok(())
}

#[test]
fn неизвестный_тип_ресурса_не_становится_исполняемой_страницей()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("page.html");
    std::fs::write(&path, "<script>alert(1)</script>")?;
    let bundle = InputBundle {
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![ResourceRecord {
            key: "\\page.html".into(),
            path,
        }],
    };
    let rendered = render_article(
        &article("[s]page.html[/s]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(!rendered.html.contains("href=\"page.html\""));
    assert!(
        rendered
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported_resource_type")
    );
    Ok(())
}

#[test]
fn читатель_подключает_рендерер_и_сохраняет_порядок() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("main.dsl");
    std::fs::write(
        &path,
        "colo(u)r\n~ test\n [b]~[/b]\n @child\n  [i]дочерняя[/i]\n @\ncolor\n второе значение\n",
    )?;
    let bundle = InputBundle {
        main: prepared(&path),
        abbreviations: None,
        resources: vec![],
    };
    let mut sink = CollectSink::default();
    DslReader.read(
        &bundle,
        &ReadOptions::default(),
        &mut sink,
        &RunContext::new(temp.path().join("work")),
    )?;
    assert_eq!(sink.entries.len(), 3);
    assert_eq!(
        sink.entries[0].headwords,
        ["color", "colour", "color test", "colour test"]
    );
    assert!(sink.entries[0].html.contains("<strong>colour</strong>"));
    assert_eq!(sink.entries[1].parent_id, Some(sink.entries[0].id));
    assert!(sink.entries[1].html.contains("<em>дочерняя</em>"));
    assert_eq!(sink.entries[2].headwords, ["color"]);
    assert!(sink.diagnostics.is_empty());
    Ok(())
}
