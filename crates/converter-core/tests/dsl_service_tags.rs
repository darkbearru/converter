use converter_core::{
    dsl::{parser::ArticleAst, renderer::render_article},
    model::{InputBundle, PreparedFile, ReadOptions, ResourceRecord},
};
use std::path::Path;

fn bundle(directory: &Path) -> InputBundle {
    InputBundle {
        main: PreparedFile {
            id: "main".into(),
            display_name: "main.dsl".into(),
            path: directory.join("main.dsl"),
        },
        abbreviations: None,
        resources: vec![],
    }
}

fn article(body: &str) -> ArticleAst {
    ArticleAst {
        id: 0,
        source_line: 1,
        parent_id: None,
        headwords: vec!["word".into()],
        body: body.into(),
    }
}

#[test]
fn служебные_теги_сохраняют_текст_и_оформление_без_предупреждений()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    for tag in ["!trs", "trs", "!trn"] {
        let body = format!("до [{tag}][b]слово & <текст>[/b] [i]~[/i][/{tag}] после");
        let rendered = render_article(&article(&body), &bundle, &ReadOptions::default())?;
        assert!(rendered.html.contains(
            "<div class=\"dsl-body\">до <strong>слово &amp; &lt;текст&gt;</strong> <em>word</em> после</div>"
        ), "Изменён видимый текст или оформление [{tag}]: {}", rendered.html);
        assert!(
            rendered.diagnostics.is_empty(),
            "Лишняя диагностика [{tag}]: {:?}",
            rendered.diagnostics
        );
    }
    Ok(())
}

#[test]
fn служебные_теги_сохраняют_цели_ссылок_и_подключённые_ресурсы()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let mut bundle = bundle(temp.path());
    let resource = temp.path().join("image.png");
    std::fs::write(&resource, b"PNG")?;
    bundle.resources.push(ResourceRecord {
        key: "\\media\\image.png".into(),
        path: resource,
    });
    for tag in ["!trs", "trs", "!trn"] {
        let body = format!(
            "[ref][{tag}][b]target[/b][/{tag}][/ref] [{tag}][url target=\"https://example.com\"][i]сайт[/i][/url] <<other>> [s]media/image.png[/s][/{tag}]"
        );
        let rendered = render_article(&article(&body), &bundle, &ReadOptions::default())?;
        assert!(
            rendered
                .html
                .contains("<a href=\"entry://target\"><strong>target</strong></a>")
        );
        assert!(rendered.html.contains(
            "<a href=\"https://example.com\" rel=\"noreferrer noopener\"><em>сайт</em></a>"
        ));
        assert!(
            rendered
                .html
                .contains("<a href=\"entry://other\">other</a>")
        );
        assert!(
            rendered.html.contains(
                "<img class=\"dsl-image\" src=\"media/image.png\" alt=\"media/image.png\">"
            )
        );
        assert_eq!(rendered.resource_keys, ["\\media\\image.png"]);
        assert!(
            rendered.diagnostics.is_empty(),
            "Лишняя диагностика [{tag}]: {:?}",
            rendered.diagnostics
        );
    }
    Ok(())
}

#[test]
fn служебные_теги_продолжаются_через_границы_оформления_и_абзацев()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    for tag in ["!trs", "trs", "!trn"] {
        let body = format!("[b]начало[{tag}]оба[/b]обычно[/{tag}]");
        let rendered = render_article(&article(&body), &bundle, &ReadOptions::default())?;
        assert!(
            rendered
                .html
                .contains("<div class=\"dsl-body\"><strong>началооба</strong>обычно</div>")
        );
        assert_eq!(rendered.diagnostics.len(), 1);
        assert_eq!(rendered.diagnostics[0].code, "repaired_markup");

        let body = format!("[m1][{tag}]первый[/m][m2]второй[/{tag}][/m]");
        let rendered = render_article(&article(&body), &bundle, &ReadOptions::default())?;
        assert!(rendered.html.contains("<div class=\"dsl-indent dsl-indent-1\">первый</div><div class=\"dsl-indent dsl-indent-2\">второй</div>"));
        assert!(
            rendered
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "repaired_markup")
        );
    }
    Ok(())
}

#[test]
fn неизвестные_и_экранированные_теги_остаются_текстом() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    let rendered = render_article(
        &article(r"[!trs][!новый]текст & данные[/!новый][/!trs] \[!trs\]буквально\[/!trs\]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(rendered.html.contains(
        "<div class=\"dsl-body\">[!новый]текст &amp; данные[/!новый] [!trs]буквально[/!trs]</div>"
    ));
    assert_eq!(rendered.diagnostics.len(), 1);
    assert_eq!(rendered.diagnostics[0].code, "unknown_tag");
    Ok(())
}

#[test]
fn служебные_теги_требуют_парного_закрытия() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    for body in [
        "[!trs]текст",
        "[trs]текст",
        "[!trn]текст",
        "[/!trs]текст",
        "[trs]текст[/!trn]",
    ] {
        assert_eq!(
            render_article(&article(body), &bundle, &ReadOptions::default())
                .expect_err("Непарный служебный тег недопустим")
                .code,
            "invalid_markup",
            "Не отклонена повреждённая разметка: {body}"
        );
    }
    Ok(())
}

#[test]
fn служебные_теги_не_разрывают_ссылки_и_ресурсы() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    for tag in ["!trs", "trs", "!trn"] {
        for element in ["ref", "url", "s"] {
            for body in [
                format!("[{tag}][{element}]target[/{tag}][/{element}]"),
                format!("[{element}][{tag}]target[/{element}][/{tag}]"),
            ] {
                assert_eq!(
                    render_article(&article(&body), &bundle, &ReadOptions::default())
                        .expect_err("Ссылка или ресурс не должны разрываться")
                        .code,
                    "invalid_markup",
                    "Не отклонено пересечение: {body}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn служебные_теги_учитываются_в_ограничении_глубины() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    for tag in ["!trs", "trs", "!trn"] {
        let body = format!(
            "{}текст{}",
            format!("[{tag}]").repeat(129),
            format!("[/{tag}]").repeat(129)
        );
        assert_eq!(
            render_article(&article(&body), &bundle, &ReadOptions::default())
                .expect_err("Вложенность служебных тегов ограничена")
                .code,
            "invalid_markup"
        );
    }
    Ok(())
}

#[test]
fn служебные_теги_не_отменяют_лимит_html() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    let body = format!("[!trs]{}[/!trs]", "&".repeat(2 * 1024 * 1024));
    assert_eq!(
        render_article(&article(&body), &bundle, &ReadOptions::default())
            .expect_err("HTML с экранированным текстом превышает лимит")
            .code,
        "article_too_large"
    );
    Ok(())
}

#[test]
fn отсутствие_файла_сокращений_не_порождает_предупреждения_о_каждой_помете()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let bundle = bundle(temp.path());
    let rendered = render_article(
        &article("[p]сущ.[/p] [p]гл.[/p]"),
        &bundle,
        &ReadOptions::default(),
    )?;
    assert!(rendered.html.contains(
        "<abbr class=\"dsl-abbreviation\">сущ.</abbr> <abbr class=\"dsl-abbreviation\">гл.</abbr>"
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "Отсутствие общего файла не означает ошибку каждой пометы"
    );
    Ok(())
}
