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
        source_name: String::new(),
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
        includes: vec![],
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
        includes: vec![],
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
        includes: vec![],
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
fn восстанавливает_пересечение_форматирования_без_потери_текста()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    let result = render_article(
        &article("[b]начало[i]оба[/b]курсив[/i]конец"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(
        result
            .html
            .contains("<strong>начало<em>оба</em></strong><em>курсив</em>конец")
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "repaired_markup");
    assert_eq!(
        result.diagnostics[0]
            .source_location
            .as_ref()
            .expect("Источник")
            .line,
        1
    );

    let result = render_article(
        &article("[i][lang id=1049]текст[/i]продолжение[/lang]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert_eq!(result.html.matches("lang=\"ru\"").count(), 2);
    assert!(result.html.contains("текст</span></em><span"));
    assert!(result.html.contains("продолжение</span>"));
    Ok(())
}

#[test]
fn закрытие_сокращения_сохраняет_его_цель_при_пересечении_оформления()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let abrv = temp.path().join("main_abrv.dsl");
    std::fs::write(&abrv, "обр.\n образное выражение\n")?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: Some(prepared(&abrv)),
        resources: vec![],
    };
    let rendered = render_article(
        &article("[m1][p][trn]обр.[/p] взявшись за руки[/trn][/m]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(rendered.html.contains("title=\"образное выражение\""));
    assert!(rendered.html.contains("<span class=\"dsl-translation\">обр.</span></abbr><span class=\"dsl-translation\"> взявшись за руки</span>"));
    assert_eq!(rendered.diagnostics.len(), 1);
    assert_eq!(rendered.diagnostics[0].code, "repaired_markup");
    assert_eq!(rendered.html.matches("<abbr").count(), 1);
    Ok(())
}

#[test]
fn пересекающиеся_сокращения_остаются_целыми_при_закрытии_внешнего_оформления()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let abrv = temp.path().join("main_abrv.dsl");
    std::fs::write(
        &abrv,
        "прям.\n прямое значение\nперен.\n переносное значение\n",
    )?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: Some(prepared(&abrv)),
        resources: vec![],
    };
    let rendered = render_article(
        &article(
            "[m1][p][i][c][trn][com]прям.[/i][/p], [p][i]перен.[/com][/c][/i][/p] третья линия[/trn][/m]",
        ),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert_eq!(rendered.html.matches("<abbr").count(), 2);
    assert!(rendered.html.contains("title=\"прямое значение\""));
    assert!(rendered.html.contains("title=\"переносное значение\""));
    assert_eq!(rendered.html.matches("прям.").count(), 1);
    assert_eq!(rendered.html.matches("перен.").count(), 1);
    assert!(rendered.html.contains(" третья линия</span>"));
    assert!(
        rendered
            .diagnostics
            .iter()
            .all(|d| d.code == "repaired_markup")
    );
    assert!(!rendered.diagnostics.is_empty());

    let valid = render_article(
        &article("[i]до [p]прям.[/p] после[/i]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(valid.diagnostics.is_empty());
    assert!(valid.html.contains("title=\"прямое значение\""));
    Ok(())
}

#[test]
fn оформление_продолжается_через_границы_абзацев() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    let result = render_article(
        &article("[m1][c][com]первый[/m]\n[m2][i]второй[/com][/c][/i] конец[/m]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert_eq!(result.html.matches("первый").count(), 1);
    assert_eq!(result.html.matches("второй").count(), 1);
    assert_eq!(result.html.matches(" конец").count(), 1);
    assert!(result.html.contains("dsl-indent-1"));
    assert!(result.html.contains("dsl-indent-2"));
    assert_eq!(result.html.matches("dsl-comment").count(), 3);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "repaired_markup")
    );
    let result = render_article(
        &article(
            "[c][com][lang id=1049]вступление\n[m1][p][i]суффикс[/lang][/com][/c][/i][/p][/m]",
        ),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert_eq!(result.html.matches("вступление").count(), 1);
    assert_eq!(result.html.matches("суффикс").count(), 1);
    assert_eq!(result.html.matches("<abbr").count(), 1);
    assert!(result.html.contains("dsl-indent-1"));
    Ok(())
}

#[test]
fn ударение_сохраняет_букву_и_цель_ссылки_без_служебных_маркеров()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    let rendered = render_article(
        &article("[ref]з[']а[/']мок[/ref]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(
        rendered
            .html
            .contains("entry://%D0%B7%D0%B0%D0%BC%D0%BE%D0%BA")
    );
    assert!(
        rendered
            .html
            .contains("з<span class=\"dsl-stress\">а</span>мок")
    );
    assert!(!rendered.html.contains("[']"));
    assert!(!rendered.html.contains("[/']"));
    assert!(rendered.diagnostics.is_empty());
    Ok(())
}

#[test]
fn факультативная_зона_сохраняет_весь_пример_видимым() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    let rendered = render_article(
        &article("до [*][ex]пример & пояснение[/ex][/*] после [новый]текст[/новый]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(rendered.html.contains("до <span class=\"dsl-optional\"><span class=\"dsl-example\">пример &amp; пояснение</span></span> после"));
    assert!(!rendered.html.contains("[*]"));
    assert!(!rendered.html.contains("[/*]"));
    assert!(rendered.html.contains("[новый]текст[/новый]"));
    assert_eq!(rendered.diagnostics.len(), 1);
    assert_eq!(rendered.diagnostics[0].code, "unknown_tag");
    Ok(())
}

#[test]
fn восстановление_оформления_не_подменяет_ошибку_ссылки() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    for body in [
        "[ref][i]текст[/ref][/i]",
        "[i][ref]текст[/i][/ref]",
        "[b][url]https://example.com[/b][/url]",
        "[s][i]file.png[/s][/i]",
        "[/p]текст",
        "[/b]текст",
        "[/ref]текст",
        "[s]file.png",
    ] {
        assert_eq!(
            render_article(&article(body), &bundle, &ReadOptions::default())
                .expect_err("Повреждённая ссылка или ресурс")
                .code,
            "invalid_markup"
        );
    }
    Ok(())
}

