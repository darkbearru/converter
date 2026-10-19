use converter_core::{
    model::{InputBundle, PreparedFile, ReadOptions, RunContext},
    prepare::select_primary_dictionary,
};
use std::path::Path;

fn bundles(root: &Path, sources: &[(&str, &str)]) -> Vec<InputBundle> {
    sources
        .iter()
        .enumerate()
        .map(|(index, (name, body))| {
            let path = root.join(format!("{index}.dsl"));
            std::fs::write(&path, format!("\u{feff}#NAME \"{name}\"\nword\n\t{body}\n"))
                .expect("Исходник");
            InputBundle {
                includes: vec![],
                main: PreparedFile {
                    id: index.to_string(),
                    display_name: format!("{index}.dsl"),
                    path,
                },
                abbreviations: None,
                resources: Vec::new(),
            }
        })
        .collect()
}

#[test]
fn основной_выбирается_по_связям_а_разговорник_предлагается_отдельно()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let input = bundles(
        temp.path(),
        &[
            ("Index", "[ref dict=\"Main\"]word[/ref]"),
            ("Main", "Body"),
            ("PhraseBook", "Other"),
        ],
    );
    let selection = select_primary_dictionary(
        &input,
        &ReadOptions::default(),
        &RunContext::new(temp.path().join("scratch")),
    )?
    .ok_or("Основной")?;
    assert_eq!(selection.main_id, "1");
    assert_eq!(selection.main_title, "Main");
    assert_eq!(selection.independent_ids, ["2"]);
    Ok(())
}

#[test]
fn небольшой_справочник_по_ссылке_не_становится_основным() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let input = bundles(
        temp.path(),
        &[
            (
                "Universal",
                "[ref dict=\"Grammar\"]word[/ref]\nsecond\n body",
            ),
            ("Grammar", "Grammar help"),
        ],
    );
    assert!(
        select_primary_dictionary(
            &input,
            &ReadOptions::default(),
            &RunContext::new(temp.path().join("scratch"))
        )?
        .is_none()
    );
    Ok(())
}

#[test]
fn независимый_большой_словарь_не_меняет_основной_связанной_пары()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let input = bundles(
        temp.path(),
        &[
            ("Index", "[ref dict=\"Main\"]word[/ref]"),
            ("Main", "Body"),
            ("Other", "Other\nsecond\n body"),
        ],
    );
    let selection = select_primary_dictionary(
        &input,
        &ReadOptions::default(),
        &RunContext::new(temp.path().join("scratch")),
    )?
    .ok_or("Основной")?;
    assert_eq!(selection.main_title, "Main");
    assert_eq!(selection.independent_ids, ["2"]);
    Ok(())
}

#[test]
fn неоднозначные_пакеты_сохраняют_ручной_выбор() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    for sources in [
        vec![("One", "Body"), ("Two", "Body")],
        vec![
            ("Index", "[ref dict=\"Main\"]word[/ref]"),
            ("Main", "Body"),
            ("main", "Other"),
        ],
        vec![
            ("One", "[ref dict=\"Two\"]word[/ref]"),
            ("Two", "[ref dict=\"One\"]word[/ref]"),
        ],
        vec![
            ("Index", "[ref dict=\"Missing\"]word[/ref]"),
            ("Main", "Body"),
        ],
        vec![
            (
                "Index",
                "[ref dict=\"Main\"]word[/ref][ref dict=\"Other\"]word[/ref]",
            ),
            ("Main", "Body"),
            ("Other", "Body"),
        ],
    ] {
        let input = bundles(temp.path(), &sources);
        assert!(
            select_primary_dictionary(
                &input,
                &ReadOptions::default(),
                &RunContext::new(temp.path().join("scratch"))
            )?
            .is_none()
        );
    }
    Ok(())
}

#[test]
fn экранированные_теги_и_комментарии_не_создают_связи() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let input = bundles(
        temp.path(),
        &[
            (
                "Index",
                r#"\[ref dict="Main"\]word\[/ref\] {{ [ref dict="Main"]word[/ref] }}"#,
            ),
            ("Main", "Body"),
        ],
    );
    assert!(
        select_primary_dictionary(
            &input,
            &ReadOptions::default(),
            &RunContext::new(temp.path().join("scratch"))
        )?
        .is_none()
    );
    Ok(())
}

#[test]
fn сокращенные_ссылки_не_разбираются_как_межсловарные_теги()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    for body in [
        r#"<<[ref dict="Main"]word[/ref]>>"#,
        r#"<<escaped \>> [ref dict="Main"]word[/ref]>>"#,
    ] {
        let input = bundles(temp.path(), &[("Index", body), ("Main", "Body")]);
        assert!(
            select_primary_dictionary(
                &input,
                &ReadOptions::default(),
                &RunContext::new(temp.path().join("scratch"))
            )?
            .is_none()
        );
    }
    Ok(())
}

#[test]
fn отмена_анализа_не_превращается_в_ручной_выбор() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let input = bundles(
        temp.path(),
        &[("Index", "[ref dict=\"Main\"]word[/ref]"), ("Main", "Body")],
    );
    let ctx = RunContext::new(temp.path().join("scratch"));
    ctx.control.cancel();
    assert_eq!(
        select_primary_dictionary(&input, &ReadOptions::default(), &ctx)
            .expect_err("Отмена")
            .code,
        "cancelled"
    );
    Ok(())
}