#[test]
fn восстановление_соблюдает_границы_вложенности_и_размера_html()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("main.dsl");
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&path),
        abbreviations: None,
        resources: vec![],
    };
    let body = format!("[i]{}маркер[/i]{}", "[b]".repeat(127), "[/b]".repeat(127));
    let rendered = render_article(&article(&body), &bundle, &ReadOptions::default())?;
    assert_eq!(rendered.html.matches("маркер").count(), 1);
    assert_eq!(rendered.diagnostics.len(), 1);
    assert_eq!(rendered.diagnostics[0].code, "repaired_markup");
    let too_deep = format!("[b]{body}[/b]");
    assert_eq!(
        render_article(&article(&too_deep), &bundle, &ReadOptions::default())
            .expect_err("Глубина 129 недопустима")
            .code,
        "invalid_markup"
    );

    std::fs::write(&path, format!("word\n {body}\n"))?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.max_html_bytes = 1024;
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundle, &ReadOptions::default(), &mut sink, &ctx)
            .expect_err("Восстановленная статья также ограничена размером HTML")
            .code,
        "article_too_large"
    );
    assert!(sink.entries.is_empty());
    Ok(())
}

#[test]
fn восстановление_оформления_не_исправляет_опасный_путь_ресурса()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = InputBundle {
        includes: vec![],
        main: prepared(&temp.path().join("main.dsl")),
        abbreviations: None,
        resources: vec![],
    };
    assert_eq!(
        render_article(
            &article("[b]начало[i]текст[/b] ещё[/i][s]../secret.png[/s]"),
            &bundle,
            &ReadOptions::default(),
        )
        .expect_err("Путь с обходом каталога запрещён")
        .code,
        "unsafe_archive_path"
    );
    Ok(())
}

#[test]
fn неизвестный_тип_ресурса_не_становится_исполняемой_страницей()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("page.html");
    std::fs::write(&path, "<script>alert(1)</script>")?;
    let bundle = InputBundle {
        includes: vec![],
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
        includes: vec![],
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

#[test]
fn отступ_m_заканчивается_следующим_абзацем_или_концом_статьи()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("main.dsl");
    std::fs::write(&file, "word\n [m2]opening[m1]first[/m][m1]last\n")?;
    let bundle = InputBundle {
        includes: vec![],
        main: PreparedFile {
            id: "main".into(),
            display_name: "main.dsl".into(),
            path: file,
        },
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
    let html = &sink.entries[0].html;
    assert!(html.contains("opening") && html.contains("first") && html.contains("last"));
    assert_eq!(html.matches("<div class=\"dsl-indent ").count(), 3);
    Ok(())
}

#[test]
fn повторное_закрытие_абзаца_не_теряет_текст() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("main.dsl");
    std::fs::write(&path, "word\n [m2]before[m1]first[/m]after[/m]\n")?;
    let bundle = InputBundle {
        includes: vec![],
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
    assert!(sink.entries[0].html.contains("after"));
    Ok(())
}
